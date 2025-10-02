use cvm_agent_models::bootstrap::{CADDY_ACME_EAB_KEY_ID, CADDY_ACME_EAB_MAC_KEY};
use docker_compose_types::{
    Compose, ComposeNetworks, ComposeVolume, MapOrEmpty, Ports, PublishedPort, Service, StringOrList, TopLevelVolumes,
    Volumes,
};
use std::{
    collections::{HashMap, HashSet},
    iter,
};

const RESERVED_CONTAINERS: &[&str] = &["nilcc-attester", "nilcc-proxy"];
const RESERVED_PORTS: &[u16] = &[80, 443];

pub(crate) fn validate_docker_compose(
    docker_compose: &str,
    public_container_name: &str,
    files: &HashMap<String, Vec<u8>>,
) -> Result<(), DockerComposeValidationError> {
    use DockerComposeValidationError as Error;
    for env in &[CADDY_ACME_EAB_KEY_ID, CADDY_ACME_EAB_MAC_KEY] {
        if docker_compose.contains(env) {
            return Err(Error::ReservedEnv(env));
        }
    }

    let compose: Compose = serde_yaml::from_str(docker_compose)?;
    if compose.services.is_empty() {
        return Err(Error::Invalid("no services defined".into()));
    }
    let top_level_volumes = validate_top_level_volumes(&compose.volumes)?;
    let mut found_public_container = false;
    for (service_name, service) in &compose.services.0 {
        let service = service.as_ref().ok_or_else(|| Error::Invalid(format!("no body in service '{service_name}'")))?;
        let container_names = iter::once(service_name).chain(service.container_name.as_ref());
        for container_name in container_names {
            // Make sure it doesn't contain a substring of our reserved container names
            for reserved in RESERVED_CONTAINERS {
                if container_name.contains(reserved) {
                    return Err(Error::ReservedContainerName(container_name.clone(), service_name.clone()));
                }
            }
            if container_name == public_container_name {
                found_public_container = true;
            }
        }
        validate_service(service, &top_level_volumes, files)
            .map_err(|e| Error::InvalidService(service_name.to_string(), e))?;
    }
    if compose.includes.is_some() {
        return Err(Error::Includes);
    }
    if compose.secrets.is_some() {
        return Err(Error::Secrets);
    }
    validate_networks(&compose.networks)?;
    if found_public_container { Ok(()) } else { Err(Error::PublicContainer(public_container_name.to_string())) }
}

fn validate_service(
    service: &Service,
    top_level_volumes: &HashSet<&str>,
    files: &HashMap<String, Vec<u8>>,
) -> Result<(), ServiceValidationError> {
    use ServiceValidationError as Error;
    validate_ports(&service.ports)?;
    if !service.cap_add.is_empty() {
        return Err(Error::Capabilities);
    }
    if service.privileged {
        return Err(Error::PrivilegedService);
    }
    if !service.security_opt.is_empty() {
        return Err(Error::SecurityOpt);
    }
    if !service.devices.is_empty() {
        return Err(Error::Devices);
    }
    if service.pid.is_some() {
        return Err(Error::Pid);
    }
    if service.ipc.is_some() {
        return Err(Error::Ipc);
    }
    if service.network_mode.is_some() {
        return Err(Error::NetworkMode);
    }
    for volume in &service.volumes {
        validate_volumes(volume, top_level_volumes, files)?;
    }
    if let Some(env) = &service.env_file {
        validate_env_file(env, files)?;
    }
    validate_extends(&service.extends)?;
    if service.cgroup_parent.is_some() {
        return Err(Error::Cgroups);
    }

    Ok(())
}

fn validate_top_level_volumes(volumes: &TopLevelVolumes) -> Result<HashSet<&str>, DockerComposeValidationError> {
    let mut output = HashSet::new();
    for (name, volume) in &volumes.0 {
        if let MapOrEmpty::Map(volume) = volume {
            let ComposeVolume { driver, driver_opts, external, labels, name } = volume;
            if driver.is_some() || !driver_opts.is_empty() || external.is_some() || !labels.is_empty() || name.is_some()
            {
                return Err(DockerComposeValidationError::VolumeAttributes);
            }
        }
        output.insert(name.as_str());
    }
    Ok(output)
}

