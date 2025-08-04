use serde::Deserialize;

static CADDYFILE: &str = include_str!("../resources/caddy.json");
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
