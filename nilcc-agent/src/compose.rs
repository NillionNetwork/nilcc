use cvm_agent_models::bootstrap::{CADDY_ACME_EAB_KEY_ID, CADDY_ACME_EAB_MAC_KEY};
use docker_compose_types::{Compose, Ports, PublishedPort};
use std::iter;

const RESERVED_CONTAINERS: &[&str] = &["nilcc-attester", "nilcc-proxy"];
const RESERVED_PORTS: &[u16] = &[80, 443];

pub(crate) fn validate_docker_compose(
    docker_compose: &str,
    public_container_name: &str,
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
    let mut found_public_container = false;
    for (name, service) in &compose.services.0 {
        let service = service.as_ref().ok_or_else(|| Error::Invalid(format!("no body in service '{name}'")))?;
        let names = iter::once(name).chain(service.container_name.as_ref());
        for name in names {
            // Make sure it doesn't contain a substring of our reserved container names
            for reserved in RESERVED_CONTAINERS {
                if name.contains(reserved) {
                    return Err(Error::ReservedContainerName(name.clone()));
                }
            }
            if name == public_container_name {
                found_public_container = true;
            }
            validate_ports(&service.ports)?;
        }
    }
    if found_public_container {
        Ok(())
    } else {
        Err(Error::PublicContainer(public_container_name.to_string()))
    }
}

fn validate_ports(ports: &Ports) -> Result<(), DockerComposeValidationError> {
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

fn extract_published_ports(s: &str) -> Result<Option<PublishedPort>, DockerComposeValidationError> {
    use DockerComposeValidationError::InvalidPorts;
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
    fn validate(&self) -> Result<(), DockerComposeValidationError>;
}

impl PublishedPortExt for PublishedPort {
    fn validate(&self) -> Result<(), DockerComposeValidationError> {
        use DockerComposeValidationError as Error;
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

#[derive(Debug, thiserror::Error)]
pub(crate) enum DockerComposeValidationError {
    #[error("maformed docker compose: {0}")]
    Malformed(#[from] serde_yaml::Error),

    #[error("invalid docker compose: {0}")]
    Invalid(String),

    #[error("cannot use reserved key: '{0}'")]
    ReservedEnv(&'static str),

    #[error("cannot use reserved container name '{0}'")]
    ReservedContainerName(String),

    #[error("cannot publish reserved port: {0}")]
    ReservedPort(u16),

    #[error("invalid ports definition")]
    InvalidPorts,

    #[error("container {0} is not part of compose file")]
    PublicContainer(String),
}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use super::*;

    fn validate_success(compose: &str, public_container_name: &str) {
        validate_docker_compose(compose, public_container_name).expect("validation failed");
    }

    fn validate_failure(compose: &str, public_container_name: &str, expected: DockerComposeValidationError) {
        let err = validate_docker_compose(compose, public_container_name).expect_err("validation succeeded");
        assert_eq!(err.to_string(), expected.to_string());
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
    environment:
      FOO: \${ FOO_VAR }
    command: "caddy"
"#;
        validate_success(compose, "api");
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
        validate_failure(&compose, "api", DockerComposeValidationError::ReservedPort(port));
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
        validate_failure(compose, "api", DockerComposeValidationError::ReservedPort(80));
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
        validate_failure(&compose, service, DockerComposeValidationError::ReservedContainerName(service.into()));
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
        validate_failure(&compose, "api", DockerComposeValidationError::ReservedContainerName(container_name));
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
}