fn validate_ports(ports: &Ports) -> Result<(), ServiceValidationError> {
    match ports {
        Ports::Short(ports) => {
            for port in ports {
                if let Some(port) = extract_published_ports(port)? {
                    port.validate()?;
                }
            }
        }
        Ports::Long(ports) => {
            for port in ports {
                if let Some(published) = &port.published {
                    published.validate()?;
                }
            }
        }
    };
    Ok(())
}

fn extract_published_ports(s: &str) -> Result<Option<PublishedPort>, ServiceValidationError> {
    use ServiceValidationError::InvalidPorts;
    let s = match s.split_once('/') {
        Some((s, protocol)) => {
            if !matches!(protocol, "udp" | "tcp") {
                return Err(InvalidPorts);
            }
            s
        }
        None => s,
    };
    let port = match s.split_once(':') {
        Some((from, _)) => from,
        None => return Ok(None),
    };
    Ok(Some(PublishedPort::Range(port.to_string())))
}

trait PublishedPortExt {
    fn validate(&self) -> Result<(), ServiceValidationError>;
}

impl PublishedPortExt for PublishedPort {
    fn validate(&self) -> Result<(), ServiceValidationError> {
        use ServiceValidationError as Error;
        let ports = match self {
            Self::Single(port) => *port..=*port,
            Self::Range(spec) => match spec.split_once('-') {
                Some((from, to)) => {
                    let from: u16 = from.parse().map_err(|_| Error::InvalidPorts)?;
                    let to: u16 = to.parse().map_err(|_| Error::InvalidPorts)?;
                    if from > to {
                        return Err(Error::InvalidPorts);
                    }
                    from..=to
                }
                None => {
                    let port = spec.parse::<u16>().map_err(|_| Error::InvalidPorts)?;
                    port..=port
                }
            },
        };
        for port in ports {
            if RESERVED_PORTS.contains(&port) {
                return Err(Error::ReservedPort(port));
            }
        }
        Ok(())
    }
}

fn validate_volumes(
    volume: &Volumes,
    top_level_volumes: &HashSet<&str>,
    files: &HashMap<String, Vec<u8>>,
) -> Result<(), ServiceValidationError> {
    use ServiceValidationError as Error;
    let Volumes::Simple(spec) = volume else {
        return Err(Error::LongFormVolumes);
    };

    let Some((source, _)) = spec.split_once(':') else {
        return Err(Error::VolumeColon);
    };
    if top_level_volumes.contains(source) {
        return Ok(());
    }
    validate_volume_path(source, files)
}

fn validate_volume_path(path: &str, files: &HashMap<String, Vec<u8>>) -> Result<(), ServiceValidationError> {
    use ServiceValidationError as Error;
    if path.contains("../") {
        return Err(Error::MountDotDot);
    }

    // Make sure whatever we're trying to mount is at `$FILES`.
    let path = path.strip_prefix("$FILES").or_else(|| path.strip_prefix("${FILES}")).ok_or(Error::FilesEnvVar)?;

    // We're okay with mounting _all_ files under `$FILES`.
    if path.is_empty() || path == "/" {
        return Ok(());
    }

    // Make sure whatever is left starts with "/"
    let path = path.strip_prefix('/').ok_or_else(|| ServiceValidationError::NoSourceMount(path.to_string()))?;

    // Otherwise make sure the referenced files are mounted.
    if !files.contains_key(path) {
        return Err(ServiceValidationError::MissingMount(path.to_string()));
    }

    Ok(())
}

