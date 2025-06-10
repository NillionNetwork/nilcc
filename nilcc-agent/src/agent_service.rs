use crate::{
    build_info::get_agent_version,
    data_schemas::{MetalInstance, MetalInstanceDetails, SyncRequest, SyncResponse, Workload},
    gpu,
    http_client::NilccApiClient,
    repositories::workload::WorkloadRepository,
};
use anyhow::{Context, Result};
use std::{collections::HashMap, time::Duration};
use sysinfo::{Disks, System};
use tokio::sync::watch;
use tracing::{error, info, warn};
use uuid::Uuid;

/// Arguments to the agent service.
pub struct AgentServiceArgs {
    /// The identifier for this agent.
    pub agent_id: Uuid,

    /// The nilcc-api client.
    pub api_client: Box<dyn NilccApiClient>,

    /// The workloads repository.
    pub workload_repository: Box<dyn WorkloadRepository>,

    /// The interval to use for sync requests.
    pub sync_interval: Duration,
}

pub struct AgentService {
    agent_id: Uuid,
    api_client: Box<dyn NilccApiClient>,
    workload_repository: Box<dyn WorkloadRepository>,
    sync_interval: Duration,
}

impl AgentService {
    pub fn new(args: AgentServiceArgs) -> Self {
        let AgentServiceArgs { agent_id, api_client, workload_repository, sync_interval } = args;
        Self { agent_id, api_client, workload_repository, sync_interval }
    }

    /// Starts the agent service: registers the agent and begins periodic syncing.
    #[tracing::instrument("service.run", skip_all, fields(agent_id = self.agent_id.to_string()))]
    pub async fn run(mut self) -> Result<AgentServiceHandle> {
        info!("Starting run sequence");

        self.perform_registration().await.context("Initial agent registration failed")?;

        let handle = self.spawn_sync_executor();
        info!("AgentService is now operational. Registration complete and status reporter started");

        Ok(handle)
    }

    /// Gathers system information and registers the agent with the API server.
    async fn perform_registration(&mut self) -> Result<()> {
        info!("Attempting to register agent...");

        let details = gather_metal_instance_details().await?;
        let instance = MetalInstance { id: self.agent_id, details };
        info!("Metal instance: {instance:?}");

        self.api_client.register(instance).await.inspect_err(|e| {
            error!("Agent registration failed: {e:#}");
        })?;

        info!("Successfully registered with API");
        Ok(())
    }

    /// Spawns a Tokio task that periodically reports the agent's status.
    fn spawn_sync_executor(self) -> AgentServiceHandle {
        let (shutdown_tx, shutdown_rx) = watch::channel(());
        info!("Spawning periodic sync executor");

        tokio::spawn(async move { self.run_loop(shutdown_rx).await });
        AgentServiceHandle(shutdown_tx)
    }

    async fn run_loop(self, mut shutdown_rx: watch::Receiver<()>) {
        let mut interval = tokio::time::interval(self.sync_interval);

        info!("Sync task started. Will report every {:?}", self.sync_interval);
        loop {
            tokio::select! {
                biased; // prefer shutdown signal if available simultaneously
                _ = shutdown_rx.changed() => {
                    info!("Received shutdown signal. Exiting task.");
                    break;
                }
                _ = interval.tick() => {
                    self.run_once().await;
                }
            }
        }
        info!("Sync executor task for agent has finished.");
    }

    async fn run_once(&self) {
        let sync_request = SyncRequest {};
        let response = match self.api_client.sync(self.agent_id, sync_request).await {
            Ok(response) => response,
            Err(e) => {
                //TODO: Consider more robust error handling: e.g., retries with backoff.
                error!("Failed to sync {e:#}");
                return;
            }
        };
        if let Err(e) = self.process_sync_response(response).await {
            error!("Failed to process sync response: {e:#}");
        }
    }

    async fn process_sync_response(&self, response: SyncResponse) -> anyhow::Result<()> {
        let actions = self.compute_workload_actions(response.workloads).await?;
        if actions.is_empty() {
            info!("No actions need to be executed");
        }
        info!("Need to perform {} workload actions", actions.len());
        self.apply_actions(actions).await?;
        Ok(())
    }

    async fn compute_workload_actions(&self, expected: Vec<Workload>) -> anyhow::Result<Vec<WorkloadAction>> {
        let mut existing: HashMap<_, _> = self
            .workload_repository
            .list()
            .await
            .context("failed to find workloads")?
            .into_iter()
            .map(|w| (w.id, w))
            .collect();
        let mut actions = Vec::new();
        for workload in expected {
            let workload_id = workload.id;
            match existing.remove(&workload_id) {
                Some(existing) => {
                    // If it exists and is different, we need to update it
                    if existing != workload {
                        info!("Need to update workload {workload_id}");
                        actions.push(WorkloadAction::Update(workload));
                    }
                }
                None => {
                    // It doesn't exist, it needs to be started
                    info!("Need to start workload {workload_id}");
                    actions.push(WorkloadAction::Start(workload));
                }
            };
        }
        // Anything that's left we should remove
        for workload_id in existing.into_keys() {
            info!("Need to stop workload {workload_id}");
            actions.push(WorkloadAction::Stop(workload_id));
        }
        Ok(actions)
    }

