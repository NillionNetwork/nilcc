use crate::repositories::sqlite::SqliteTransactionContext;
use async_trait::async_trait;
use nilcc_artifacts::metadata::ArtifactsMetadata;
use sqlx::FromRow;

#[cfg_attr(test, mockall::automock)]
#[async_trait]
pub trait ArtifactsRepository: Send + Sync {
    /// Insert a new version.
    async fn create(&mut self, version: &str, metadata: &ArtifactsMetadata) -> Result<(), ArtifactsRepositoryError>;

    /// Update the metadata for an artifact.
    async fn update_metadata(
        &mut self,
        version: &str,
        metadata: &ArtifactsMetadata,
    ) -> Result<(), ArtifactsRepositoryError>;

    /// Find an artifacts version.
    async fn find(&mut self, version: &str) -> Result<Option<Artifacts>, ArtifactsRepositoryError>;

    /// List the available versions.
    async fn list(&mut self) -> Result<Vec<Artifacts>, ArtifactsRepositoryError>;

    /// Check if a version already exists.
    async fn exists(&mut self, version: &str) -> Result<bool, ArtifactsRepositoryError>;

    /// Delete a version.
    async fn delete(&mut self, version: &str) -> Result<(), ArtifactsRepositoryError>;

    /// Commit all changes.
    async fn commit(self: Box<Self>) -> Result<(), ArtifactsRepositoryError>;
}

#[derive(FromRow)]
pub struct Artifacts {
    pub version: String,
    #[sqlx(json)]
    pub metadata: ArtifactsMetadata,
}

#[derive(Debug, thiserror::Error)]
pub enum ArtifactsRepositoryError {
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),
}

pub struct SqliteArtifactsRepository<'a> {
    ctx: SqliteTransactionContext<'a>,
}

impl<'a> SqliteArtifactsRepository<'a> {
    pub fn new(ctx: SqliteTransactionContext<'a>) -> Self {
        Self { ctx }
    }
}

#[async_trait]
impl<'a> ArtifactsRepository for SqliteArtifactsRepository<'a> {
    async fn create(&mut self, version: &str, metadata: &ArtifactsMetadata) -> Result<(), ArtifactsRepositoryError> {
        let query = "INSERT INTO artifacts (version, metadata) VALUES (?, ?)";
        sqlx::query(query).bind(version).bind(sqlx::types::Json(metadata)).execute(&mut *self.ctx).await?;
        Ok(())
    }

    async fn update_metadata(
        &mut self,
        version: &str,
        metadata: &ArtifactsMetadata,
    ) -> Result<(), ArtifactsRepositoryError> {
        let query = "UPDATE artifacts SET metadata = ? WHERE version = ?";
        sqlx::query(query).bind(sqlx::types::Json(metadata)).bind(version).execute(&mut *self.ctx).await?;
        Ok(())
    }

    async fn find(&mut self, version: &str) -> Result<Option<Artifacts>, ArtifactsRepositoryError> {
        let query = "SELECT version, metadata FROM artifacts WHERE version = ?";
        let row = sqlx::query_as(query).bind(version).fetch_optional(&mut *self.ctx).await?;
        Ok(row)
    }

    async fn list(&mut self) -> Result<Vec<Artifacts>, ArtifactsRepositoryError> {
        let query = "SELECT version, metadata FROM artifacts";
        let rows = sqlx::query_as(query).fetch_all(&mut *self.ctx).await?;
        Ok(rows)
    }

    async fn exists(&mut self, version: &str) -> Result<bool, ArtifactsRepositoryError> {
        let query = "SELECT 1 FROM artifacts WHERE version = ?";
        let row = sqlx::query(query).bind(version).fetch_optional(&mut *self.ctx).await?;
        Ok(row.is_some())
    }

    async fn delete(&mut self, version: &str) -> Result<(), ArtifactsRepositoryError> {
        let query = "DELETE FROM artifacts WHERE version = ?";
        sqlx::query(query).bind(version).execute(&mut *self.ctx).await?;
        Ok(())
    }

    async fn commit(mut self: Box<Self>) -> Result<(), ArtifactsRepositoryError> {
        self.ctx.commit().await?;
        Ok(())
    }
}

#[cfg(test)]
pub(crate) mod utils {
    use nilcc_artifacts::metadata::{
        Artifact, ArtifactsMetadata, Cvm, CvmDisk, CvmImage, CvmImages, DiskFormat, KernelCommandLine, PackageMetadata,
        Verity, VerityDisk,
    };

    pub(crate) fn make_artifacts_metadata() -> ArtifactsMetadata {
        ArtifactsMetadata {
            kernel: PackageMetadata { commit: "".into() },
            qemu: PackageMetadata { commit: "".into() },
            ovmf: Artifact { path: "".into(), sha256: [0; 32] },
            initrd: Artifact { path: "".into(), sha256: [0; 32] },
            cvm: Cvm {
                cmdline: KernelCommandLine("panic=-1 root=/dev/sda2 verity_disk=/dev/sdb verity_roothash={VERITY_ROOT_HASH} state_disk=/dev/sdc docker_compose_disk=/dev/sr0 docker_compose_hash={DOCKER_COMPOSE_HASH}".into()),
                images: CvmImages {
                    cpu: CvmImage {
                        disk: CvmDisk {
                            artifact: Artifact { path: "vm_images/cvm-cpu.qcow2".into(), sha256: [0; 32] },
                            format: DiskFormat::Qcow2,
                        },
                        verity: Verity {
                            disk: VerityDisk {
                                path: "vm_images/cvm-cpu-verity/verity-hash-dev".into(),
                                format: DiskFormat::Raw,
                            },
                            root_hash: [0; 32],
                        },
                        kernel: Artifact { path: "vm_images/kernel/cpu-vmlinuz".into(), sha256: [0; 32] },
                    },
                    gpu: CvmImage {
                        disk: CvmDisk {
                            artifact: Artifact { path: "vm_images/cvm-gpu.qcow2".into(), sha256: [0; 32] },
                            format: DiskFormat::Qcow2,
                        },
                        verity: Verity {
                            disk: VerityDisk {
                                path: "vm_images/cvm-gpu-verity/verity-hash-dev".into(),
                                format: DiskFormat::Raw,
                            },
                            root_hash: [0; 32],
                        },
                        kernel: Artifact { path: "vm_images/kernel/gpu-vmlinuz".into(), sha256: [0; 32] },
                    },
                },
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::repositories::{
        artifacts::utils::make_artifacts_metadata,
        sqlite::{SqliteDb, SqliteTransactionContextInner},
    };

    #[tokio::test]
    async fn crud() {
        let db = SqliteDb::connect("sqlite://:memory:").await.expect("failed to create db");
        let connection = db.0.acquire().await.expect("failed to acquire");
        let mut repo = SqliteArtifactsRepository::new(SqliteTransactionContextInner::Connection(connection).into());

        let meta = make_artifacts_metadata();
        repo.create("aaa", &meta).await.expect("failed to set");
        repo.create("bbb", &meta).await.expect("failed to set");

        assert!(repo.exists("aaa").await.expect("lookup failed"));
        assert!(repo.exists("bbb").await.expect("lookup failed"));
        assert!(!repo.exists("cc").await.expect("lookup failed"));

        let versions = repo.list().await.expect("list failed");
        assert_eq!(versions.len(), 2);

        repo.delete("aaa").await.expect("delete failed");
        assert!(!repo.exists("aaa").await.expect("lookup failed"));
    }
}