fn validate_env_file(env: &StringOrList, files: &HashMap<String, Vec<u8>>) -> Result<(), ServiceValidationError> {
    match env {
        StringOrList::Simple(path) => validate_volume_path(path, files),
        StringOrList::List(paths) => {
            for path in paths {
                validate_volume_path(path, files)?;
            }
            Ok(())
        }
    }
}

fn validate_extends(attrs: &HashMap<String, String>) -> Result<(), ServiceValidationError> {
    use ServiceValidationError as Error;
    if attrs.is_empty() {
        return Ok(());
    }
    if attrs.get("file").is_some() {
        return Err(Error::ExtendFile);
    }
    Ok(())
}

fn validate_networks(networks: &ComposeNetworks) -> Result<(), DockerComposeValidationError> {
    use DockerComposeValidationError as Error;
    for network in networks.0.values() {
        let MapOrEmpty::Map(network) = network else {
            continue;
        };
        if network.driver.is_some() {
            return Err(Error::NetworkDriver);
        }
        if !network.driver_opts.is_empty() {
            return Err(Error::NetworkDriverOpts);
        }
        if network.ipam.is_some() {
            return Err(Error::NetworkIpam);
        }
    }
    Ok(())
}

#[derive(Debug, thiserror::Error)]
pub(crate) enum DockerComposeValidationError {
    #[error("malformed docker compose: {0}")]
    Malformed(#[from] serde_yaml::Error),

    #[error("invalid docker compose: {0}")]
    Invalid(String),

    #[error("cannot use reserved key: '{0}'")]
    ReservedEnv(&'static str),

    #[error("cannot use reserved container name '{0}' in service '{1}'")]
    ReservedContainerName(String, String),

    #[error("container {0} is not part of compose file")]
    PublicContainer(String),

    #[error("volume definitions cannot contain any attributes")]
    VolumeAttributes,

    #[error("cannot use includes")]
    Includes,

    #[error("cannot use secrets")]
    Secrets,

    #[error("cannot set network driver")]
    NetworkDriver,

    #[error("cannot set network driver opts")]
    NetworkDriverOpts,

    #[error("cannot set network ipam")]
    NetworkIpam,

    #[error("invalid service '{0}': {1}")]
    InvalidService(String, ServiceValidationError),
}

#[derive(Debug, thiserror::Error)]
pub(crate) enum ServiceValidationError {
    #[error("cannot extend service in external file")]
    ExtendFile,

    #[error("cannot publish reserved port: {0}")]
    ReservedPort(u16),

    #[error("invalid ports definition")]
    InvalidPorts,

    #[error("volumes can only use short form")]
    LongFormVolumes,

    #[error("missing ':' in volume mount")]
    VolumeColon,

    #[error("mounts need to start from the $FILES environment variable")]
    FilesEnvVar,

    #[error("mounts cannot use '../'")]
    MountDotDot,

    #[error("mount '{0}' does not exist")]
    MissingMount(String),

    #[error("volume '{0}' must mount either '/' or a specific file")]
    NoSourceMount(String),

    #[error("privileged services are not allowed")]
    PrivilegedService,

    #[error("cannot use capabilities")]
    Capabilities,

    #[error("cannot use cgroups")]
    Cgroups,

    #[error("cannot use security-opt")]
    SecurityOpt,

    #[error("cannot use devices")]
    Devices,

    #[error("cannot set pid mode")]
    Pid,

    #[error("cannot use ipc")]
    Ipc,

    #[error("cannot use network-mode")]
    NetworkMode,
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;
    use std::fmt;

    impl From<DockerComposeValidationError> for ServiceValidationError {
        fn from(e: DockerComposeValidationError) -> Self {
            match e {
                DockerComposeValidationError::InvalidService(_, e) => e,
                _ => panic!("not a service validation error: {e}"),
            }
        }
    }

    fn validate_success(compose: &str, public_container_name: &str) {
        validate_docker_compose(compose, public_container_name, &Default::default()).expect("validation failed");
    }

    fn validate_failure<E>(compose: &str, public_container_name: &str, expected: E)
    where
        DockerComposeValidationError: Into<E>,
        E: fmt::Display,
    {
        let err = validate_docker_compose(compose, public_container_name, &Default::default())
            .expect_err("validation succeeded");
        assert_eq!(err.into().to_string(), expected.to_string());
    }

    #[test]
    fn valid_minimal() {
        let compose = r"
services:
  api:
    image: caddy:2
";
        validate_success(compose, "api");
    }

    #[test]
    fn valid_container_name() {
        let compose = r"
services:
  foo:
    container_name: api
    image: caddy:2
";
        validate_success(compose, "api");
        validate_success(compose, "foo");
    }

    #[test]
    fn valid_complex() {
        let compose = r#"
services:
  api:
    image: caddy:2
    ports:
      - "80"
      - "81:80"
      - "82:80/tcp"
      - "83:80/tcp"
      - "84-85:80/tcp"
      - "86-87:80-81/tcp"
      - "86-87:80-81/udp"
    extends: some-service
    environment:
      FOO: ${FOO_VAR}
    volumes:
      - "$FILES/foo:/tmp/foo"
      - $FILES/foo:/tmp/bar
      - ${FILES}/foo:/tmp/tar
      - ${FILES}/foo:/tmp/tar2:rw
      - ${FILES}/bar:/tmp/jar
      - other:/tmp/other
    env_file:
      - $FILES/dotenv1
      - ${FILES}/dotenv2
    command: "caddy"
    deploy:
      resources:
        reservations:
          devices:
            - driver: nvidia
              capabilities: [gpu]
volumes:
  other:
"#;
        let files = ["foo", "bar", "dotenv1", "dotenv2"].into_iter().map(|file| (file.to_string(), vec![])).collect();
        validate_docker_compose(compose, "api", &files).expect("validation failed");
    }

    #[test]
    fn valid_complex_long_ports() {
        let compose = r#"
services:
  api:
    image: caddy:2
    ports:
      - published: 88
        target: 1024
      - published: "88"
        target: 1024
      - published: "89-90"
        target: 2048
    environment:
      FOO: ${FOO_VAR}
    command: "caddy"
"#;
        validate_success(compose, "api");
    }

    #[test]
    fn container_not_found() {
        let compose = r#"
services:
  api:
    image: caddy:2
    command: "caddy"
"#;
        validate_failure(compose, "other", DockerComposeValidationError::PublicContainer("other".into()));
    }

    #[test]
    fn privileged_service() {
        let compose = r#"
services:
  api:
    image: caddy:2
    command: "caddy"
    privileged: true
"#;
        validate_failure(compose, "api", ServiceValidationError::PrivilegedService);
    }

    #[test]
    fn capabilities() {
        let compose = r#"
services:
  api:
    image: caddy:2
    command: "caddy"
    cap_add:
      - NET_ADMIN
"#;
        validate_failure(compose, "api", ServiceValidationError::Capabilities);
    }

    #[test]
    fn cgroup_parent() {
        let compose = r#"
services:
  api:
    image: caddy:2
    command: "caddy"
    cgroup_parent: foo
"#;
        validate_failure(compose, "api", ServiceValidationError::Cgroups);
    }

    #[test]
    fn extends() {
        let compose = r#"
services:
  api:
    image: caddy:2
    command: "caddy"
    extends:
      file: potato.yaml
"#;
        validate_failure(compose, "api", ServiceValidationError::ExtendFile);
    }

    #[test]
    fn includes() {
        let compose = r#"
services:
  api:
    image: caddy:2
    command: "caddy"
includes:
  - foo
"#;
        validate_failure(compose, "api", DockerComposeValidationError::Includes);
    }

    #[test]
    fn secrets() {
        let compose = r#"
services:
  api:
    image: caddy:2
    command: "caddy"
secrets:
  foo:
    file: foo.txt
"#;
        validate_failure(compose, "api", DockerComposeValidationError::Secrets);
    }

    #[test]
    fn security_opt() {
        let compose = r#"
services:
  api:
    image: caddy:2
    command: "caddy"
    security_opt:
      - foo
"#;
        validate_failure(compose, "api", ServiceValidationError::SecurityOpt);
    }

    #[test]
    fn devices() {
        let compose = r#"
services:
  api:
    image: caddy:2
    command: "caddy"
    devices:
      - foo
"#;
        validate_failure(compose, "api", ServiceValidationError::Devices);
    }

    #[test]
    fn pid() {
        let compose = r#"
services:
  api:
    image: caddy:2
    command: "caddy"
    pid: "42"
"#;
        validate_failure(compose, "api", ServiceValidationError::Pid);
    }

    #[test]
    fn ipc() {
        let compose = r#"
services:
  api:
    image: caddy:2
    command: "caddy"
    ipc: "42"
"#;
        validate_failure(compose, "api", ServiceValidationError::Ipc);
    }

    #[test]
    fn network_mode() {
        let compose = r#"
services:
  api:
    image: caddy:2
    command: "caddy"
    network_mode: "42"
"#;
        validate_failure(compose, "api", ServiceValidationError::NetworkMode);
    }

    #[test]
    fn network_driver() {
        let compose = r#"
services:
  api:
    image: caddy:2
    command: "caddy"
networks:
  foo:
    driver: host
"#;
        validate_failure(compose, "api", DockerComposeValidationError::NetworkDriver);
    }

    #[test]
    fn network_driver_opts() {
        let compose = r#"
services:
  api:
    image: caddy:2
    command: "caddy"
networks:
  foo:
    driver_opts:
      foo: bar
"#;
        validate_failure(compose, "api", DockerComposeValidationError::NetworkDriverOpts);
    }

    #[test]
    fn network_ipam() {
        let compose = r#"
services:
  api:
    image: caddy:2
    command: "caddy"
networks:
  foo:
    ipam:
      driver: bar
"#;
        validate_failure(compose, "api", DockerComposeValidationError::NetworkIpam);
    }

    #[test]
    fn use_api_socket() {
        let compose = r#"
services:
  api:
    image: caddy:2
    command: "caddy"
    use_api_socket: true
"#;
        // Note: this option is currently unsupported by `docker-compose-types` but we want to make
        // sure this test keeps failing if they ever add support. Once they do, we should
        // explicitly check for this option and prevent it from being set.
        validate_docker_compose(compose, "api", &Default::default()).expect_err("success");
    }

    #[test]
    fn external_links() {
        let compose = r#"
services:
  api:
    image: caddy:2
    command: "caddy"
    external_links:
      - foo
"#;
        // same as `use_api_socket` this is unsupported but we don't want it once it is
        validate_docker_compose(compose, "api", &Default::default()).expect_err("success");
    }

    #[test]
    fn cgroup() {
        let compose = r#"
services:
  api:
    image: caddy:2
    command: "caddy"
    cgroup: host
"#;
        // same as above
        validate_docker_compose(compose, "api", &Default::default()).expect_err("success");
    }

    #[test]
    fn configs() {
        let compose = r#"
services:
  api:
    image: caddy:2
    command: "caddy"
    configs:
      - foo
"#;
        // same as above
        validate_docker_compose(compose, "api", &Default::default()).expect_err("success");
    }

    #[test]
    fn configs_top_level() {
        let compose = r#"
services:
  api:
    image: caddy:2
    command: "caddy"
configs:
  my_config:
    file: ./my_config.txt
"#;
        // same as above
        validate_docker_compose(compose, "api", &Default::default()).expect_err("success");
    }

    #[rstest]
    #[case::port_80("80", 80)]
    #[case::port_443("443", 443)]
    #[case::port_range_single("80-80", 80)]
    #[case::port_range_middle("79-81", 80)]
    #[case::port_range_edge_left("80-81", 80)]
    #[case::port_range_edge_right("79-80", 80)]
    fn reserved_port(#[case] spec: &str, #[case] port: u16) {
        let compose = format!(
            r#"
services:
  api:
    image: caddy:2
    ports:
      - "{spec}:42"
"#
        );
        validate_failure(&compose, "api", ServiceValidationError::ReservedPort(port));
    }

    #[test]
    fn reserved_long_range() {
        let compose = r"
services:
  api:
    image: caddy:2
    ports:
      - target: 42
        published: 80
";
        validate_failure(compose, "api", ServiceValidationError::ReservedPort(80));
    }

    #[rstest]
    #[case::attester("nilcc-attester")]
    #[case::proxy("nilcc-proxy")]
    fn reserved_service_name(#[case] service: &str) {
        let compose = format!(
            r#"
services:
  {service}:
    image: caddy:2
    command: "caddy"
"#
        );
        validate_failure(
            &compose,
            service,
            DockerComposeValidationError::ReservedContainerName(service.into(), service.into()),
        );
    }

    #[rstest]
    #[case::attester("nilcc-attester")]
    #[case::proxy("nilcc-proxy")]
    fn reserved_container_name(#[case] service: &str) {
        let container_name = format!("cvm-{service}-1");
        let compose = format!(
            r#"
services:
  api:
    image: caddy:2
    container_name: {container_name}
    command: "caddy"
"#
        );
        validate_failure(
            &compose,
            "api",
            DockerComposeValidationError::ReservedContainerName(container_name, "api".into()),
        );
    }

    #[rstest]
    #[case::eab_key_id("CADDY_ACME_EAB_KEY_ID")]
    #[case::eab_mac_key("CADDY_ACME_EAB_MAC_KEY")]
    fn reserved_env_vars(#[case] env: &'static str) {
        let compose = format!(
            r#"
services:
  api:
    image: caddy:2
    environment:
      FOO: ${env}
"#
        );
        validate_failure(&compose, "api", DockerComposeValidationError::ReservedEnv(env));
    }

    #[rstest]
    #[case::no_colom("/tmp/hello", ServiceValidationError::VolumeColon)]
    #[case::no_files("/tmp/hello:", ServiceValidationError::FilesEnvVar)]
    #[case::files_dotdot("$FILES/../proc/foo:", ServiceValidationError::MountDotDot)]
    #[case::dotdot("../proc/foo:", ServiceValidationError::MountDotDot)]
    #[case::no_slash("$FILESfoo:/tmp", ServiceValidationError::NoSourceMount("foo".into()))]
    fn invalid_volume_source(#[case] source: &'static str, #[case] error: ServiceValidationError) {
        let compose = format!(
            r#"
services:
  api:
    image: caddy:2
    volumes:
      - {source}/tmp/foo
"#
        );
        validate_failure(&compose, "api", error);
    }

    #[test]
    fn missing_file() {
        let compose = r#"
services:
  api:
    image: caddy:2
    volumes:
      - $FILES/foo/bar:/tmp/foo
"#;
        validate_failure(&compose, "api", ServiceValidationError::MissingMount("foo/bar".into()));
    }

    #[rstest]
    #[case::no_files("/tmp/hello", ServiceValidationError::FilesEnvVar)]
    #[case::files_dotdot("$FILES/../proc/foo", ServiceValidationError::MountDotDot)]
    #[case::dotdot("../proc/foo", ServiceValidationError::MountDotDot)]
    fn invalid_env_file(#[case] env_file: &'static str, #[case] error: ServiceValidationError) {
        let compose = format!(
            r#"
services:
  api:
    image: caddy:2
    env_file: {env_file}
"#
        );
        validate_failure(&compose, "api", error);
    }

    #[test]
    fn missing_env_file() {
        let compose = r#"
services:
  api:
    image: caddy:2
    env_file: $FILES/foo/bar
"#;
        validate_failure(&compose, "api", ServiceValidationError::MissingMount("foo/bar".into()));
    }
}
