//! Linux session shelf listener.
use crate::server_socket::SocketBinding;
use crate::shelf::ShelfLimits;
use crate::shelf_service::{ServiceLimits, ShelfService};
use eyre::Result;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::task::JoinSet;
use tokio::time::{Duration, MissedTickBehavior};

pub fn default_socket_path() -> Result<PathBuf> {
    crate::server_socket::default_socket_path()
}

pub async fn run(path: PathBuf, limits: ShelfLimits, policy: ServiceLimits) -> Result<()> {
    let service = Arc::new(ShelfService::new(limits, policy)?);
    let binding = SocketBinding::bind(&path).await?;
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
                if workers.len() >= policy.max_connections || crate::server_socket::verify_peer_credentials(&stream).is_err() { continue; }
                let service = service.clone();
                workers.spawn(async move { service.serve(stream).await });
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
