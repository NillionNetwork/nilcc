use axum::{http::StatusCode, Json};
use cvm_agent_models::stats::{CpuStats, MemoryStats, SystemStatsResponse};
use sysinfo::{CpuRefreshKind, MemoryRefreshKind, RefreshKind, System, MINIMUM_CPU_UPDATE_INTERVAL};
use tokio::time::sleep;

pub(crate) async fn handler() -> Result<Json<SystemStatsResponse>, StatusCode> {
    let specifics = RefreshKind::nothing()
        .with_memory(MemoryRefreshKind::nothing().with_ram())
        .with_cpu(CpuRefreshKind::nothing().with_cpu_usage().with_frequency());
    let mut stats = System::new_with_specifics(specifics);
    sleep(MINIMUM_CPU_UPDATE_INTERVAL).await;
    stats.refresh_cpu_usage();

    let cpus = stats
        .cpus()
        .iter()
        .map(|cpu| CpuStats { name: cpu.name().to_string(), usage: cpu.cpu_usage(), frequency: cpu.frequency() })
        .collect();
    let memory = MemoryStats { total: stats.total_memory(), used: stats.used_memory() };
    let response = SystemStatsResponse { memory, cpus };
    Ok(Json(response))
}
