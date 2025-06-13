use crate::{
    agent_service::{AgentService, AgentServiceArgs},
    http_client::RestNilccApiClient,
    repositories::{sqlite::SqliteDb, workload::SqliteWorkloadRepository},
    services::{sni_proxy::MockSniProxyService, vm::MockVmService},
};
use anyhow::Context;
use std::time::Duration;
use tracing::info;
use tracing_test::traced_test;
use uuid::Uuid;
use wiremock::{
    matchers::{header, method, path, path_regex},
    Mock, MockServer, ResponseTemplate,
};

#[traced_test]
#[tokio::test]
async fn test_agent_registration_with_mock_server() -> anyhow::Result<()> {
    let mock_server = MockServer::start().await;
    let api_base_url = mock_server.uri();
    let api_key = "test-api-key-123";
    let agent_id = Uuid::parse_str("f7b27e21-eabb-4acb-8cd7-1d8113fd2237").context("Failed to parse test agent ID")?;

    info!("Mock server started at: {api_base_url}");

    Mock::given(method("POST"))
        .and(path("/api/v1/metal-instances/~/register"))
        .and(header("x-api-key", api_key))
        .respond_with(ResponseTemplate::new(201).set_body_json(&()))
        .expect(1)
        .mount(&mock_server)
        .await;

    Mock::given(method("POST"))
        .and(path_regex(format!("/api/v1/metal-instances/{agent_id}/~/sync")))
        .and(header("x-api-key", api_key))
        .respond_with(ResponseTemplate::new(200).set_body_json(&()))
        .mount(&mock_server)
        .await;

    let api_client = Box::new(RestNilccApiClient::new(api_base_url, api_key.to_string())?);
    let db = SqliteDb::connect("sqlite://:memory:").await.expect("failed to create db");
    let workload_repository = Box::new(SqliteWorkloadRepository::new(db));
    let vm_service = Box::new(MockVmService::default());
    let sni_proxy_service = Box::new(MockSniProxyService::new());
    let args = AgentServiceArgs {
        agent_id,
        api_client,
        workload_repository,
        vm_service,
        sni_proxy_service,
        sync_interval: Duration::from_secs(1),
        dns_subdomain: "workloads.nilcc.com".to_string(),
        start_port_range: 10000,
        end_port_range: 20000,
    };
    let agent_service = AgentService::new(args);

    let _handle = agent_service.run().await.expect("failed to run service");
    tokio::time::sleep(Duration::from_millis(1500)).await;

    let received_requests = mock_server.received_requests().await.unwrap_or_default();
    let registration_path = "/api/v1/metal-instances/~/register";
    let registration_requests: Vec<_> =
        received_requests.iter().filter(|req| req.url.path() == registration_path && req.method == "POST").collect();
    assert_eq!(registration_requests.len(), 1, "Expected exactly one registration request.");
    info!("Registration call verified.");

    info!("Waiting for status reports (approx 3 seconds for 2 syncs at 1s interval)...");
    tokio::time::sleep(Duration::from_secs(3)).await;

    let received_sync_requests = mock_server.received_requests().await.unwrap_or_default();
    let sync_request_path = format!("/api/v1/metal-instances/{agent_id}/~/sync");
    let sync_reports: Vec<_> = received_sync_requests
        .iter()
        .filter(|req| req.url.path() == sync_request_path.as_str() && req.method == "POST")
        .collect();

    assert!(sync_reports.len() >= 1, "Expected at least one sync request. Found {}.", sync_reports.len());
    info!("Received {} sync reports. Verification successful.", sync_reports.len());

    info!("Requesting AgentService shutdown...");

    mock_server.reset().await;

    Ok(())
}
