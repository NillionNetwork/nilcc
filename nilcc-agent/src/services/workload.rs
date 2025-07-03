use crate::{
    clients::qemu::{HardDiskFormat, HardDiskSpec, VmSpec},
    config::{CvmConfig, CvmFiles},
    iso::{ApplicationMetadata, ContainerMetadata, EnvironmentVariable, IsoSpec},
    repositories::workload::{Workload, WorkloadRepository, WorkloadRepositoryError},
    resources::{GpuAddress, SystemResources},
    routes::workloads::create::CreateWorkloadRequest,
    services::{
        disk::{DiskService, DockerComposeHash},
        proxy::ProxyService,
    },
    workers::scheduler::VmSchedulerHandle,
};
use async_trait::async_trait;
use std::{collections::BTreeSet, fmt, io, ops::Range, path::PathBuf};
use strum::EnumDiscriminants;
use tokio::{fs, sync::Mutex};
use tracing::info;
use uuid::Uuid;

#[cfg_attr(test, mockall::automock)]
#[async_trait]
pub trait WorkloadService: Send + Sync {
    async fn create_workload(&self, request: CreateWorkloadRequest) -> Result<(), CreateWorkloadError>;
    async fn delete_workload(&self, id: Uuid) -> Result<(), DeleteWorkloadError>;
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
pub enum DeleteWorkloadError {
    #[error("workload not found")]
    WorkloadNotFound,

    #[error("database: {0}")]
    Database(WorkloadRepositoryError),
}

impl From<WorkloadRepositoryError> for DeleteWorkloadError {
    fn from(e: WorkloadRepositoryError) -> Self {
        match e {
            WorkloadRepositoryError::WorkloadNotFound => Self::WorkloadNotFound,
            e => Self::Database(e),
        }
    }
}

pub struct WorkloadServiceArgs {
    pub state_path: PathBuf,
    pub disk_service: Box<dyn DiskService>,
    pub scheduler: Box<dyn VmSchedulerHandle>,
    pub repository: Box<dyn WorkloadRepository>,
    pub proxy_service: Box<dyn ProxyService>,
    pub resources: SystemResources,
    pub open_ports: Range<u16>,
    pub cvm_config: CvmConfig,
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
}

pub struct DefaultWorkloadService {
    state_path: PathBuf,
    repository: Box<dyn WorkloadRepository>,
    scheduler: Box<dyn VmSchedulerHandle>,
    disk_service: Box<dyn DiskService>,
    proxy_service: Box<dyn ProxyService>,
    resources: Mutex<AvailableResources>,
    cvm_config: CvmConfig,
}

impl DefaultWorkloadService {
    pub async fn new(args: WorkloadServiceArgs) -> Result<Self, CreateServiceError> {
        let WorkloadServiceArgs {
            state_path,
            disk_service,
            scheduler,
            repository,
            proxy_service,
            resources,
            open_ports,
            cvm_config,
        } = args;
        fs::create_dir_all(&state_path).await.map_err(CreateServiceError::StateDirectory)?;

        let workloads = repository.list().await?;
        let mut gpus: BTreeSet<_> = resources.gpus.iter().flat_map(|g| g.addresses.iter().cloned()).collect();
        let mut ports: BTreeSet<_> = open_ports.collect();
        let mut cpus = resources.available_cpus();
        let mut memory_mb = resources.available_memory_mb();
        let mut disk_space_gb = resources.available_disk_space_gb();
        for workload in workloads {
            let workload_id = workload.id;
            for gpu in workload.gpus {
                if !gpus.remove(&gpu) {
                    return Err(CreateServiceError::CommittedGpuMissing(workload_id, gpu));
                }
            }
            for port in [workload.proxy_http_port, workload.proxy_https_port] {
                if !ports.remove(&port) {
                    return Err(CreateServiceError::PortOutOfRange(port));
                }
            }
            cpus = cpus.checked_sub(workload.cpus).ok_or(CreateServiceError::OvercommittedCpus)?;
            memory_mb = memory_mb.checked_sub(workload.memory_mb).ok_or(CreateServiceError::OvercommittedMemory)?;
            disk_space_gb =
                disk_space_gb.checked_sub(workload.disk_space_gb).ok_or(CreateServiceError::OvercommittedDiskSpace)?;
        }
        let gpus = gpus.into_iter().collect();
        let ports = ports.into_iter().collect();
        let resources = AvailableResources { cpus, gpus, ports, memory_mb, disk_space_gb }.into();
        Ok(Self { state_path, scheduler, disk_service, repository, proxy_service, resources, cvm_config })
    }

