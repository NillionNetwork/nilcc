use crate::{
    build_info::get_agent_version,
    data_schemas::{MetalInstance, MetalInstanceDetails, SyncRequest, SyncWorkload, SyncWorkloadStatus, Workload},
    http_client::NilccApiClient,
    repositories::workload::{WorkloadModel, WorkloadModelStatus, WorkloadRepository},
    resources::{GpuAddress, SystemResources},
    services::vm::VmService,
};
use anyhow::{anyhow, bail, Context, Result};
use metrics::{gauge, histogram};
use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
    time::{Duration, Instant},
};
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
    pub workload_repository: Arc<dyn WorkloadRepository>,

    /// The VM service.
    pub vm_service: Box<dyn VmService>,

    /// The interval to use for sync requests.
    pub sync_interval: Duration,

    /// The start port range for workloads.
    pub start_port_range: u16,

    /// The end port range for workloads.
    pub end_port_range: u16,

    /// The system resources.
    pub system_resources: SystemResources,
}

pub struct AgentService {
    agent_id: Uuid,
    api_client: Box<dyn NilccApiClient>,
    workload_repository: Arc<dyn WorkloadRepository>,
    vm_service: Box<dyn VmService>,
    sync_interval: Duration,
    start_port_range: u16,
    end_port_range: u16,
    system_resources: SystemResources,
}

