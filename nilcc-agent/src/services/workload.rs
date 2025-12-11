use crate::{
    heartbeat_verifier::{VerifierKey, VerifierKeys},
    repositories::{
        artifacts::ArtifactsRepositoryError,
        sqlite::{ProviderError, ProviderMode, RepositoryProvider},
        workload::{Workload, WorkloadHeartbeat, WorkloadRepositoryError},
    },
    resources::{GpuAddress, SystemResources},
    services::{
        proxy::{ProxiedVm, ProxyService},
        vm::{StartVmError, VmService},
    },
};
use anyhow::Context;
use async_trait::async_trait;
use nilcc_agent_models::workloads::create::CreateWorkloadRequest;
use std::{
    collections::{BTreeSet, HashMap},
    io,
    ops::Range,
    sync::Arc,
};
use strum::EnumDiscriminants;
use tokio::sync::Mutex;
use tracing::info;
use uuid::Uuid;

const TOTAL_PORTS: usize = 3;

#[cfg_attr(test, mockall::automock)]
#[async_trait]
pub trait WorkloadService: Send + Sync {
    async fn bootstrap(&self) -> anyhow::Result<()>;
    async fn create_workload(&self, request: CreateWorkloadRequest) -> Result<(), CreateWorkloadError>;
    async fn list_workloads(&self) -> Result<Vec<Workload>, WorkloadLookupError>;
    async fn delete_workload(&self, id: Uuid) -> Result<(), WorkloadLookupError>;
    async fn restart_workload(
        &self,
        id: Uuid,
        env_vars: Option<HashMap<String, String>>,
    ) -> Result<(), WorkloadLookupError>;
    async fn stop_workload(&self, id: Uuid) -> Result<(), WorkloadLookupError>;
    async fn start_workload(&self, id: Uuid) -> Result<(), WorkloadLookupError>;
    async fn cvm_agent_port(&self, workload_id: Uuid) -> Result<u16, WorkloadLookupError>;
}

#[derive(Debug, thiserror::Error)]
pub enum CreateWorkloadError {
    #[error("not enough {0} avalable")]
    InsufficientResources(&'static str),

    #[error("internal: {0}")]
    Internal(String),

    #[error("artifacts version is not installed")]
    ArtifactVersionMissing,

    #[error("workload already exists")]
    AlreadyExists,

    #[error("domain is already managed by another workload")]
    DomainExists,

    #[error("not enough verifier keys")]
    NotEnoughKeys,
}

impl From<ArtifactsRepositoryError> for CreateWorkloadError {
    fn from(e: ArtifactsRepositoryError) -> Self {
        Self::Internal(e.to_string())
    }
}

impl From<ProviderError> for CreateWorkloadError {
    fn from(e: ProviderError) -> Self {
        Self::Internal(e.to_string())
    }
}

impl From<StartVmError> for CreateWorkloadError {
    fn from(e: StartVmError) -> Self {
        Self::Internal(e.to_string())
    }
}

impl From<WorkloadRepositoryError> for CreateWorkloadError {
    fn from(e: WorkloadRepositoryError) -> Self {
        match e {
            WorkloadRepositoryError::DuplicateWorkload => Self::AlreadyExists,
            WorkloadRepositoryError::DuplicateDomain => Self::DomainExists,
            WorkloadRepositoryError::WorkloadNotFound | WorkloadRepositoryError::Database(_) => {
                Self::Internal(e.to_string())
            }
        }
    }
}

#[derive(Debug, thiserror::Error, EnumDiscriminants)]
pub enum WorkloadLookupError {
    #[error("workload not found")]
    WorkloadNotFound,

    #[error("database: {0}")]
    Database(WorkloadRepositoryError),

