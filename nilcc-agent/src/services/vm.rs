use crate::{
    clients::{
        nilcc_api::NilccApiClient,
        qemu::{HardDiskFormat, HardDiskSpec, VmClient, VmSpec},
    },
    config::{CvmConfig, CvmFiles},
    repositories::workload::Workload,
    services::disk::DiskService,
    services::disk::{ApplicationMetadata, ContainerMetadata, EnvironmentVariable, IsoSpec},
    workers::vm::{VmWorker, VmWorkerHandle},
};
use anyhow::Context;
use async_trait::async_trait;
use sha2::{Digest, Sha256};
use std::{collections::HashMap, fmt, path::PathBuf, sync::Arc};
use tokio::{fs, sync::Mutex};
use tracing::{error, info};
use uuid::Uuid;

#[cfg_attr(test, mockall::automock)]
#[async_trait]
pub trait VmService: Send + Sync {
    async fn start_vm(&self, workload: Workload) -> Result<(), StartVmError>;
    async fn stop_vm(&self, id: Uuid);
}

pub struct VmServiceArgs {
    pub state_path: PathBuf,
    pub vm_client: Arc<dyn VmClient>,
    pub nilcc_api_client: Arc<dyn NilccApiClient>,
    pub disk_service: Box<dyn DiskService>,
    pub cvm_config: CvmConfig,
}

pub struct DefaultVmService {
    vm_client: Arc<dyn VmClient>,
    nilcc_api_client: Arc<dyn NilccApiClient>,
    disk_service: Box<dyn DiskService>,
    workers: Mutex<HashMap<Uuid, VmWorkerHandle>>,
    state_path: PathBuf,
    cvm_config: CvmConfig,
}

impl DefaultVmService {
    pub async fn new(args: VmServiceArgs) -> anyhow::Result<Self> {
        let VmServiceArgs { state_path, vm_client, nilcc_api_client, disk_service, cvm_config } = args;
        fs::create_dir_all(&state_path).await.context("Creating state directory")?;
        Ok(Self { vm_client, nilcc_api_client, disk_service, workers: Default::default(), state_path, cvm_config })
    }

    fn create_vm_spec(
        &self,
        workload: &Workload,
        iso_path: PathBuf,
        state_disk_path: PathBuf,
        docker_compose_hash: String,
    ) -> VmSpec {
        let CvmFiles { base_disk, kernel, verity_root_hash, verity_disk } =
            if workload.gpus.is_empty() { &self.cvm_config.cpu } else { &self.cvm_config.gpu };
        let kernel_args =
            KernelArgs { filesystem_root_hash: verity_root_hash, docker_compose_hash: &docker_compose_hash };
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

    async fn create_state_disk(&self, workload: &Workload) -> Result<PathBuf, StartVmError> {
        let disk_name = format!("{}.raw", workload.id);
        let disk_path = self.state_path.join(disk_name);
        if disk_path.exists() {
            return Ok(disk_path);
        }
        self.disk_service
            .create_disk(&disk_path, HardDiskFormat::Raw, workload.disk_space_gb)
            .await
            .map_err(|e| StartVmError(format!("failed to create state disk: {e}")))?;
        Ok(disk_path)
    }

    async fn create_application_iso(&self, workload: &Workload) -> Result<(PathBuf, String), StartVmError> {
        let iso_name = format!("{}.iso", workload.id);
        let iso_path = self.state_path.join(iso_name);
        let docker_compose_hash = hex::encode(Sha256::digest(&workload.docker_compose));
        if iso_path.exists() {
            return Ok((iso_path, docker_compose_hash));
        }
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
        self.disk_service
            .create_application_iso(&iso_path, spec)
            .await
            .map_err(|e| StartVmError(format!("failed to create ISO: {e}")))?;
        Ok((iso_path, docker_compose_hash))
    }
}

#[async_trait]
impl VmService for DefaultVmService {
    async fn start_vm(&self, workload: Workload) -> Result<(), StartVmError> {
        let id = workload.id;
        let socket_path = self.state_path.join(format!("{id}.sock"));
        let mut workers = self.workers.lock().await;
        match workers.get(&id) {
            Some(_) => {
                info!("VM {id} is already running");
                Ok(())
            }
            None => {
                info!("Creating disks for VM {id}");
                let (iso_path, docker_compose_hash) = self.create_application_iso(&workload).await?;
                let state_disk = self.create_state_disk(&workload).await?;
                let spec = self.create_vm_spec(&workload, iso_path, state_disk, docker_compose_hash);
                let worker =
                    VmWorker::spawn(id, self.vm_client.clone(), self.nilcc_api_client.clone(), spec, socket_path);
                workers.insert(id, worker);
                Ok(())
            }
        }
    }

    async fn stop_vm(&self, id: Uuid) {
        let mut workers = self.workers.lock().await;
        match workers.remove(&id) {
            Some(worker) => {
                worker.stop_vm().await;
            }
            None => {
                error!("VM {id} is not being managed by any worker");
            }
        }
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

#[derive(Debug, thiserror::Error)]
#[error("internal: {0}")]
pub struct StartVmError(String);

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        clients::{nilcc_api::MockNilccApiClient, qemu::MockVmClient},
        services::disk::MockDiskService,
    };
    use mockall::predicate::eq;

    struct Builder {
        state_path: PathBuf,
        vm_client: MockVmClient,
        nilcc_api_client: MockNilccApiClient,
        disk_service: MockDiskService,
        cvm_config: CvmConfig,
    }

    impl Builder {
        async fn build(self) -> DefaultVmService {
            let Self { state_path, vm_client, nilcc_api_client, disk_service, cvm_config } = self;
            let args = VmServiceArgs {
                state_path,
                vm_client: Arc::new(vm_client),
                nilcc_api_client: Arc::new(nilcc_api_client),
                disk_service: Box::new(disk_service),
                cvm_config,
            };
            DefaultVmService::new(args).await.expect("failed to build")
        }
    }

    impl Default for Builder {
        fn default() -> Self {
            Self {
                state_path: PathBuf::from("/tmp"),
                vm_client: Default::default(),
                nilcc_api_client: Default::default(),
                disk_service: Default::default(),
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
            }
        }
    }

    #[tokio::test]
    async fn start_vm() {
        let workload = Workload {
            id: Uuid::new_v4(),
            docker_compose: "compose".into(),
            env_vars: Default::default(),
            public_container_name: "api".into(),
            public_container_port: 80,
            memory_mb: 1024,
            cpus: 1,
            gpus: vec![],
            disk_space_gb: 1.try_into().unwrap(),
            proxy_http_port: 1000,
            proxy_https_port: 1001,
        };
        let mut builder = Builder::default();
        let id = workload.id;
        let base_path = builder.state_path.display();
        let state_disk_path = PathBuf::from(format!("{base_path}/{id}.raw"));

        builder.disk_service.expect_create_application_iso().return_once(move |_, _| Ok(()));
        builder
            .disk_service
            .expect_create_disk()
            .with(eq(state_disk_path), eq(HardDiskFormat::Raw), eq(1))
            .return_once(move |_, _, _| Ok(()));

        let service = builder.build().await;
        service.start_vm(workload).await.expect("failed to start");
    }
}
