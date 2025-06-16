use crate::{
    build_info::get_agent_version,
    data_schemas::{MetalInstance, MetalInstanceDetails, SyncRequest, Workload},
    gpu,
    http_client::NilccApiClient,
    repositories::workload::{WorkloadModel, WorkloadRepository},
    services::{sni_proxy::SniProxyService, vm::VmService},
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

    /// The VM service.
    pub vm_service: Box<dyn VmService>,

    /// The SNI proxy service.
    pub sni_proxy_service: Box<dyn SniProxyService>,

    /// The interval to use for sync requests.
    pub sync_interval: Duration,

    /// The start port range for workloads.
    pub start_port_range: u16,

    /// The end port range for workloads.
    pub end_port_range: u16,
}

pub struct AgentService {
    agent_id: Uuid,
    api_client: Box<dyn NilccApiClient>,
    workload_repository: Box<dyn WorkloadRepository>,
    vm_service: Box<dyn VmService>,
    sni_proxy_service: Box<dyn SniProxyService>,
    sync_interval: Duration,
    start_port_range: u16,
    end_port_range: u16,
}

impl AgentService {
    pub fn new(args: AgentServiceArgs) -> Self {
        let AgentServiceArgs {
            agent_id,
            api_client,
            workload_repository,
            vm_service,
            sync_interval,
            sni_proxy_service,
            start_port_range,
            end_port_range,
        } = args;
        Self {
            agent_id,
            api_client,
            workload_repository,
            vm_service,
            sync_interval,
            sni_proxy_service,
            start_port_range,
            end_port_range,
        }
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
                    if let Err(e) = self.run_once().await {
                        error!("Failed to run: {e:#}");
                    }
                }
            }
        }
        info!("Sync executor task for agent has finished.");
    }

    async fn run_once(&self) -> Result<()> {
        let sync_request = SyncRequest {};
        let response = self.api_client.sync(self.agent_id, sync_request).await.context("Failed to sync")?;
        let actions = self.compute_workload_actions(response.workloads).await?;
        if actions.is_empty() {
            info!("No actions need to be executed");
            return Ok(());
        }
        info!("Need to perform {} workload actions", actions.len());
        self.apply_actions(actions).await?;
        self.update_sni_proxy().await
    }

    async fn compute_workload_actions(&self, expected: Vec<Workload>) -> Result<Vec<WorkloadAction>> {
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
                    let (metal_http_port, metal_https_port) = (existing.metal_http_port, existing.metal_https_port);
                    if Workload::from(existing) != workload {
                        info!("Need to update workload {workload_id}");
                        let new_workload_model =
                            WorkloadModel::from_schema(workload, metal_http_port, metal_https_port);
                        actions.push(WorkloadAction::Update(new_workload_model));
                    }
                }
                None => {
                    // It doesn't exist, it needs to be started
                    info!("Need to start workload {workload_id}");
                    let (metal_http_port, metal_https_port) = self.find_free_ports().await?;
                    let workload_model = WorkloadModel::from_schema(workload, metal_http_port, metal_https_port);
                    actions.push(WorkloadAction::Start(workload_model));
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
            match action {
                WorkloadAction::Start(workload) | WorkloadAction::Update(workload) => {
                    self.workload_repository.upsert(workload.clone()).await?;
                    self.vm_service.sync_vm(workload).await;
                }
                WorkloadAction::Stop(id) => {
                    self.workload_repository.delete(id).await?;
                    self.vm_service.stop_vm(id).await;
                }
            };
        }
        Ok(())
    }
    async fn find_free_ports(&self) -> Result<(u16, u16)> {
        let workloads = self.workload_repository.list().await?;
        let mut used_ports =
            workloads.iter().flat_map(|w| vec![w.metal_http_port, w.metal_https_port]).collect::<Vec<_>>();
        let mut first_port = 0;
        for port in self.start_port_range..self.end_port_range {
            if first_port == 0 && !used_ports.contains(&port) {
                used_ports.push(port);
                first_port = port;
            } else if !used_ports.contains(&port) {
                return Ok((first_port, port));
            }
        }
        Err(anyhow::anyhow!("No free ports were found."))
    }
    async fn update_sni_proxy(&self) -> Result<()> {
        info!("Updating SNI proxy configuration");
        let workloads = self.workload_repository.list().await?;

        self.sni_proxy_service.update_config(workloads).await.context("Failed to update SNI proxy configuration")
    }
}

