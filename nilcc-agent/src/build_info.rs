use chrono::{DateTime, TimeZone, Utc};
use once_cell::sync::Lazy;

const PKG_VERSION: &str = env!("CARGO_PKG_VERSION");
const BUILD_GIT_COMMIT_HASH: &str = env!("BUILD_GIT_COMMIT_HASH");
const BUILD_TIMESTAMP: &str = env!("BUILD_TIMESTAMP");

static AGENT_VERSION: Lazy<String> = Lazy::new(|| {
    let mut metadata_parts: Vec<String> = Vec::new();

    let build_datetime_iso_formatted = BUILD_TIMESTAMP
        .parse::<i64>()
        .ok()
        .and_then(|ts| Utc.timestamp_opt(ts, 0).single())
        .map(|dt: DateTime<Utc>| dt.format("%Y%m%dT%H%M%SZ").to_string());

    metadata_parts.push(format!("git.{BUILD_GIT_COMMIT_HASH}"));

    if let Some(datetime_str) = build_datetime_iso_formatted {
        metadata_parts.push(format!("build.{datetime_str}"));
    }

    if metadata_parts.is_empty() {
        PKG_VERSION.to_string()
    } else {
        format!("{PKG_VERSION}+{}", metadata_parts.join("."))
    }
});

/// Returns the agent version in the format: PKG_VERSION+git.GIT_COMMIT_HASH.build.YYYYMMDDTHHMMSSZ
/// Example: 0.1.0+git.9cd0b27.build.20250604T161556Z
pub fn get_agent_version() -> &'static str {
    &AGENT_VERSION
}