    #[error("internal: {0}")]
    Internal(String),
}

impl From<ProviderError> for WorkloadLookupError {
    fn from(e: ProviderError) -> Self {
        Self::Internal(e.to_string())
    }
}

impl From<WorkloadRepositoryError> for WorkloadLookupError {
    fn from(e: WorkloadRepositoryError) -> Self {
        match e {
            WorkloadRepositoryError::WorkloadNotFound => Self::WorkloadNotFound,
            e => Self::Database(e),
        }
    }
}

pub struct WorkloadServiceArgs {
    pub vm_service: Box<dyn VmService>,
    pub repository_provider: Arc<dyn RepositoryProvider>,
    pub proxy_service: Box<dyn ProxyService>,
    pub resources: SystemResources,
    pub open_ports: Range<u16>,
    pub verifier_keys: VerifierKeys,
}

struct AvailableResources {
    cpus: u32,
    gpus: Vec<GpuAddress>,
    memory_mb: u32,
    disk_space_gb: u32,
    ports: Vec<u16>,
}

#[derive(Debug, thiserror::Error)]
pub enum CreateServiceError {
    #[error("too many vCPUs allocated")]
    OvercommittedCpus,

    #[error("too much memory allocated")]
    OvercommittedMemory,

    #[error("too much disk space allocated")]
    OvercommittedDiskSpace,

    #[error("workload {0} uses GPU {1} but it is not part of the detected resources")]
    CommittedGpuMissing(Uuid, GpuAddress),

    #[error("allocated port {0} is out of the public range")]
    PortOutOfRange(u16),

    #[error("failed to create state directory: {0}")]
    StateDirectory(io::Error),

