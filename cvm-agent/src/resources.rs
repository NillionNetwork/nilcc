use crate::routes::VmType;
use serde::Deserialize;

static CADDYFILE: &str = include_str!("../resources/Caddyfile");
static DOCKER_COMPOSE: &str = include_str!("../resources/docker-compose.yaml");
static DOCKER_COMPOSE_DEPLOY: &str = r"
    deploy:
      resources:
        reservations:
          devices:
            - driver: nvidia
              capabilities: [gpu]";

#[derive(Debug, Deserialize, PartialEq)]
pub struct ContainerMetadata {
    container: String,
    port: u16,
}

#[derive(Debug, Deserialize, PartialEq)]
pub struct ApplicationMetadata {
    hostname: String,
    api: ContainerMetadata,
}

pub struct Resources {
    pub caddyfile: Vec<u8>,
    pub docker_compose: Vec<u8>,
}

impl Resources {
    pub fn render(metadata: &ApplicationMetadata, vm_type: &VmType) -> Self {
        let container_target = format!("{}:{}", metadata.api.container, metadata.api.port);
        let caddyfile = CADDYFILE
            .replace("{NILCC_PROXY_HOSTNAME}", &metadata.hostname)
            .replace("{NILCC_PROXY_TARGET}", &container_target)
            .into_bytes();
        let replacement = match vm_type {
            VmType::Cpu => "",
            VmType::Gpu => DOCKER_COMPOSE_DEPLOY,
        };
        let docker_compose = DOCKER_COMPOSE.replace("{DOCKER_COMPOSE_DEPLOY}", replacement).into();
        Self { caddyfile, docker_compose }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use regex::bytes::Regex;
    use std::sync::LazyLock;

    static VERSION_REGEX: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new("\n    image: ghcr.io/nillionnetwork/nilcc-attester:[^\n]+").expect("invalid regex")
    });

    fn replace_version(compose: &[u8]) -> Vec<u8> {
        VERSION_REGEX.replace(compose, b"\n    image: ghcr.io/nillionnetwork/nilcc-attester:ATTESTER_VERSION").to_vec()
    }

    #[test]
    fn caddyfile() {
        let metadata = ApplicationMetadata {
            hostname: "foo.com".into(),
            api: ContainerMetadata { container: "api".into(), port: 1337 },
        };
        let caddyfile = Resources::render(&metadata, &VmType::Cpu).caddyfile;
        let expected = "{
    servers {
        protocols h1 h2
    }
}

(ssl_config) {
    tls {
        protocols tls1.2 tls1.3
        ca https://acme.zerossl.com/v2/DV90
        eab {$CADDY_ACME_EAB_KEY_ID} {$CADDY_ACME_EAB_MAC_KEY}
    }
}

https://foo.com {
    import ssl_config

    handle_path /nilcc/* {
      reverse_proxy http://nilcc-attester
    }

    reverse_proxy /* api:1337
}
";
        assert_eq!(String::from_utf8_lossy(&caddyfile), expected);
    }

    #[test]
    fn compose_cpu() {
        let metadata = ApplicationMetadata {
            hostname: "foo.com".into(),
            api: ContainerMetadata { container: "api".into(), port: 1337 },
        };
        let compose = Resources::render(&metadata, &VmType::Cpu).docker_compose;
        let compose = replace_version(&compose);
        let expected = r#"services:
  nilcc-attester:
    image: ghcr.io/nillionnetwork/nilcc-attester:ATTESTER_VERSION
    restart: unless-stopped
    privileged: true
    volumes:
      - "/dev/sev-guest:/dev/sev-guest"
    ports:
      - 80
    environment:
      APP__SERVER__BIND_ENDPOINT: "0.0.0.0:80"
      APP__NILCC_VERSION: ${NILCC_VERSION}
      APP__VM_TYPE: ${NILCC_VM_TYPE}
      APP__ATTESTATION_DOMAIN: ${NILCC_DOMAIN}
      NO_COLOR: 1
    healthcheck:
      test: ["CMD", "curl", "-f", "http://localhost/health"]
    

  nilcc-proxy:
    image: caddy:2.10.0
    restart: unless-stopped
    cap_add:
      - NET_ADMIN
    ports:
      - "80:80"
      - "443:443"
    environment:
      CADDY_ACME_EAB_KEY_ID: ${CADDY_ACME_EAB_KEY_ID}
      CADDY_ACME_EAB_MAC_KEY: ${CADDY_ACME_EAB_MAC_KEY}
    volumes:
      - ${CADDY_INPUT_FILE}:/etc/caddy/Caddyfile
"#;
        assert_eq!(String::from_utf8_lossy(&compose), expected);
    }

    #[test]
    fn compose_gpu() {
        let metadata = ApplicationMetadata {
            hostname: "foo.com".into(),
            api: ContainerMetadata { container: "api".into(), port: 1337 },
        };
        let compose = Resources::render(&metadata, &VmType::Gpu).docker_compose;
        let compose = replace_version(&compose);
        let expected = r#"services:
  nilcc-attester:
    image: ghcr.io/nillionnetwork/nilcc-attester:ATTESTER_VERSION
    restart: unless-stopped
    privileged: true
    volumes:
      - "/dev/sev-guest:/dev/sev-guest"
    ports:
      - 80
    environment:
      APP__SERVER__BIND_ENDPOINT: "0.0.0.0:80"
      APP__NILCC_VERSION: ${NILCC_VERSION}
      APP__VM_TYPE: ${NILCC_VM_TYPE}
      APP__ATTESTATION_DOMAIN: ${NILCC_DOMAIN}
      NO_COLOR: 1
    healthcheck:
      test: ["CMD", "curl", "-f", "http://localhost/health"]
    
    deploy:
      resources:
        reservations:
          devices:
            - driver: nvidia
              capabilities: [gpu]

  nilcc-proxy:
    image: caddy:2.10.0
    restart: unless-stopped
    cap_add:
      - NET_ADMIN
    ports:
      - "80:80"
      - "443:443"
    environment:
      CADDY_ACME_EAB_KEY_ID: ${CADDY_ACME_EAB_KEY_ID}
      CADDY_ACME_EAB_MAC_KEY: ${CADDY_ACME_EAB_MAC_KEY}
    volumes:
      - ${CADDY_INPUT_FILE}:/etc/caddy/Caddyfile
"#;
        assert_eq!(String::from_utf8_lossy(&compose), expected);
    }
}
