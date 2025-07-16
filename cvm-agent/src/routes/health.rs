use axum::Json;

// TODO: eventually expose information about e.g. docker
pub(crate) async fn handler() -> Json<()> {
    Json(())
}
