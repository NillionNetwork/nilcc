use nilcc_agent::grpc::nilcc::agent::v1::{
    info::{AgentInfo, MachineInfo},
    registration::{
        registration_service_server::{RegistrationService, RegistrationServiceServer},
        RegisterAgentRequest, RegisterAgentResponse,
    },
};

use nilcc_agent::{
    agent_service::AgentService,
    grpc::nilcc::agent::v1::info::{AllocatableResources, VirtualizationInfo},
};
use std::{net::SocketAddr, time::Duration};
use tokio::sync::oneshot;
use tonic::{transport::Server, Request, Response, Status};
use tracing::{error, info};

#[derive(Debug, Default)]
struct TestRegistrationServiceImpl {}

#[tonic::async_trait]
impl RegistrationService for TestRegistrationServiceImpl {
    async fn register_agent(
        &self,
        request: Request<RegisterAgentRequest>,
    ) -> Result<Response<RegisterAgentResponse>, Status> {
        let req_data = request.into_inner();

        let agent_id_from_request = req_data.agent_info.as_ref().map_or_else(String::new, |info| info.agent_id.clone());

        info!("RegisterAgent request received for agent_id: '{}'", agent_id_from_request);

        let response_message = format!("TestServer: Agent '{}' registered successfully.", agent_id_from_request);
        info!("Sending response: {}", response_message);
        let reply = RegisterAgentResponse { agent_id: agent_id_from_request, message: response_message, success: true };

        Ok(Response::new(reply))
    }
}

async fn run_test_server(addr: SocketAddr, shutdown_rx: oneshot::Receiver<()>) -> Result<(), tonic::transport::Error> {
    let service_impl = TestRegistrationServiceImpl::default();
    let server = RegistrationServiceServer::new(service_impl);

    info!("Starting gRPC server on {}", addr);

    Server::builder()
        .add_service(server)
        .serve_with_shutdown(addr, async {
            shutdown_rx.await.ok();
            info!("Shutdown signal received, stopping server on {}.", addr);
        })
        .await
}

#[tokio::test]
async fn test_agent_registration_with_test_server() -> anyhow::Result<()> {
    let _ = tracing_subscriber::fmt().try_init();
    let (shutdown_tx, shutdown_rx) = oneshot::channel::<()>();

    // get random available available port
    let listener = std::net::TcpListener::bind("127.0.0.1:0")?;
    let actual_addr = listener.local_addr()?;
    let server_uri = format!("http://{}", actual_addr);
    drop(listener);

    let server_task = tokio::spawn(run_test_server(actual_addr, shutdown_rx));
    // ensure the server is up before connecting
    tokio::time::sleep(Duration::from_millis(200)).await;

    let mut agent = AgentService::connect(server_uri.clone()).await.expect("Failed to connect to test server");

    let agent_info = AgentInfo { agent_id: "test-agent".to_string(), agent_version: "0.1.0".to_string() };
    let machine_info = MachineInfo { hardware_id: "test-id".to_string(), ..Default::default() };

    let response = agent
        .register(agent_info, machine_info, VirtualizationInfo::default(), AllocatableResources::default())
        .await?;
    info!("Agent registration response: {:?}", response);

    assert!(response.success, "Registration should be successful");
    assert_eq!(response.agent_id, "test-agent", "Agent ID in response should match request");

    if shutdown_tx.send(()).is_err() {
        error!("Failed to send shutdown signal to server.");
    }

    match tokio::time::timeout(Duration::from_secs(5), server_task).await {
        Ok(_) => info!("Test server shut down."),
        Err(_) => error!("Test serve failed to shut down."),
    }

    Ok(())
}