    fn build_workload(&self, request: CreateWorkloadRequest, resources: &AvailableResources) -> Workload {
        let CreateWorkloadRequest {
            id,
            docker_compose,
            env_vars,
            public_container_name,
            public_container_port,
            memory_mb,
            cpus,
            gpus,
            disk_space_gb,
        } = request;
        let gpus = resources.gpus.iter().take(gpus as usize).cloned().collect();
        let ports: Vec<u16> = resources.ports.iter().take(2).copied().collect();

        Workload {
            id,
            docker_compose,
            env_vars,
            public_container_name,
            public_container_port,
            memory_mb,
            cpus,
            gpus,
            disk_space_gb,
            proxy_http_port: ports[0],
            proxy_https_port: ports[1],
        }
    }

    fn create_vm_spec(
        &self,
        workload: &Workload,
        iso_path: PathBuf,
        state_disk_path: PathBuf,
        docker_compose_hash: DockerComposeHash,
    ) -> VmSpec {
        let CvmFiles { base_disk, kernel, verity_root_hash, verity_disk } =
            if workload.gpus.is_empty() { &self.cvm_config.cpu } else { &self.cvm_config.gpu };
        let kernel_args =
            KernelArgs { filesystem_root_hash: verity_root_hash, docker_compose_hash: &docker_compose_hash.0 };
        VmSpec {
            cpu: workload.cpus,
            ram_mib: workload.memory_mb,
            hard_disks: vec![
                HardDiskSpec { path: base_disk.clone(), format: HardDiskFormat::Qcow2 },
                HardDiskSpec { path: verity_disk.clone(), format: HardDiskFormat::Raw },
                HardDiskSpec { path: state_disk_path, format: HardDiskFormat::Raw },
            ],
            cdrom_iso_path: Some(iso_path),
            gpus: workload.gpus.clone(),
            port_forwarding: vec![(workload.proxy_http_port, 80), (workload.proxy_https_port, 443)],
            bios_path: Some(self.cvm_config.bios.clone()),
            initrd_path: Some(self.cvm_config.initrd.clone()),
            kernel_path: Some(kernel.clone()),
            kernel_args: Some(kernel_args.to_string()),
            display_gtk: false,
            enable_cvm: true,
        }
    }

    async fn create_state_disk(&self, workload: &Workload) -> Result<PathBuf, CreateWorkloadError> {
        let disk_name = format!("{}.raw", workload.id);
        let disk_path = self.state_path.join(disk_name);
        self.disk_service
            .create_disk(&disk_path, HardDiskFormat::Raw, workload.disk_space_gb)
            .await
            .map_err(|e| CreateWorkloadError::Internal(format!("failed to create state disk: {e}")))?;
        Ok(disk_path)
    }

