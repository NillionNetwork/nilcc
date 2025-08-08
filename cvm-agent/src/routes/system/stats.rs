use axum::{http::StatusCode, Json};
use cvm_agent_models::stats::{CpuStats, DiskStats, MemoryStats, SystemStatsResponse};
use sysinfo::{
    CpuRefreshKind, DiskRefreshKind, Disks, MemoryRefreshKind, RefreshKind, System, MINIMUM_CPU_UPDATE_INTERVAL,
};
use tokio::time::sleep;

pub(crate) async fn handler() -> Result<Json<SystemStatsResponse>, StatusCode> {
    let specifics = RefreshKind::nothing()
        .with_memory(MemoryRefreshKind::nothing().with_ram())
        .with_cpu(CpuRefreshKind::nothing().with_cpu_usage().with_frequency());
    let mut stats = System::new_with_specifics(specifics);
    sleep(MINIMUM_CPU_UPDATE_INTERVAL).await;
    stats.refresh_cpu_usage();

    let cpus = cpu_stats(&stats);
    let memory = memory_stats(&stats);
    let disks = disk_stats();
    let response = SystemStatsResponse { memory, cpus, disks };
    Ok(Json(response))
}

fn cpu_stats(stats: &System) -> Vec<CpuStats> {
    stats
        .cpus()
        .iter()
        .map(|cpu| CpuStats { name: cpu.name().to_string(), usage: cpu.cpu_usage(), frequency: cpu.frequency() })
        .collect()
}

fn memory_stats(stats: &System) -> MemoryStats {
    MemoryStats { total: stats.total_memory(), used: stats.used_memory() }
}

fn disk_stats() -> Vec<DiskStats> {
    let specifics = DiskRefreshKind::nothing().with_storage();
    let disks = Disks::new_with_refreshed_list_specifics(specifics);
    disks
        .list()
        .iter()
        .map(|d| DiskStats {
            name: d.name().to_string_lossy().to_string(),
            filesystem: d.file_system().to_string_lossy().to_string(),
            mount_point: d.mount_point().to_path_buf(),
            size: d.total_space(),
            used: d.total_space().saturating_sub(d.available_space()),
        })
        .collect()
}
