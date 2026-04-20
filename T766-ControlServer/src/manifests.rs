/// Serves puppet manifests as a tarball.
use axum::response::IntoResponse;

pub async fn handler() -> impl IntoResponse {
    match tokio::task::spawn_blocking(|| -> std::io::Result<Vec<u8>> {
        let mut buf = Vec::new();
        let mut archive = tar::Builder::new(&mut buf);
        archive.append_dir_all("manifests", "/puppet/manifests")?;
        archive.finish()?;
        drop(archive);
        Ok(buf)
    }).await {
        Ok(Ok(bytes)) => (
            [(axum::http::header::CONTENT_TYPE, "application/x-tar")],
            bytes,
        ).into_response(),
        _ => (
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            "Failed to build archive",
        ).into_response(),
    }
}