    async fn create_application_iso(
        &self,
        workload: &Workload,
    ) -> Result<(PathBuf, DockerComposeHash), CreateWorkloadError> {
        let iso_name = format!("{}.iso", workload.id);
        let iso_path = self.state_path.join(iso_name);
        let environment_variables =
            workload.env_vars.iter().map(|(name, value)| EnvironmentVariable::new(name, value)).collect();
        let spec = IsoSpec {
            docker_compose_yaml: workload.docker_compose.clone(),
            metadata: ApplicationMetadata {
                hostname: "UNKNOWN".into(),
                api: ContainerMetadata {
                    container: workload.public_container_name.clone(),
                    port: workload.public_container_port,
                },
            },
            environment_variables,
        };
        let docker_compose_hash = self
            .disk_service
            .create_application_iso(&iso_path, spec)
            .await
            .map_err(|e| CreateWorkloadError::Internal(format!("failed to create ISO: {e}")))?;
        Ok((iso_path, docker_compose_hash))
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
        // TODO: this should be a create, change once everything is migrated to new api
        self.repository.create(workload.clone()).await?;

        info!("Creating disks for VM {id}");
        let (iso_path, docker_compose_hash) = self.create_application_iso(&workload).await?;
        let state_disk = self.create_state_disk(&workload).await?;
        let spec = self.create_vm_spec(&workload, iso_path, state_disk, docker_compose_hash);

        info!("Scheduling VM {id}");
        let socket_path = self.state_path.join(format!("{}.sock", workload.id));
        self.scheduler.start_vm(workload.id, spec, socket_path).await;
        self.proxy_service.start_vm_proxy((&workload).into()).await;

        resources.cpus -= cpus;
        resources.gpus.drain(0..gpus);
        resources.ports.drain(0..2);
        resources.memory_mb -= memory_mb;
        resources.disk_space_gb -= disk_space_gb;
        Ok(())
    }

    async fn delete_workload(&self, id: Uuid) -> Result<(), DeleteWorkloadError> {
        // Make sure it exists first
        self.repository.find(id).await?;

        info!("Deleting workload: {id}");
        self.repository.delete(id).await?;
        self.proxy_service.stop_vm_proxy(id).await;
        self.scheduler.stop_vm(id).await;
        Ok(())
    }
}

struct KernelArgs<'a> {
    docker_compose_hash: &'a str,
    filesystem_root_hash: &'a str,
}

