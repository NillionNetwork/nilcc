use crate::{
    clients::nilcc_api::NilccApiClient,
    repositories::{sqlite::RepositoryProvider, workload::Workload},
    services::upgrade::{UpgradeError, UpgradeService},
};
use std::{collections::BTreeSet, sync::Arc, time::Duration};
use tokio::time::sleep;
use tracing::{debug, error, info, warn};

const HEARTBEAT_INTERVAL: Duration = Duration::from_secs(30);

pub struct HeartbeatWorkerArgs {
    pub api_client: Arc<dyn NilccApiClient>,
    pub provider: Arc<dyn RepositoryProvider>,
    pub upgrader: Arc<dyn UpgradeService>,
}

pub struct HeartbeatWorker {
    api_client: Arc<dyn NilccApiClient>,
    provider: Arc<dyn RepositoryProvider>,
    upgrader: Arc<dyn UpgradeService>,
}

impl HeartbeatWorker {
    pub fn spawn(args: HeartbeatWorkerArgs) {
        let HeartbeatWorkerArgs { api_client, provider, upgrader } = args;
        tokio::spawn(async move {
            let worker = Self { api_client, provider, upgrader };
            worker.run().await
        });
    }

    async fn run(self) {
        loop {
            debug!("Sending heartbeat");
            if let Err(e) = self.run_once().await {
                error!("Failed to send/process heartbeat: {e:#}");
            }
            debug!("Sleeping for {HEARTBEAT_INTERVAL:?}");
            sleep(HEARTBEAT_INTERVAL).await;
        }
    }

    async fn run_once(&self) -> anyhow::Result<()> {
        match self.load_available_artifact_versions().await {
            Ok(available_versions) => match self.api_client.heartbeat(available_versions.clone()).await {
                Ok(response) => {
                    self.handle_versions(available_versions, response.expected_artifact_versions).await;
                    Ok(())
                }
                Err(e) => {
                    warn!("Could not submit heartbeat: {e}");
                    Ok(())
                }
            },
            Err(e) => Err(e.context("Failed to load available artifact versions")),
        }
    }

    async fn load_available_artifact_versions(&self) -> anyhow::Result<Vec<String>> {
        let mut repo = self.provider.artifacts(Default::default()).await?;
        let artifacts = repo.list().await?;
        Ok(artifacts.into_iter().map(|a| a.version).collect())
    }

    async fn load_workloads(&self) -> anyhow::Result<Vec<Workload>> {
        let mut repo = self.provider.workloads(Default::default()).await?;
        let workloads = repo.list().await?;
        Ok(workloads)
    }

    async fn handle_versions(&self, available_versions: Vec<String>, expected_versions: Vec<String>) {
        let available_versions: BTreeSet<_> = available_versions.into_iter().collect();
        let expected_versions: BTreeSet<_> = expected_versions.into_iter().collect();
        self.install_missing_versions(&available_versions, &expected_versions).await;
        self.uninstall_unused_versions(&available_versions, &expected_versions).await;
    }

    async fn install_missing_versions(
        &self,
        available_versions: &BTreeSet<String>,
        expected_versions: &BTreeSet<String>,
    ) {
        let mut missing_versions = expected_versions.difference(available_versions);
        if let Some(version) = missing_versions.next() {
            info!("Installing artifacts version {version}");
            match self.upgrader.install_artifacts(version.clone()).await {
                Ok(_) => info!("Installation of artifact version {version} started"),
                Err(e) => match e {
                    UpgradeError::InvalidVersion => error!("Cannot install version {version} because it's invalid"),
                    UpgradeError::ExistingVersion => info!("Version {version} is already installed"),
                    UpgradeError::ActiveUpgrade(_) => {
                        info!("Can't install version {version} yet because another upgrade is in progress")
                    }
                    UpgradeError::Internal => error!("Failed to install {version} because of an internal error"),
                },
            }
        }
    }

    async fn uninstall_unused_versions(
        &self,
        available_versions: &BTreeSet<String>,
        expected_versions: &BTreeSet<String>,
    ) {
        let redundant_versions: Vec<_> = available_versions.difference(expected_versions).collect();
        if redundant_versions.is_empty() {
            return;
        }
        let workloads = match self.load_workloads().await {
            Ok(workloads) => workloads,
            Err(e) => {
                error!("Failed to load workloads: {e:#}");
                return;
            }
        };
        let versions_in_use: BTreeSet<_> = workloads.into_iter().map(|w| w.artifacts_version).collect();
        info!("Artifact versions {redundant_versions:?} are no longer required, {versions_in_use:?} are in use");
        for version in redundant_versions {
            if versions_in_use.contains(version) {
                continue;
            }
            if let Err(e) = self.upgrader.uninstall_artifact_version(version).await {
                error!("Failed to uninstall artifact version: {e:#}");
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        clients::nilcc_api::{HeartbeatResponse, MockNilccApiClient},
        repositories::{
            artifacts::{Artifacts, MockArtifactsRepository, utils::make_artifacts_metadata},
            sqlite::MockRepositoryProvider,
            workload::MockWorkloadRepository,
        },
        services::upgrade::MockUpgradeService,
    };
    use mockall::predicate::eq;

    #[derive(Default)]
    struct Builder {
        api_client: MockNilccApiClient,
        provider: MockRepositoryProvider,
        upgrader: MockUpgradeService,
    }

    impl Builder {
        fn build(self) -> HeartbeatWorker {
            let Self { api_client, provider, upgrader } = self;
            HeartbeatWorker {
                api_client: Arc::new(api_client),
                provider: Arc::new(provider),
                upgrader: Arc::new(upgrader),
            }
        }

        async fn set_existing_artifact_versions(&mut self, versions: &[&str]) {
            let artifacts = versions
                .into_iter()
                .map(|version| Artifacts { version: version.to_string(), metadata: make_artifacts_metadata() })
                .collect();
            self.provider.expect_artifacts().return_once(move |_| {
                let mut repo = MockArtifactsRepository::default();
                repo.expect_list().return_once(move || Ok(artifacts));
                Ok(Box::new(repo))
            });
        }
    }

    #[tokio::test]
    async fn install_versions() {
        let existing = &["a", "b", "d"];
        let expected = vec!["a".to_string(), "b".to_string(), "c".to_string()];
        let mut builder = Builder::default();
        builder.set_existing_artifact_versions(existing).await;
        builder.provider.expect_workloads().return_once(move |_| {
            let mut repo = MockWorkloadRepository::default();
            repo.expect_list().return_once(move || Ok(vec![]));
            Ok(Box::new(repo))
        });
        builder
            .api_client
            .expect_heartbeat()
            .with(eq(existing.into_iter().map(ToString::to_string).collect::<Vec<_>>()))
            .return_once(move |_| Ok(HeartbeatResponse { expected_artifact_versions: expected }));
        builder.upgrader.expect_install_artifacts().with(eq("c".to_string())).once().return_once(move |_| Ok(()));
        builder
            .upgrader
            .expect_uninstall_artifact_version()
            .with(eq("d".to_string()))
            .once()
            .return_once(move |_| Ok(()));

        let worker = builder.build();
        worker.run_once().await.expect("failed to run");
    }
}
