use serde::Deserialize;

static CADDYFILE: &str = include_str!("../services/Caddyfile");
static DOCKER_COMPOSE: &str = include_str!("../services/docker-compose.yaml");

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
        let caddyfile = CADDYFILE
            .replace("{NILCC_PROXY_HOSTNAME}", &metadata.hostname)
            .replace("{NILCC_PROXY_TARGET}", &metadata.api.container)
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
        let expected = "(ssl_config) {
    tls {
        protocols tls1.2 tls1.3
    }
}

https://foo.com {
    import ssl_config

    handle_path /nilcc/* {
      reverse_proxy http://nilcc-attester
    }

    reverse_proxy /* api
}
";
        assert_eq!(caddyfile, expected.as_bytes());
    }
}