impl fmt::Display for KernelArgs<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let Self { docker_compose_hash, filesystem_root_hash } = self;
        write!(f, "panic=-1 root=/dev/sda2 verity_disk=/dev/sdb verity_roothash={filesystem_root_hash} state_disk=/dev/sdc docker_compose_disk=/dev/sr0 docker_compose_hash={docker_compose_hash}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        repositories::workload::MockWorkloadRepository,
        resources::Gpus,
        services::{
            disk::MockDiskService,
            proxy::{MockProxyService, ProxiedVm},
        },
        workers::scheduler::MockVmSchedulerHandle,
    };
    use mockall::predicate::eq;
    use rstest::rstest;
    use uuid::Uuid;

    struct Builder {
        scheduler: MockVmSchedulerHandle,
        repository: MockWorkloadRepository,
        disk_service: MockDiskService,
        proxy_service: MockProxyService,
        resources: SystemResources,
        open_ports: Range<u16>,
        existing_workloads: Vec<Workload>,
        cvm_config: CvmConfig,
        state_path: PathBuf,
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
                scheduler,
                mut repository,
                disk_service,
                proxy_service,
                resources,
                open_ports,
                existing_workloads,
                cvm_config,
                state_path,
            } = self;
            repository.expect_list().return_once(move || Ok(existing_workloads));
            let args = WorkloadServiceArgs {
                scheduler: Box::new(scheduler),
                repository: Box::new(repository),
                disk_service: Box::new(disk_service),
                proxy_service: Box::new(proxy_service),
                resources,
                open_ports,
                cvm_config,
                state_path,
            };
            DefaultWorkloadService::new(args).await
        }
    }

    impl Default for Builder {
        fn default() -> Self {
            Self {
                scheduler: Default::default(),
                repository: Default::default(),
                disk_service: Default::default(),
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
                cvm_config: CvmConfig {
                    initrd: "/initrd".into(),
                    bios: "/bios".into(),
                    cpu: CvmFiles {
                        base_disk: "/cpu/base-disk".into(),
                        kernel: "/cpu/kernel".into(),
                        verity_disk: "/cpu/verity-disk".into(),
                        verity_root_hash: "cpu-root-hash".into(),
                    },
                    gpu: CvmFiles {
                        base_disk: "/gpu/base-disk".into(),
                        kernel: "/gpu/kernel".into(),
                        verity_disk: "/gpu/verity-disk".into(),
                        verity_root_hash: "gpu-root-hash".into(),
                    },
                },
                state_path: PathBuf::from("/tmp"),
            }
        }
    }

    fn make_workload() -> Workload {
        Workload {
            id: Uuid::new_v4(),
            docker_compose: Default::default(),
            env_vars: Default::default(),
            public_container_name: Default::default(),
            public_container_port: Default::default(),
            memory_mb: Default::default(),
            cpus: 1.try_into().unwrap(),
            disk_space_gb: 1.try_into().unwrap(),
            gpus: Default::default(),
            proxy_http_port: 150,
            proxy_https_port: 151,
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
        Workload { proxy_http_port: 50, ..make_workload() },
        CreateServiceError::PortOutOfRange(50)
    )]
    #[case::https_port(
        Workload { proxy_https_port: 50, ..make_workload() },
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
            proxy_http_port: 1000,
            proxy_https_port: 1001,
            ..make_workload()
        };
        builder.existing_workloads = vec![workload];

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
            public_container_name: "api".into(),
            public_container_port: 80,
            memory_mb: 1024,
            cpus: 1.try_into().unwrap(),
            gpus: 1,
            disk_space_gb: 1.try_into().unwrap(),
        };
        let workload = Workload {
            id: request.id,
            docker_compose: request.docker_compose.clone(),
            env_vars: request.env_vars.clone(),
            public_container_name: request.public_container_name.clone(),
            public_container_port: request.public_container_port,
            memory_mb: request.memory_mb,
            cpus: request.cpus,
            gpus: vec!["addr1".into()],
            disk_space_gb: request.disk_space_gb,
            proxy_http_port: 100,
            proxy_https_port: 101,
        };
        let mut builder = Builder::default();
        let docker_compose_hash = "deadbeef";
        let id = workload.id;
        let base_path = builder.state_path.display();
        let iso_path = PathBuf::from(format!("{base_path}/{id}.iso"));
        let state_disk_path = PathBuf::from(format!("{base_path}/{id}.raw"));
        let socket_path = PathBuf::from(format!("{base_path}/{id}.sock"));
        let kernel_args = KernelArgs {
            docker_compose_hash: docker_compose_hash,
            filesystem_root_hash: &builder.cvm_config.gpu.verity_root_hash,
        }
        .to_string();
        let spec = VmSpec {
            cpu: 1,
            ram_mib: 1024,
            hard_disks: vec![
                HardDiskSpec { path: "/gpu/base-disk".into(), format: HardDiskFormat::Qcow2 },
                HardDiskSpec { path: "/gpu/verity-disk".into(), format: HardDiskFormat::Raw },
                HardDiskSpec { path: state_disk_path.clone(), format: HardDiskFormat::Raw },
            ],
            cdrom_iso_path: Some(iso_path.clone()),
            gpus: vec!["addr1".into()],
            port_forwarding: vec![(workload.proxy_http_port, 80), (workload.proxy_https_port, 443)],
            bios_path: Some("/bios".into()),
            initrd_path: Some("/initrd".into()),
            kernel_path: Some("/gpu/kernel".into()),
            kernel_args: Some(kernel_args),
            display_gtk: false,
            enable_cvm: true,
        };

        let expected_cpus = builder.resources.available_cpus() - request.cpus as u32;
        let expected_memory = builder.resources.available_memory_mb() - request.memory_mb;
        let expected_disk_space = builder.resources.available_disk_space_gb() - request.disk_space_gb;

        builder.open_ports = 100..200;
        builder.resources.gpus = Some(Gpus::new("H100", ["addr1".into()]));
        builder.repository.expect_create().with(eq(workload.clone())).once().return_once(|_| Ok(()));
        builder
            .disk_service
            .expect_create_application_iso()
            .return_once(move |_, _| Ok(DockerComposeHash(docker_compose_hash.into())));
        builder
            .disk_service
            .expect_create_disk()
            .with(eq(state_disk_path), eq(HardDiskFormat::Raw), eq(1))
            .return_once(move |_, _, _| Ok(()));
        builder
            .scheduler
            .expect_start_vm()
            .with(eq(workload.id), eq(spec), eq(socket_path))
            .once()
            .return_once(|_, _, _| ());
        builder
            .proxy_service
            .expect_start_vm_proxy()
            .with(eq(ProxiedVm { id: workload.id, http_port: 100, https_port: 101 }))
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
