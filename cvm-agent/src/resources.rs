use serde::Deserialize;

static CADDYFILE: &str = include_str!("../resources/Caddyfile");
static DOCKER_COMPOSE: &str = include_str!("../resources/docker-compose.yaml");

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
    pub fn render(metadata: &ApplicationMetadata) -> Self {
        let container_target = format!("{}:{}", metadata.api.container, metadata.api.port);
        let caddyfile = CADDYFILE
            .replace("{NILCC_PROXY_HOSTNAME}", &metadata.hostname)
            .replace("{NILCC_PROXY_TARGET}", &container_target)
            .into_bytes();
        let docker_compose = DOCKER_COMPOSE.as_bytes().to_vec();
        Self { caddyfile, docker_compose }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn caddyfile() {
        let metadata = ApplicationMetadata {
            hostname: "foo.com".into(),
            api: ContainerMetadata { container: "api".into(), port: 1337 },
        };
        let caddyfile = Resources::render(&metadata).caddyfile;
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
}
