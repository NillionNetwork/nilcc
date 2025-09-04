use anyhow::Context;
use reqwest::{tls::TlsInfo, ClientBuilder};
use sha2::{Digest, Sha256};
use std::net::ToSocketAddrs;
use x509_parser::parse_x509_certificate;

pub struct CertFetcher {
    pub proxy_endpoint: String,
    pub server_name: String,
}

impl CertFetcher {
    pub async fn fetch_fingerprint(&self) -> anyhow::Result<[u8; 32]> {
        let Self { proxy_endpoint, server_name } = self;
        let addresses: Vec<_> = proxy_endpoint.to_socket_addrs().context("Failed to resolve proxy hostname")?.collect();
        let client = ClientBuilder::default()
            .tls_info(true)
            .danger_accept_invalid_certs(true)
            .resolve_to_addrs(server_name, &addresses)
            .build()
            .context("Failed to build HTTP client")?;
        let response = client.get(format!("https://{server_name}")).send().await.context("Failed to send request")?;
        let info = response.extensions().get::<TlsInfo>().context("No TLS information")?;
        let cert = info.peer_certificate().context("No certificate in TLS info")?;
        let (_, cert) = parse_x509_certificate(cert).context("Invalid TLS certificate")?;
        let pubkey = cert.tbs_certificate.subject_pki;
        let digest = Sha256::digest(pubkey.raw);
        Ok(digest.into())
    }
}
