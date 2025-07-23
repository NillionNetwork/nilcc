use crate::{
    repositories::workload::{Workload, WorkloadRepository, WorkloadRepositoryError},
    resources::{GpuAddress, SystemResources},
    routes::workloads::create::CreateWorkloadRequest,
    services::{
        proxy::{ProxiedVm, ProxyService},
        vm::{StartVmError, VmService},
    },
};
use async_trait::async_trait;
use std::{collections::BTreeSet, io, ops::Range};
use strum::EnumDiscriminants;
use tokio::sync::Mutex;
use tracing::info;
use uuid::Uuid;

#[cfg_attr(test, mockall::automock)]
#[async_trait]
pub trait WorkloadService: Send + Sync {
    async fn create_workload(&self, request: CreateWorkloadRequest) -> Result<(), CreateWorkloadError>;
    async fn delete_workload(&self, id: Uuid) -> Result<(), WorkloadLookupError>;
    async fn restart_workload(&self, id: Uuid) -> Result<(), WorkloadLookupError>;
    async fn cvm_agent_port(&self, workload_id: Uuid) -> Result<u16, WorkloadLookupError>;
}

#[derive(Debug, thiserror::Error)]
pub enum CreateWorkloadError {
    #[error("not enough {0} avalable")]
    InsufficientResources(&'static str),

    #[error("internal: {0}")]
    Internal(String),

    #[error("workload already exists")]
    AlreadyExists,
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
    pub repository: Box<dyn WorkloadRepository>,
    pub proxy_service: Box<dyn ProxyService>,
    pub resources: SystemResources,
    pub open_ports: Range<u16>,
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

    #[error("staring existing workload: {0}")]
    StartWorkload(#[from] StartVmError),
}

pub struct DefaultWorkloadService {
    repository: Box<dyn WorkloadRepository>,
    vm_service: Box<dyn VmService>,
    proxy_service: Box<dyn ProxyService>,
    resources: Mutex<AvailableResources>,
}

impl DefaultWorkloadService {
    pub async fn new(args: WorkloadServiceArgs) -> Result<Self, CreateServiceError> {
        let WorkloadServiceArgs { vm_service, repository, proxy_service, resources, open_ports } = args;

        let workloads = repository.list().await?;
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
            info!("Starting existing workload {workload_id}");
            vm_service.start_vm(workload).await?;
        }
        let gpus = gpus.into_iter().collect();
        let ports = ports.into_iter().collect();
        let resources = AvailableResources { cpus, gpus, ports, memory_mb, disk_space_gb }.into();
        Ok(Self { vm_service, repository, proxy_service, resources })
    }

    fn build_workload(&self, request: CreateWorkloadRequest, resources: &AvailableResources) -> Workload {
        let CreateWorkloadRequest {
            id,
            docker_compose,
            env_vars,
            files,
            public_container_name,
            public_container_port,
            memory_mb,
            cpus,
            gpus,
            disk_space_gb,
            domain,
        } = request;
        let gpus = resources.gpus.iter().take(gpus as usize).cloned().collect();
        let ports: Vec<_> = resources.ports.iter().take(3).copied().collect();
        let ports = ports.try_into().expect("not enough ports");

        Workload {
            id,
            docker_compose,
            env_vars,
            files,
            public_container_name,
            public_container_port,
            memory_mb,
            cpus,
            gpus,
            disk_space_gb,
            ports,
            domain,
        }
    }
}

#[async_trait]
impl WorkloadService for DefaultWorkloadService {
    async fn create_workload(&self, request: CreateWorkloadRequest) -> Result<(), CreateWorkloadError> {
        let mut resources = self.resources.lock().await;
        let cpus = request.cpus;
        let gpus = request.gpus as usize;
        let disk_space_gb = request.disk_space_gb;
        let memory_mb = request.memory_mb;
        if resources.cpus < cpus {
            return Err(CreateWorkloadError::InsufficientResources("CPUs"));
        }
        if resources.gpus.len() < gpus {
            return Err(CreateWorkloadError::InsufficientResources("GPUs"));
        }
        if resources.memory_mb < memory_mb {
            return Err(CreateWorkloadError::InsufficientResources("memory"));
        }
        if resources.disk_space_gb < disk_space_gb {
            return Err(CreateWorkloadError::InsufficientResources("disk space"));
        }
        if resources.ports.len() < 2 {
            return Err(CreateWorkloadError::InsufficientResources("open ports"));
        }
        let workload = self.build_workload(request, &resources);
        let id = workload.id;
        info!("Storing workload {id} in database");
        self.repository.create(workload.clone()).await?;

        info!("Scheduling VM {id}");
        let proxied_vm = ProxiedVm::from(&workload);
        self.vm_service.start_vm(workload).await?;
        self.proxy_service.start_vm_proxy(proxied_vm).await;

        resources.cpus -= cpus;
        resources.gpus.drain(0..gpus);
        resources.ports.drain(0..2);
        resources.memory_mb -= memory_mb;
        resources.disk_space_gb -= disk_space_gb;
        Ok(())
    }