#[derive(Clone, Debug, PartialEq)]
enum WorkloadAction {
    Start(WorkloadModel),
    Update(WorkloadModel),
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
    use super::*;
    use crate::{
        http_client::MockNilccApiClient,
        repositories::workload::MockWorkloadRepository,
        services::{sni_proxy::MockSniProxyService, vm::MockVmService},
    };
    use mockall::predicate::eq;

    fn make_workload() -> WorkloadModel {
        WorkloadModel {
            id: Uuid::new_v4(),
            docker_compose: Default::default(),
            environment_variables: Default::default(),
            public_container_name: Default::default(),
            public_container_port: Default::default(),
            memory_mb: Default::default(),
            cpus: 1.try_into().unwrap(),
            disk_gb: 1.try_into().unwrap(),
            gpus: Default::default(),
            metal_http_port: 0,
            metal_https_port: 0,
        }
    }

    struct ServiceBuilder {
        agent_id: Uuid,
        workload_repository: MockWorkloadRepository,
        api_client: MockNilccApiClient,
        vm_service: MockVmService,
    }

    impl ServiceBuilder {
        fn build(self) -> AgentService {
            let Self { agent_id, workload_repository, vm_service, api_client } = self;
            let args = AgentServiceArgs {
                agent_id,
                api_client: Box::new(api_client),
                workload_repository: Box::new(workload_repository),
                sni_proxy_service: Box::new(MockSniProxyService::new()),
                vm_service: Box::new(vm_service),
                sync_interval: Duration::from_secs(10),
                start_port_range: 10000,
                end_port_range: 20000,
            };
            AgentService::new(args)
        }
    }

    impl Default for ServiceBuilder {
        fn default() -> Self {
            Self {
                agent_id: Uuid::new_v4(),
                workload_repository: Default::default(),
                vm_service: Default::default(),
                api_client: Default::default(),
            }
        }
    }

    #[tokio::test]
    async fn diff_workloads() {
        let mut builder = ServiceBuilder::default();
        let unmodified = make_workload();
        let mut modified = make_workload();
        let removed = make_workload();
        let existing = vec![unmodified.clone(), modified.clone(), removed.clone()];
        builder.workload_repository.expect_list().returning(move || Ok(existing.clone()));

        modified.cpus = modified.cpus.checked_add(1).unwrap();

        let mut new = make_workload();
        let expected_workloads: Vec<Workload> =
            vec![unmodified, modified.clone(), new.clone()].into_iter().map(Workload::from).collect();
        let service = builder.build();
        let mut actions =
            service.compute_workload_actions(expected_workloads.clone()).await.expect("failed to compute");
        new.metal_http_port = 10000;
        new.metal_https_port = 10001;
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
        builder.workload_repository.expect_upsert().with(eq(start_workload.clone())).return_once(move |_| Ok(()));
        builder.workload_repository.expect_upsert().with(eq(update_workload.clone())).return_once(move |_| Ok(()));
        builder.vm_service.expect_sync_vm().with(eq(start_workload)).return_once(|_| ());
        builder.vm_service.expect_sync_vm().with(eq(update_workload)).return_once(|_| ());
        builder.vm_service.expect_stop_vm().with(eq(stop_id)).return_once(|_| ());

        let service = builder.build();
        service.apply_actions(actions).await.expect("failed to apply actions");
    }
}