    #[error("database: {0}")]
    Database(#[from] WorkloadRepositoryError),

    #[error(transparent)]
    Provider(#[from] ProviderError),

    #[error("staring existing workload: {0}")]
    StartWorkload(#[from] StartVmError),
}

pub struct DefaultWorkloadService {
    repository_provider: Arc<dyn RepositoryProvider>,
    vm_service: Box<dyn VmService>,
    proxy_service: Box<dyn ProxyService>,
    resources: Mutex<AvailableResources>,
    verifier_keys: VerifierKeys,
}

impl DefaultWorkloadService {
    pub async fn new(args: WorkloadServiceArgs) -> Result<Self, CreateServiceError> {
        let WorkloadServiceArgs {
            vm_service,
            repository_provider,
            proxy_service,
            resources,
            open_ports,
            verifier_keys,
        } = args;

        let mut repo = repository_provider.workloads(Default::default()).await?;
        let workloads = repo.list().await?;
        let mut gpus: BTreeSet<_> = resources.gpus.iter().flat_map(|g| g.addresses.iter().cloned()).collect();
        let mut ports: BTreeSet<_> = open_ports.collect();
        let mut cpus = resources.available_cpus();
        let mut memory_mb = resources.available_memory_mb();
        let mut disk_space_gb = resources.available_disk_space_gb();
        for workload in workloads {
            let workload_id = workload.id;
            for gpu in &workload.gpus {
                if !gpus.remove(gpu) {
                    return Err(CreateServiceError::CommittedGpuMissing(workload_id, gpu.clone()));
                }
            }
            for port in workload.ports {
                if !ports.remove(&port) {
                    return Err(CreateServiceError::PortOutOfRange(port));
                }
            }
            cpus = cpus.checked_sub(workload.cpus).ok_or(CreateServiceError::OvercommittedCpus)?;
            memory_mb = memory_mb.checked_sub(workload.memory_mb).ok_or(CreateServiceError::OvercommittedMemory)?;
            disk_space_gb =
                disk_space_gb.checked_sub(workload.disk_space_gb).ok_or(CreateServiceError::OvercommittedDiskSpace)?;
        }
        let gpus: Vec<_> = gpus.into_iter().collect();
        let ports = ports.into_iter().collect();
        let gpu_count = gpus.len();
        info!(
            "Starting with available cpus = {cpus}, gpus = {gpu_count}, memory = {memory_mb}MB, disk = {disk_space_gb}GB"
        );
        let resources = AvailableResources { cpus, gpus, ports, memory_mb, disk_space_gb }.into();
        Ok(Self { vm_service, repository_provider, proxy_service, resources, verifier_keys })
    }

    fn build_workload(
        &self,
        request: CreateWorkloadRequest,
        resources: &AvailableResources,
        artifacts_version: String,
        heartbeat: Option<WorkloadHeartbeat>,
    ) -> Workload {
        let CreateWorkloadRequest {
            id,
            docker_compose,
            env_vars,
            files,
            docker_credentials,
            public_container_name,
            public_container_port,
            memory_mb,
            cpus,
            gpus,
            disk_space_gb,
            domain,
            ..
        } = request;

        let gpus = resources.gpus.iter().take(gpus as usize).cloned().collect();
        let ports: Vec<_> = resources.ports.iter().take(TOTAL_PORTS).copied().collect();
        let ports = ports.try_into().expect("not enough ports");
        Workload {
            id,
            docker_compose,
            artifacts_version,
            env_vars,
            files,
            docker_credentials,
            public_container_name,
            public_container_port,
            memory_mb,
            cpus,
            gpus,
            disk_space_gb,
            ports,
            domain,
            last_reported_event: None,
            enabled: true,
            heartbeat,
        }
    }

    fn workload_key(&self, workload: &Workload) -> anyhow::Result<Option<VerifierKey>> {
        let id = workload.id;
        let key = match &workload.heartbeat {
            Some(config) => {
                // Lookup the existing key if one is already set. This will only happen during the
                // transition period where this is still null for some workloads.
                let key = match &config.wallet_public_key {
                    Some(key) => {
                        info!("Looking up key {} for workload {id}", hex::encode(key));
                        self.verifier_keys.get(key)?
                    }
                    None => self.verifier_keys.next_key().context("Not enough verifier keys")?,
                };
                if config.wallet_public_key.is_some() {
                    info!("Using existing key {} for workload {id}", hex::encode(key.public_key()));
                } else {
                    info!("Assigning key {} to workload {id}", hex::encode(key.public_key()));
                }
                Some(key)
            }
            None => None,
        };
        Ok(key)
    }
}

#[async_trait]
impl WorkloadService for DefaultWorkloadService {
    async fn bootstrap(&self) -> anyhow::Result<()> {
        let mut repo = self.repository_provider.workloads(Default::default()).await?;
        let workloads = repo.list().await?;
        for workload in workloads {
            let id = workload.id;
            if workload.enabled {
                let key = self.workload_key(&workload)?;
                info!("Starting existing workload {id}");
                self.vm_service.create_vm(workload, key).await?;
            } else {
                info!("Not starting workload {id} because it's disabled");
                continue;
            }
        }
        Ok(())
    }

    async fn create_workload(&self, request: CreateWorkloadRequest) -> Result<(), CreateWorkloadError> {
        use CreateWorkloadError::*;
        let mut artifacts_repo = self.repository_provider.artifacts(Default::default()).await?;
        let artifacts = artifacts_repo.find(&request.artifacts_version).await?.ok_or(ArtifactVersionMissing)?;
        let mut resources = self.resources.lock().await;
        let cpus = request.cpus;
        let gpus = request.gpus as usize;
        let disk_space_gb = request.disk_space_gb;
        let memory_mb = request.memory_mb;
        if resources.cpus < cpus {
            return Err(InsufficientResources("CPUs"));
        }
        if resources.gpus.len() < gpus {
            return Err(InsufficientResources("GPUs"));
        }
        if resources.memory_mb < memory_mb {
            return Err(InsufficientResources("memory"));
        }
        if resources.disk_space_gb < disk_space_gb {
            return Err(InsufficientResources("disk space"));
        }
        if resources.ports.len() < TOTAL_PORTS {
            return Err(InsufficientResources("open ports"));
        }

        let (heartbeat, wallet_key) = match &request.heartbeat {
            Some(config) => {
                let wallet_key = self.verifier_keys.next_key().map_err(|_| NotEnoughKeys)?;
                let wallet_public_key = Some(wallet_key.public_key().to_vec());
                (
                    Some(WorkloadHeartbeat {
                        wallet_public_key,
                        measurement_hash_url: config.measurement_hash_url.clone(),
                    }),
                    Some(wallet_key),
                )
            }
            None => (None, None),
        };
        let workload = self.build_workload(request, &resources, artifacts.version.clone(), heartbeat);
        let id = workload.id;
        info!("Storing workload {id} in database");
        let mut repo = self.repository_provider.workloads(ProviderMode::Transactional).await?;
        repo.create(&workload).await?;

        info!(
            "Scheduling VM {id} using artifacts version {} and heartbeat key {:?}",
            artifacts.version,
            wallet_key.as_ref().map(|w| hex::encode(w.public_key()))
        );
        let proxied_vm = ProxiedVm::from(&workload);
        self.vm_service.create_vm(workload, wallet_key).await?;
        self.proxy_service.start_vm_proxy(proxied_vm).await;
        repo.commit().await?;

        resources.cpus -= cpus;
        resources.gpus.drain(0..gpus);
        resources.ports.drain(0..TOTAL_PORTS);
        resources.memory_mb -= memory_mb;
        resources.disk_space_gb -= disk_space_gb;
        Ok(())
    }

    async fn list_workloads(&self) -> Result<Vec<Workload>, WorkloadLookupError> {
        let mut repo = self.repository_provider.workloads(Default::default()).await?;
        Ok(repo.list().await?)
    }

    async fn delete_workload(&self, id: Uuid) -> Result<(), WorkloadLookupError> {
        // Make sure it exists first
        let mut repo = self.repository_provider.workloads(Default::default()).await?;
        let workload = repo.find(id).await?;

        info!("Deleting workload: {id}");
        repo.delete(id).await?;
        self.proxy_service.stop_vm_proxy(id).await;
        self.vm_service.delete_vm(id).await;

        let mut resources = self.resources.lock().await;
        resources.cpus += workload.cpus;
        resources.gpus.extend(workload.gpus);
        resources.memory_mb += workload.memory_mb;
        resources.disk_space_gb += workload.disk_space_gb;
        resources.ports.extend(workload.ports);
        Ok(())
    }

    async fn restart_workload(
        &self,
        id: Uuid,
        env_vars: Option<HashMap<String, String>>,
    ) -> Result<(), WorkloadLookupError> {
        // Make sure it exists first
        let mut repo = self.repository_provider.workloads(ProviderMode::Transactional).await?;
        let workload = repo.find(id).await?;
        if let Some(env_vars) = env_vars {
            repo.set_env_vars(id, env_vars).await.map_err(|e| WorkloadLookupError::Internal(e.to_string()))?;
        }
        if workload.enabled {
            info!("Restarting workload {id}");
            self.vm_service.restart_vm(id).await.map_err(|e| WorkloadLookupError::Internal(e.to_string()))?;
        } else {
            info!("Enabling workload {id}");
            let key = self.workload_key(&workload).map_err(|e| WorkloadLookupError::Internal(e.to_string()))?;
            self.vm_service.create_vm(workload, key).await.map_err(|e| WorkloadLookupError::Internal(e.to_string()))?;
            repo.set_enabled(id, true).await?;
        }
        repo.commit().await.map_err(|e| WorkloadLookupError::Internal(e.to_string()))?;
        Ok(())
    }

    async fn stop_workload(&self, id: Uuid) -> Result<(), WorkloadLookupError> {
        let mut repo = self.repository_provider.workloads(ProviderMode::Transactional).await?;
        let workload = repo.find(id).await?;
        if !workload.enabled {
            info!("Workload {id} is already disabled");
            return Ok(());
        }
        // The workload key is going back to the pool
        let mut heartbeats = workload.heartbeat;
        if let Some(config) = &mut heartbeats {
            config.wallet_public_key = None;
        }
        info!("Disabling workload {id}");
        repo.set_enabled(id, false).await?;
        repo.set_heartbeat(id, heartbeats).await?;
        repo.commit().await?;
        self.vm_service.delete_vm(id).await;
        Ok(())
    }

    async fn start_workload(&self, id: Uuid) -> Result<(), WorkloadLookupError> {
        let mut repo = self.repository_provider.workloads(ProviderMode::Transactional).await?;
        let mut workload = repo.find(id).await?;
        if workload.enabled {
            info!("Workload {id} is already enabled");
            return Ok(());
        }
        let key = self.verifier_keys.next_key().map_err(|e| WorkloadLookupError::Internal(e.to_string()))?;
        if let Some(config) = &mut workload.heartbeat {
            config.wallet_public_key = Some(key.public_key().to_vec());
        }
        info!("Starting workload {id} using wallet key {}", hex::encode(key.public_key()));
        repo.set_enabled(id, true).await?;
        repo.set_heartbeat(id, workload.heartbeat.clone()).await?;
        repo.commit().await?;
        self.vm_service
            .create_vm(workload, Some(key))
            .await
            .map_err(|e| WorkloadLookupError::Internal(e.to_string()))?;
        Ok(())
    }

    async fn cvm_agent_port(&self, workload_id: Uuid) -> Result<u16, WorkloadLookupError> {
        let mut repo = self.repository_provider.workloads(Default::default()).await?;
        let workload = repo.find(workload_id).await?;
        Ok(workload.cvm_agent_port())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        repositories::{
            artifacts::{Artifacts, MockArtifactsRepository, utils::make_artifacts_metadata},
            sqlite::MockRepositoryProvider,
            workload::MockWorkloadRepository,
        },
        resources::Gpus,
        services::{
            proxy::{MockProxyService, ProxiedVm},
            vm::MockVmService,
        },
    };
    use mockall::predicate::{always, eq};
    use nilcc_agent_models::workloads::create::CreateWorkloadHeartbeat;
    use rstest::rstest;
    use uuid::Uuid;

    struct Builder {
        vm_service: MockVmService,
        workloads_repository: MockWorkloadRepository,
        artifacts_repository: MockArtifactsRepository,
        proxy_service: MockProxyService,
        resources: SystemResources,
        open_ports: Range<u16>,
        existing_workloads: Vec<Workload>,
    }

    impl Builder {
        async fn build(self) -> DefaultWorkloadService {
            self.try_build().await.expect("failed to build")
        }

        async fn build_invalid(self) -> CreateServiceError {
            self.try_build().await.map(|_| ()).expect_err("build succeeded")
        }

        async fn try_build(self) -> Result<DefaultWorkloadService, CreateServiceError> {
            let Self {
                vm_service,
                workloads_repository,
                artifacts_repository,
                proxy_service,
                resources,
                open_ports,
                existing_workloads,
            } = self;

            let mut provider = MockRepositoryProvider::default();
            provider.expect_workloads().once().return_once(|_| {
                let mut repo = MockWorkloadRepository::default();
                repo.expect_list().return_once(move || Ok(existing_workloads));
                Ok(Box::new(repo))
            });
            provider.expect_workloads().return_once(move |_| Ok(Box::new(workloads_repository)));
            provider.expect_artifacts().return_once(move |_| Ok(Box::new(artifacts_repository)));

            let args = WorkloadServiceArgs {
                vm_service: Box::new(vm_service),
                repository_provider: Arc::new(provider),
                proxy_service: Box::new(proxy_service),
                resources,
                open_ports,
                verifier_keys: VerifierKeys::dummy(),
            };
            DefaultWorkloadService::new(args).await
        }
    }

    impl Default for Builder {
        fn default() -> Self {
            Self {
                vm_service: Default::default(),
                workloads_repository: Default::default(),
                artifacts_repository: Default::default(),
                proxy_service: Default::default(),
                resources: SystemResources {
                    hostname: "foo".into(),
                    memory_mb: 65536,
                    reserved_memory_mb: 2048,
                    disk_space_gb: 100,
                    reserved_disk_space_gb: 2,
                    cpus: 8,
                    reserved_cpus: 2,
                    gpus: None,
                },
                open_ports: 100..200,
                existing_workloads: Default::default(),
            }
        }
    }

    fn make_workload() -> Workload {
        Workload {
            id: Uuid::new_v4(),
            docker_compose: Default::default(),
            artifacts_version: "default".into(),
            env_vars: Default::default(),
            files: Default::default(),
            docker_credentials: Default::default(),
            public_container_name: Default::default(),
            public_container_port: Default::default(),
            memory_mb: Default::default(),
            cpus: 1.try_into().unwrap(),
            disk_space_gb: 1.try_into().unwrap(),
            gpus: Default::default(),
            ports: [150, 151, 152],
            domain: "example.com".into(),
            last_reported_event: None,
            enabled: true,
            heartbeat: None,
        }
    }

    #[rstest]
    #[case::cpu(
        Workload { cpus: 2.try_into().unwrap(), ..make_workload() },
        CreateServiceError::OvercommittedCpus
    )]
    #[case::memory(
        Workload { memory_mb: 2048.try_into().unwrap(), ..make_workload() },
        CreateServiceError::OvercommittedMemory
    )]
    #[case::disk_space(
        Workload { disk_space_gb: 2048.try_into().unwrap(), ..make_workload() },
        CreateServiceError::OvercommittedDiskSpace
    )]
    #[case::gpus(
        Workload { id: Uuid::nil(), gpus: vec!["addr1".into(), "addr2".into()], ..make_workload() },
        CreateServiceError::CommittedGpuMissing(Uuid::nil(), "addr2".into())
    )]
    #[case::http_port(
        Workload { ports: [50, 150, 151], ..make_workload() },
        CreateServiceError::PortOutOfRange(50)
    )]
    #[case::https_port(
        Workload { ports: [150, 50, 151], ..make_workload() },
        CreateServiceError::PortOutOfRange(50)
    )]
    #[case::cvm_port_port(
        Workload { ports: [150, 151, 50], ..make_workload() },
        CreateServiceError::PortOutOfRange(50)
    )]
    #[tokio::test]
    async fn startup_overcommitted(#[case] workload: Workload, #[case] error: CreateServiceError) {
        let mut builder = Builder::default();
        builder.resources.cpus = 2;
        builder.resources.reserved_cpus = 1;
        builder.resources.memory_mb = 2;
        builder.resources.reserved_memory_mb = 1;
        builder.resources.disk_space_gb = 2;
        builder.resources.reserved_disk_space_gb = 1;
        builder.resources.gpus = Some(Gpus::new("H100", &["addr1".into()]));
        builder.open_ports = 100..200;
        builder.existing_workloads = vec![workload];
        assert_eq!(builder.build_invalid().await.to_string(), error.to_string());
    }

    #[tokio::test]
    async fn tally_used_resources() {
        let mut builder = Builder::default();
        builder.resources.cpus = 4;
        builder.resources.reserved_cpus = 1;
        builder.resources.memory_mb = 8192;
        builder.resources.reserved_memory_mb = 2048;
        builder.resources.disk_space_gb = 100;
        builder.resources.reserved_disk_space_gb = 20;
        builder.resources.gpus = Some(Gpus::new("H100", &["addr1".into(), "addr2".into()]));
        builder.open_ports = 1000..2000;

        let workload = Workload {
            cpus: 1.try_into().unwrap(),
            memory_mb: 1024,
            disk_space_gb: 10.try_into().unwrap(),
            gpus: vec!["addr1".into()],
            ports: [1000, 1001, 1002],
            ..make_workload()
        };
        builder.existing_workloads = vec![workload.clone()];

        let service = builder.build().await;
        let resources = service.resources.lock().await;
        // 4 total, 1 reserved, 1 used
        assert_eq!(resources.cpus, 2);

        // 8 total, 2 reserved, 1 used
        assert_eq!(resources.memory_mb, 5120);

        // 100 total, 20 reserved, 10 used
        assert_eq!(resources.disk_space_gb, 70);

        // 2 total, 1 used
        assert_eq!(resources.gpus, vec!["addr2".into()]);
    }

    #[tokio::test]
    async fn create_success() {
        let request = CreateWorkloadRequest {
            id: Uuid::new_v4(),
            artifacts_version: "default".into(),
            docker_compose: "compose".into(),
            env_vars: Default::default(),
            files: Default::default(),
            docker_credentials: Default::default(),
            public_container_name: "api".into(),
            public_container_port: 80,
            memory_mb: 1024,
            cpus: 1.try_into().unwrap(),
            gpus: 1,
            disk_space_gb: 1.try_into().unwrap(),
            domain: "example.com".into(),
            heartbeat: Some(CreateWorkloadHeartbeat { measurement_hash_url: "url".into() }),
        };
        let expected_key = VerifierKeys::dummy().next_key().unwrap().public_key().to_vec();
        let workload = Workload {
            id: request.id,
            docker_compose: request.docker_compose.clone(),
            artifacts_version: "default".into(),
            env_vars: request.env_vars.clone(),
            files: request.files.clone(),
            docker_credentials: request.docker_credentials.clone(),
            public_container_name: request.public_container_name.clone(),
            public_container_port: request.public_container_port,
            memory_mb: request.memory_mb,
            cpus: request.cpus,
            gpus: vec!["addr1".into()],
            disk_space_gb: request.disk_space_gb,
            ports: [100, 101, 102],
            domain: request.domain.clone(),
            last_reported_event: None,
            enabled: true,
            heartbeat: Some(WorkloadHeartbeat {
                wallet_public_key: Some(expected_key),
                measurement_hash_url: "url".into(),
            }),
        };
        let mut builder = Builder::default();
        let id = workload.id;

        let expected_cpus = builder.resources.available_cpus() - request.cpus as u32;
        let expected_memory = builder.resources.available_memory_mb() - request.memory_mb;
        let expected_disk_space = builder.resources.available_disk_space_gb() - request.disk_space_gb;
        let metadata = make_artifacts_metadata();

        builder.open_ports = 100..200;
        builder.resources.gpus = Some(Gpus::new("H100", ["addr1".into()]));
        builder
            .artifacts_repository
            .expect_find()
            .with(eq("default"))
            .return_once(|_| Ok(Some(Artifacts { metadata, version: "default".into() })));
        builder.workloads_repository.expect_create().with(eq(workload.clone())).once().return_once(|_| Ok(()));
        builder.workloads_repository.expect_commit().once().return_once(|| Ok(()));
        builder.vm_service.expect_create_vm().with(eq(workload), always()).once().return_once(|_, _| Ok(()));
        builder
            .proxy_service
            .expect_start_vm_proxy()
            .with(eq(ProxiedVm { id, domain: "example.com".into(), http_port: 100, https_port: 101 }))
            .return_once(move |_| ());

        let service = builder.build().await;
        service.create_workload(request).await.expect("failed to create");

        // Make sure the allocated resources are successfully tracked.
        let resources = service.resources.lock().await;
        assert_eq!(resources.cpus, expected_cpus);
        assert_eq!(resources.memory_mb, expected_memory);
        assert_eq!(resources.disk_space_gb, expected_disk_space);
        assert_eq!(resources.gpus, vec![]);
    }
}