    async fn apply_actions(&self, actions: Vec<WorkloadAction>) -> anyhow::Result<()> {
        for action in actions {
            // TODO start/stop vms based on actions.
            match action {
                WorkloadAction::Start(workload) => self.workload_repository.upsert(workload).await?,
                WorkloadAction::Update(workload) => self.workload_repository.upsert(workload).await?,
                WorkloadAction::Stop(id) => self.workload_repository.delete(id).await?,
            };
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq)]
enum WorkloadAction {
    Start(Workload),
    Update(Workload),
    Stop(Uuid),
}

impl WorkloadAction {
    #[cfg(test)]
    fn workload_id(&self) -> Uuid {
        use WorkloadAction::*;
        match self {
            Start(workload) | Update(workload) => workload.id,
            Stop(id) => *id,
        }
    }
}

#[must_use]
pub struct AgentServiceHandle(watch::Sender<()>);

impl Drop for AgentServiceHandle {
    fn drop(&mut self) {
        if self.0.send(()).is_err() {
            warn!("Task might have already exited");
        }
    }
}

// Gather system details for the agent's metal instance. Gpu for now is optional and details are supplied by the config.
pub async fn gather_metal_instance_details() -> Result<MetalInstanceDetails> {
    info!("Gathering metal instance details...");

    let mut sys = System::new_all();
    sys.refresh_all();

    let hostname = System::host_name().context("Failed to get hostname from sysinfo")?;
    let memory = sys.total_memory() / (1024 * 1024 * 1024);
    let disks = Disks::new_with_refreshed_list();
    let mut root_disk_bytes = 0;
    for disk in disks.list() {
        if disk.mount_point().as_os_str() == "/" {
            root_disk_bytes = disk.total_space();
        }
    }
    let disk = root_disk_bytes / (1024 * 1024 * 1024);
    let cpu = sys.cpus().len() as u32;
    let gpu_group = gpu::find_gpus().await?;

    let (gpu_model, gpu_count) =
        gpu_group.map(|group| (Some(group.model.clone()), Some(group.addresses.len() as u32))).unwrap_or_default();

    let details = MetalInstanceDetails {
        agent_version: get_agent_version().to_string(),
        hostname,
        memory,
        disk,
        cpu,
        gpu: gpu_count,
        gpu_model,
    };

    Ok(details)
}

#[cfg(test)]
mod tests {
    use mockall::predicate::eq;

    use super::*;
    use crate::{http_client::MockNilccApiClient, repositories::workload::MockWorkloadRepository};

    fn make_workload() -> Workload {
        Workload {
            id: Uuid::new_v4(),
            docker_compose: Default::default(),
            env_vars: Default::default(),
            service_to_expose: Default::default(),
            service_port_to_expose: Default::default(),
            memory: Default::default(),
            cpu: 1.try_into().unwrap(),
            disk: 1.try_into().unwrap(),
            gpu: Default::default(),
        }
    }

    struct ServiceBuilder {
        agent_id: Uuid,
        workload_repository: MockWorkloadRepository,
        api_client: MockNilccApiClient,
    }

    impl ServiceBuilder {
        fn build(self) -> AgentService {
            let Self { agent_id, workload_repository, api_client } = self;
            let args = AgentServiceArgs {
                agent_id,
                api_client: Box::new(api_client),
                workload_repository: Box::new(workload_repository),
                sync_interval: Duration::from_secs(10),
            };
            AgentService::new(args)
        }
    }

    impl Default for ServiceBuilder {
        fn default() -> Self {
            Self { agent_id: Uuid::new_v4(), workload_repository: Default::default(), api_client: Default::default() }
        }
    }

    #[tokio::test]
    async fn diff_workloads() {
        let mut builder = ServiceBuilder::default();
        let unmodified = make_workload();
        let mut modified = make_workload();
        let removed = make_workload();
        let existing = vec![unmodified.clone(), modified.clone(), removed.clone()];
        builder.workload_repository.expect_list().return_once(move || Ok(existing.clone()));

        modified.cpu = modified.cpu.checked_add(1).unwrap();

        let new = make_workload();
        let expected_workloads = vec![unmodified, modified.clone(), new.clone()];
        let service = builder.build();
        let mut actions =
            service.compute_workload_actions(expected_workloads.clone()).await.expect("failed to compute");
        let mut expected_actions =
            vec![WorkloadAction::Start(new), WorkloadAction::Stop(removed.id), WorkloadAction::Update(modified)];
        // Sort by workload id so we can compare them
        actions.sort_by_key(WorkloadAction::workload_id);
        expected_actions.sort_by_key(WorkloadAction::workload_id);

        assert_eq!(actions, expected_actions);
    }

    #[tokio::test]
    async fn apply_actions() {
        let mut builder = ServiceBuilder::default();
        let start_workload = make_workload();
        let update_workload = make_workload();
        let stop_id = Uuid::new_v4();
        let actions = vec![
            WorkloadAction::Start(start_workload.clone()),
            WorkloadAction::Update(update_workload.clone()),
            WorkloadAction::Stop(stop_id),
        ];
        builder.workload_repository.expect_delete().with(eq(stop_id)).return_once(move |_| Ok(()));
        builder.workload_repository.expect_upsert().with(eq(start_workload)).return_once(move |_| Ok(()));
        builder.workload_repository.expect_upsert().with(eq(update_workload)).return_once(move |_| Ok(()));

        let service = builder.build();
        service.apply_actions(actions).await.expect("failed to apply actions");
    }
}
