//! Experimental Linux shelf listener with an optional explicit V2 compatibility socket.
use crate::server_socket::SocketBinding;
use crate::shelf::ShelfLimits;
use crate::shelf_service::{ServiceLimits, ShelfService};
use eyre::Result;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::task::JoinSet;
use tokio::time::{Duration, MissedTickBehavior};

pub fn default_socket_path() -> Result<PathBuf> {
    Ok(crate::daemon::default_socket_path()?.with_file_name("shelf.sock"))
}

pub async fn run(path: PathBuf, limits: ShelfLimits, policy: ServiceLimits) -> Result<()> {
    run_with_legacy(path, limits, policy, None).await
}

pub struct LegacyEndpoint {
    pub path: PathBuf,
    pub max_frame: usize,
}

pub async fn run_with_legacy(
    path: PathBuf,
    limits: ShelfLimits,
    policy: ServiceLimits,
    legacy: Option<LegacyEndpoint>,
) -> Result<()> {
    if let Some(legacy) = &legacy {
        eyre::ensure!(
            legacy.path != path,
            "shelf and compatibility sockets must differ"
        );
        eyre::ensure!(
            legacy.max_frame > 0
                && legacy.max_frame
                    <= crate::session_protocol::MAX_PAYLOAD_BYTES
                        + crate::protocol::RESPONSE_OVERHEAD,
            "invalid compatibility frame limit"
        );
    }
    let service = Arc::new(ShelfService::new(limits, policy)?);
    let binding = SocketBinding::bind(&path).await?;
    // Bind both before serving. An occupied compatibility path is preserved and
    // failure releases the new shelf socket instead of starting half-configured.
    let legacy_binding = match &legacy {
        Some(endpoint) => Some(SocketBinding::bind(&endpoint.path).await?),
        None => None,
    };
    let mut workers = JoinSet::new();
    let mut maintenance = tokio::time::interval(Duration::from_secs(1));
    maintenance.set_missed_tick_behavior(MissedTickBehavior::Skip);
    tracing::info!(path = %path.display(), "shelf daemon listening");
    loop {
        tokio::select! {
            accepted = binding.listener.accept() => {
                let (stream, _) = accepted?;
                while let Some(result) = workers.try_join_next() { report(result); }
                // Bound accepted tasks before spawning; a rejected socket is closed unread.
                if workers.len() >= policy.max_connections || crate::daemon::verify_peer_credentials(&stream).is_err() { continue; }
                let service = service.clone();
                workers.spawn(async move { service.serve(stream).await });
            }
            accepted = async {
                match &legacy_binding {
                    Some(binding) => binding.listener.accept().await,
                    None => std::future::pending().await,
                }
            } => {
                let (stream, _) = accepted?;
                while let Some(result) = workers.try_join_next() { report(result); }
                if workers.len() >= policy.max_connections || crate::daemon::verify_peer_credentials(&stream).is_err() { continue; }
                let service = service.clone();
                let max_frame = legacy.as_ref().expect("enabled listener").max_frame;
                workers.spawn(async move { service.serve_legacy(stream, max_frame).await });
            }
            completed = workers.join_next(), if !workers.is_empty() => {
                if let Some(result) = completed { report(result); }
            }
            _ = maintenance.tick() => { service.maintenance()?; }
        }
    }
}

fn report(result: Result<Result<(), crate::session_protocol::WireError>, tokio::task::JoinError>) {
    match result {
        Ok(Err(error)) => tracing::debug!(error = %error, "shelf session closed"),
        Err(error) if !error.is_cancelled() => {
            tracing::error!(error = %error, "shelf session task failed")
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session_protocol::framing::{self, FrameLimits};
    use crate::session_protocol::{Request, Response};
    use tokio::net::UnixStream;
    #[tokio::test]
    async fn native_listener_negotiates_and_duplicate_startup_preserves_the_primary() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("runtime/shelf.sock");
        let task = tokio::spawn(run(
            path.clone(),
            ShelfLimits::default(),
            ServiceLimits::default(),
        ));
        let mut client = tokio::time::timeout(Duration::from_secs(2), async {
            loop {
                if let Ok(stream) = UnixStream::connect(&path).await {
                    break stream;
                }
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
        })
        .await
        .unwrap();
        assert!(
            run(
                path.clone(),
                ShelfLimits::default(),
                ServiceLimits::default()
            )
            .await
            .is_err()
        );
        let hello = Request::Hello {
            request_id: 1.into(),
            versions: vec![1],
            max_item_bytes: 1024,
            max_decoded_bytes: 1024,
        };
        framing::write_message(&mut client, &hello, &[], FrameLimits::bootstrap())
            .await
            .unwrap();
        let header = framing::read_header(&mut client, FrameLimits::bootstrap())
            .await
            .unwrap();
        let response: Response = framing::read_metadata(&mut client, &header).await.unwrap();
        assert!(matches!(response, Response::Welcome { .. }));
        task.abort();
        let _ = task.await;
        assert!(!path.exists());
    }
}
