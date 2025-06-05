use chrono::DateTime;
use once_cell::sync::Lazy;

const PKG_VERSION: &str = env!("CARGO_PKG_VERSION");
const BUILD_GIT_COMMIT_HASH: &str = env!("BUILD_GIT_COMMIT_HASH");
const BUILD_TIMESTAMP: &str = env!("BUILD_TIMESTAMP");

static AGENT_VERSION: Lazy<String> = Lazy::new(|| {
    let timestamp_secs = BUILD_TIMESTAMP.parse::<i64>().expect("BUILD_TIMESTAMP must be a Unix timestamp.");
    let build_datetime_str = DateTime::from_timestamp(timestamp_secs, 0)
        .expect("BUILD_TIMESTAMP must correspond to a valid UTC DateTime.")
        .format("%Y%m%dT%H%M%SZ")
        .to_string();

    format!("{PKG_VERSION}+git.{BUILD_GIT_COMMIT_HASH}.build.{build_datetime_str}")
});

/// Returns the agent version in the format: PKG_VERSION+git.GIT_COMMIT_HASH.build.YYYYMMDDTHHMMSSZ
/// Example: 0.1.0+git.9cd0b27.build.20250604T161556Z
pub fn get_agent_version() -> &'static str {
    &AGENT_VERSION
}