impl AgentService {
    pub fn new(args: AgentServiceArgs) -> Self {
        let AgentServiceArgs {
            agent_id,
            api_client,
            workload_repository,
            vm_service,
            sync_interval,
            start_port_range,
            end_port_range,
            system_resources,
        } = args;
        Self {
            agent_id,
            api_client,
            workload_repository,
            vm_service,
            sync_interval,
            start_port_range,
            end_port_range,
            system_resources,
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

        let instance = MetalInstance {
            id: self.agent_id,
            details: MetalInstanceDetails {
                agent_version: get_agent_version().to_string(),
                hostname: self.system_resources.hostname.clone(),
                memory_gb: self.system_resources.memory_gb,
                os_reserved_memory_gb: self.system_resources.reserved_memory_gb,
                disk_space_gb: self.system_resources.disk_space_gb,
                os_reserved_disk_space_gb: self.system_resources.reserved_disk_space_gb,
                cpus: self.system_resources.cpus,
                os_reserved_cpus: self.system_resources.reserved_cpus,
                gpus: self.system_resources.gpus.as_ref().map(|g| g.addresses.len() as u32),
                gpu_model: self.system_resources.gpus.as_ref().map(|g| g.model.clone()),
            },
        };
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
        let existing_workloads = self.workload_repository.list().await.context("Failed to find workloads")?;
        let sync_request = SyncRequest {
            id: self.agent_id,
            workloads: existing_workloads
                .iter()
                .map(|w| SyncWorkload { id: w.id, status: SyncWorkloadStatus::Running })
                .collect(),
        };
        let now = Instant::now();
        let response = self.api_client.sync(self.agent_id, sync_request).await.context("Failed to sync")?;
        self.validate_resource_allocation(&response.workloads).context("Resource allocation validation failed")?;
        histogram!("api_sync_duration_seconds").record(now.elapsed());

        let actions = self.compute_workload_actions(existing_workloads, response.workloads).await?;
        if actions.is_empty() {
            info!("No actions need to be executed");
            return Ok(());
        }
        info!("Need to perform {} workload actions", actions.len());
        self.apply_actions(actions).await
    }

    async fn compute_workload_actions(
        &self,
        existing: Vec<WorkloadModel>,
        expected: Vec<Workload>,
    ) -> Result<Vec<WorkloadAction>> {
        let mut used_ports: HashSet<_> =
            existing.iter().flat_map(|w| [w.metal_http_port, w.metal_https_port]).collect();
        let mut used_gpus: HashSet<_> = existing.iter().flat_map(|w| w.gpus.iter().cloned()).collect();
        let mut existing: HashMap<_, _> = existing.into_iter().map(|w| (w.id, w)).collect();
        let mut actions = Vec::new();
        for workload in expected {
            let workload_id = workload.id;
            match existing.remove(&workload_id) {
                Some(existing) => {
                    // If it exists and is different, we need to update it
                    let (metal_http_port, metal_https_port) = (existing.metal_http_port, existing.metal_https_port);
                    if Workload::from(existing.clone()) != workload {
                        let gpus = match existing.gpus.len() != workload.gpus as usize {
                            true => {
                                info!("GPU count for workload {workload_id} changed, need to re-assign them");
                                for gpu in &existing.gpus {
                                    used_gpus.remove(gpu);
                                }
                                self.find_free_gpus(&mut used_gpus, workload.gpus as usize)?
                            }
                            false => existing.gpus,
                        };
                        info!("Need to update workload {workload_id}");
                        let new_workload_model =
                            WorkloadModel::from_schema(workload, metal_http_port, metal_https_port, gpus);
                        actions.push(WorkloadAction::Update(new_workload_model));
                    }
                }
                None => {
                    // It doesn't exist, it needs to be started
                    info!("Need to start workload {workload_id}");
                    let (metal_http_port, metal_https_port) = self.find_free_ports(&mut used_ports)?;
                    let gpus = self.find_free_gpus(&mut used_gpus, workload.gpus as usize)?;
                    let workload_model = WorkloadModel::from_schema(workload, metal_http_port, metal_https_port, gpus);
                    actions.push(WorkloadAction::Start(workload_model));
                }
            };
        }
        // Anything that's left we should remove
        for workload_id in existing.into_keys() {
            info!("Need to stop workload {workload_id}");
            actions.push(WorkloadAction::Stop(workload_id));
        }
        let total_ports = self.end_port_range.saturating_sub(self.start_port_range);
        gauge!("sni_proxy_ports_total", "status" => "total").set(total_ports);
        gauge!("sni_proxy_ports_total", "status" => "used").set(used_ports.len() as u16);
        Ok(actions)
    }

    async fn apply_actions(&self, actions: Vec<WorkloadAction>) -> anyhow::Result<()> {
        for action in actions {
            match action {
                WorkloadAction::Start(workload) | WorkloadAction::Update(workload) => {
                    // Store this since we've committed to running it
                    let committed_workload = WorkloadModel { status: WorkloadModelStatus::Pending, ..workload.clone() };
                    self.workload_repository.upsert(committed_workload).await?;
                    self.vm_service.sync_vm(workload).await;
                }
                WorkloadAction::Stop(id) => {
                    self.vm_service.stop_vm(id).await;
                }
            };
        }
        Ok(())
    }

    fn find_free_ports(&self, used_ports: &mut HashSet<u16>) -> Result<(u16, u16)> {
        let mut available_ports =
            (self.start_port_range..self.end_port_range).filter(|port| !used_ports.contains(port));
        let http = available_ports.next();
        let https = available_ports.next();
        if let (Some(http), Some(https)) = (http, https) {
            used_ports.extend([http, https]);
            Ok((http, https))
        } else {
            bail!("No free ports were found")
        }
    }

    fn find_free_gpus(&self, used_gpus: &mut HashSet<GpuAddress>, count: usize) -> Result<Vec<GpuAddress>> {
        if count == 0 {
            return Ok(Default::default());
        }
        let gpus = self.system_resources.gpus.as_ref().ok_or_else(|| anyhow!("No GPUs in metal instance"))?;
        let mut assigned_gpus = HashSet::new();
        for address in &gpus.addresses {
            if used_gpus.contains(address) {
                continue;
            }
            assigned_gpus.insert(address.clone());
            if assigned_gpus.len() == count {
                break;
            }
        }
        if assigned_gpus.len() != count {
            bail!("Not enough free GPUs available to be assigned to workload");
        }
        used_gpus.extend(assigned_gpus.iter().cloned());

        let mut assigned_gpus: Vec<_> = assigned_gpus.into_iter().collect();
        assigned_gpus.sort();
        Ok(assigned_gpus)
    }

    fn validate_resource_allocation(&self, workloads: &[Workload]) -> Result<()> {
        let mut cpus: u32 = 0;
        let mut gpus: usize = 0;
        let mut memory: u64 = 0;
        for workload in workloads {
            cpus = cpus.saturating_add(workload.cpus.get() as u32);
            memory = memory.saturating_add(workload.memory_gb as u64);
            gpus = gpus.saturating_add(workload.gpus as usize);
        }
        let available_cpus = self.system_resources.cpus;
        if cpus > available_cpus {
            bail!("Too many CPUs are allocated: have {available_cpus}, need {cpus}");
        }

        let available_gpus = self.system_resources.gpus.as_ref().map(|g| g.addresses.len()).unwrap_or_default();
        if gpus > available_gpus {
            bail!("Too many GPUs allocated: have {available_gpus}, need {gpus}");
        }

        let available_memory = self.system_resources.memory_gb;
        if memory > available_memory {
            bail!("Too much memory is allocated: have {available_memory}GB, need {memory}GB");
        }

        Ok(())
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        http_client::MockNilccApiClient, repositories::workload::MockWorkloadRepository, resources::Gpus,
        services::vm::MockVmService,
    };
    use mockall::predicate::eq;
    use rstest::rstest;

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
            status: Default::default(),
        }
    }

    struct ServiceBuilder {
        agent_id: Uuid,
        workload_repository: MockWorkloadRepository,
        api_client: MockNilccApiClient,
        vm_service: MockVmService,
        start_port_range: u16,
        end_port_range: u16,
        system_resources: SystemResources,
    }

    impl ServiceBuilder {
        fn build(self) -> AgentService {
            let Self {
                agent_id,
                workload_repository,
                vm_service,
                api_client,
                start_port_range,
                end_port_range,
                system_resources,
            } = self;
            let args = AgentServiceArgs {
                agent_id,
                api_client: Box::new(api_client),
                workload_repository: Arc::new(workload_repository),
                vm_service: Box::new(vm_service),
                sync_interval: Duration::from_secs(10),
                start_port_range,
                end_port_range,
                system_resources,
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
                start_port_range: 10000,
                end_port_range: 20000,
                system_resources: SystemResources {
                    hostname: "foo".into(),
                    memory_gb: 2,
                    reserved_memory_gb: 1,
                    disk_space_gb: 100,
                    reserved_disk_space_gb: 20,
                    cpus: 4,
                    reserved_cpus: 1,
                    gpus: None,
                },
            }
        }
    }

    #[tokio::test]
    async fn diff_workloads() {
        let unmodified = make_workload();
        let mut modified = make_workload();
        let removed = make_workload();
        let existing = vec![unmodified.clone(), modified.clone(), removed.clone()];

        modified.cpus = modified.cpus.checked_add(1).unwrap();

        let mut new = make_workload();
        let expected_workloads: Vec<Workload> =
            vec![unmodified, modified.clone(), new.clone()].into_iter().map(Workload::from).collect();
        let service = ServiceBuilder::default().build();
        let mut actions =
            service.compute_workload_actions(existing, expected_workloads.clone()).await.expect("failed to compute");
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
        builder
            .workload_repository
            .expect_upsert()
            .with(eq(start_workload.clone()))
            .once()
            .return_once(move |_| Ok(()));
        builder
            .workload_repository
            .expect_upsert()
            .with(eq(update_workload.clone()))
            .once()
            .return_once(move |_| Ok(()));
        builder.vm_service.expect_sync_vm().with(eq(start_workload)).once().return_once(|_| ());
        builder.vm_service.expect_sync_vm().with(eq(update_workload)).once().return_once(|_| ());
        builder.vm_service.expect_stop_vm().with(eq(stop_id)).once().return_once(|_| ());

        let service = builder.build();
        service.apply_actions(actions).await.expect("failed to apply actions");
    }

    #[test]
    fn free_ports() {
        let mut builder = ServiceBuilder::default();
        builder.start_port_range = 10;
        builder.end_port_range = 19;

        let mut used_ports = HashSet::from([11, 15]);
        let service = builder.build();
        assert_eq!(service.find_free_ports(&mut used_ports).unwrap(), (10, 12));
        assert_eq!(service.find_free_ports(&mut used_ports).unwrap(), (13, 14));
        assert_eq!(service.find_free_ports(&mut used_ports).unwrap(), (16, 17));

        service.find_free_ports(&mut used_ports).expect_err("found ports when we shouldn't have");
        assert_eq!(used_ports, (10..=17).collect());
    }

    #[test]
    fn free_gpus() {
        let mut builder = ServiceBuilder::default();
        builder.system_resources.gpus = Some(Gpus {
            model: "H100".into(),
            // We have addresses 'a' through 'f'
            addresses: ('a'..='f').map(|address| GpuAddress(address.to_string())).collect(),
        });

        let mut used_gpus = HashSet::from(["b".into()]);
        let service = builder.build();
        assert_eq!(service.find_free_gpus(&mut used_gpus, 2).unwrap(), &["a".into(), "c".into()]);
        assert_eq!(service.find_free_gpus(&mut used_gpus, 1).unwrap(), &["d".into()]);
        assert_eq!(service.find_free_gpus(&mut used_gpus, 1).unwrap(), &["e".into()]);

        // Can't allocate f and another one
        service.find_free_gpus(&mut used_gpus, 2).expect_err("should not have enough GPUs");

        // But we should still be able to allocate F alone
        assert_eq!(service.find_free_gpus(&mut used_gpus, 1).unwrap(), &["f".into()]);

        // Now we have nothing left
        service.find_free_gpus(&mut used_gpus, 1).expect_err("should not have enough GPUs");
    }

    #[test]
    fn valid_resource_allocation() {
        let workloads: Vec<Workload> = vec![
            WorkloadModel {
                cpus: 1.try_into().unwrap(),
                memory_mb: 1024,
                gpus: vec!["addr1".into()],
                ..make_workload()
            }
            .into(),
            WorkloadModel {
                cpus: 2.try_into().unwrap(),
                memory_mb: 2048,
                gpus: vec!["addr2".into(), "addr3".into()],
                ..make_workload()
            }
            .into(),
        ];
        let mut builder = ServiceBuilder::default();
        builder.system_resources.cpus = 3;
        builder.system_resources.memory_gb = 3;
        builder.system_resources.gpus =
            Some(Gpus { model: "H100".into(), addresses: vec!["addr1".into(), "addr2".into(), "addr3".into()] });

        let service = builder.build();
        service.validate_resource_allocation(&workloads).expect("resource allocation failed");
    }

    #[rstest]
    #[case::cpu(1, 3, 1)]
    #[case::memory(3, 2, 1)]
    #[case::gpus(3, 2, 2)]
    fn invalid_resource_allocation(#[case] cpus: u32, #[case] memory_gb: u64, #[case] gpu: u32) {
        let workloads: Vec<Workload> = vec![
            WorkloadModel { cpus: 1.try_into().unwrap(), memory_mb: 1024, gpus: vec!["a".into()], ..make_workload() }
                .into(),
            WorkloadModel { cpus: 2.try_into().unwrap(), memory_mb: 2048, ..make_workload() }.into(),
        ];
        let mut builder = ServiceBuilder::default();
        builder.system_resources.cpus = cpus;
        builder.system_resources.memory_gb = memory_gb;
        builder.system_resources.gpus =
            Some(Gpus { model: "H100".into(), addresses: (0..gpu).map(|i| GpuAddress(format!("{i}"))).collect() });

        let service = builder.build();
        service.validate_resource_allocation(&workloads).expect_err("resource allocation succeeded");
    }
}