    async fn delete_workload(&self, id: Uuid) -> Result<(), WorkloadLookupError> {
        // Make sure it exists first
        self.repository.find(id).await?;

        info!("Deleting workload: {id}");
        self.repository.delete(id).await?;
        self.proxy_service.stop_vm_proxy(id).await;
        self.vm_service.delete_vm(id).await;
        Ok(())
    }

    async fn restart_workload(&self, id: Uuid) -> Result<(), WorkloadLookupError> {
        // Make sure it exists first
        self.repository.find(id).await?;
        self.vm_service.restart_vm(id).await.map_err(|e| WorkloadLookupError::Internal(e.to_string()))?;
        Ok(())
    }

    async fn cvm_agent_port(&self, workload_id: Uuid) -> Result<u16, WorkloadLookupError> {
        let workload = self.repository.find(workload_id).await?;
        Ok(workload.cvm_agent_port())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        repositories::workload::MockWorkloadRepository,
        resources::Gpus,
        services::{
            proxy::{MockProxyService, ProxiedVm},
            vm::MockVmService,
        },
    };
    use mockall::predicate::eq;
    use rstest::rstest;
    use uuid::Uuid;

    struct Builder {
        vm_service: MockVmService,
        repository: MockWorkloadRepository,
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
            let Self { vm_service, mut repository, proxy_service, resources, open_ports, existing_workloads } = self;
            repository.expect_list().return_once(move || Ok(existing_workloads));
            let args = WorkloadServiceArgs {
                vm_service: Box::new(vm_service),
                repository: Box::new(repository),
                proxy_service: Box::new(proxy_service),
                resources,
                open_ports,
            };
            DefaultWorkloadService::new(args).await
        }
    }

    impl Default for Builder {
        fn default() -> Self {
            Self {
                vm_service: Default::default(),
                repository: Default::default(),
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
            env_vars: Default::default(),
            files: Default::default(),
            public_container_name: Default::default(),
            public_container_port: Default::default(),
            memory_mb: Default::default(),
            cpus: 1.try_into().unwrap(),
            disk_space_gb: 1.try_into().unwrap(),
            gpus: Default::default(),
            ports: [150, 151, 152],
            domain: "example.com".into(),
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
        // We should start this VM as part of the startup process
        builder.vm_service.expect_start_vm().with(eq(workload)).once().return_once(move |_| Ok(()));

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
            docker_compose: "compose".into(),
            env_vars: Default::default(),
            files: Default::default(),
            public_container_name: "api".into(),
            public_container_port: 80,
            memory_mb: 1024,
            cpus: 1.try_into().unwrap(),
            gpus: 1,
            disk_space_gb: 1.try_into().unwrap(),
            domain: "example.com".into(),
        };
        let workload = Workload {
            id: request.id,
            docker_compose: request.docker_compose.clone(),
            env_vars: request.env_vars.clone(),
            files: request.files.clone(),
            public_container_name: request.public_container_name.clone(),
            public_container_port: request.public_container_port,
            memory_mb: request.memory_mb,
            cpus: request.cpus,
            gpus: vec!["addr1".into()],
            disk_space_gb: request.disk_space_gb,
            ports: [100, 101, 102],
            domain: request.domain.clone(),
        };
        let mut builder = Builder::default();
        let id = workload.id;

        let expected_cpus = builder.resources.available_cpus() - request.cpus as u32;
        let expected_memory = builder.resources.available_memory_mb() - request.memory_mb;
        let expected_disk_space = builder.resources.available_disk_space_gb() - request.disk_space_gb;

        builder.open_ports = 100..200;
        builder.resources.gpus = Some(Gpus::new("H100", ["addr1".into()]));
        builder.repository.expect_create().with(eq(workload.clone())).once().return_once(|_| Ok(()));
        builder.vm_service.expect_start_vm().with(eq(workload)).once().return_once(|_| Ok(()));
        builder
            .proxy_service
            .expect_start_vm_proxy()
            .with(eq(ProxiedVm { id, http_port: 100, https_port: 101 }))
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
