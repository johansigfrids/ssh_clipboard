//! A bounded byte bridge after verified bootstrap. It does not buffer full clipboard frames.
use crate::session_protocol::framing::{self, FrameLimits, WireError};
use crate::session_protocol::{Request, Response};
use std::time::Duration;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::time::{Instant, timeout, timeout_at};

#[derive(Clone, Copy)]
pub struct BridgeLimits {
    pub handshake_timeout: Duration,
    pub read_timeout: Duration,
    pub write_timeout: Duration,
}
impl Default for BridgeLimits {
    fn default() -> Self {
        Self {
            handshake_timeout: Duration::from_secs(5),
            read_timeout: Duration::from_secs(300),
            write_timeout: Duration::from_secs(30),
        }
    }
}
impl BridgeLimits {
    pub fn validate(&self) -> Result<(), WireError> {
        if self.handshake_timeout.is_zero()
            || self.handshake_timeout > Duration::from_secs(30)
            || self.read_timeout.is_zero()
            || self.read_timeout > Duration::from_secs(3600)
            || self.write_timeout.is_zero()
            || self.write_timeout > Duration::from_secs(300)
        {
            return Err(WireError::InvalidMetadata);
        }
        Ok(())
    }
}

pub async fn forward(
    input: &mut (impl AsyncRead + Unpin),
    output: &mut (impl AsyncWrite + Unpin),
    peer: impl AsyncRead + AsyncWrite + Unpin,
    hello: Request,
    limits: BridgeLimits,
) -> Result<(), WireError> {
    limits.validate()?;
    hello.validate(0)?;
    let Request::Hello {
        versions,
        max_item_bytes,
        max_decoded_bytes,
        ..
    } = &hello
    else {
        return Err(WireError::InvalidState);
    };
    let mut peer = peer;
    let deadline = Instant::now() + limits.handshake_timeout;
    let ready = timeout_at(deadline, async {
        framing::write_message(&mut peer, &hello, &[], FrameLimits::bootstrap()).await?;
        let header = framing::read_header(&mut peer, FrameLimits::bootstrap()).await?;
        let response: Response = framing::read_metadata(&mut peer, &header).await?;
        response.validate(0)?;
        if response.request_id() != hello.request_id()
            && !(matches!(response, Response::Error { .. }) && response.request_id().get() == 0)
        {
            return Err(WireError::InvalidMetadata);
        }
        match &response {
            Response::Welcome {
                session_version,
                limits: effective,
                ..
            } => {
                if !versions.contains(session_version)
                    || effective.max_item_bytes > *max_item_bytes
                    || effective.max_decoded_bytes > *max_decoded_bytes
                {
                    return Err(WireError::InvalidMetadata);
                }
            }
            Response::Error { .. } => {}
            _ => return Err(WireError::InvalidState),
        }
        framing::write_message(output, &response, &[], FrameLimits::bootstrap()).await?;
        Ok::<_, WireError>(matches!(response, Response::Welcome { .. }))
    })
    .await
    .map_err(|_| timed_out())??;
    if !ready {
        return Ok(());
    }
    let (mut reader, mut writer) = tokio::io::split(peer);
    let upload = async {
        copy_leg(input, &mut writer, limits).await?;
        timeout(limits.write_timeout, writer.shutdown())
            .await
            .map_err(|_| timed_out())??;
        Ok::<_, WireError>(())
    };
    let download = copy_leg(&mut reader, output, limits);
    tokio::pin!(upload, download);
    tokio::select! {
        result = &mut download => result,
        result = &mut upload => {
            result?;
            timeout(limits.write_timeout, &mut download).await.map_err(|_| timed_out())?
        }
    }
}

async fn copy_leg(
    reader: &mut (impl AsyncRead + Unpin),
    writer: &mut (impl AsyncWrite + Unpin),
    limits: BridgeLimits,
) -> Result<(), WireError> {
    let mut buffer = [0; 8192];
    loop {
        let read = timeout(limits.read_timeout, reader.read(&mut buffer))
            .await
            .map_err(|_| timed_out())??;
        if read == 0 {
            return Ok(());
        }
        timeout(limits.write_timeout, async {
            writer.write_all(&buffer[..read]).await?;
            writer.flush().await
        })
        .await
        .map_err(|_| timed_out())??;
    }
}
fn timed_out() -> WireError {
    std::io::Error::new(
        std::io::ErrorKind::TimedOut,
        "session bridge deadline exceeded",
    )
    .into()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shelf::ShelfLimits;
    use crate::shelf_service::{ServiceLimits, ShelfService};
    use std::sync::Arc;
    #[tokio::test]
    async fn verified_bootstrap_and_repeated_requests_survive_half_close() {
        let service =
            Arc::new(ShelfService::new(ShelfLimits::default(), ServiceLimits::default()).unwrap());
        let (peer, server) = tokio::io::duplex(64);
        let server_task = tokio::spawn(async move { service.serve(server).await });
        let (mut client, bridge) = tokio::io::duplex(64);
        let bridge_task = tokio::spawn(async move {
            let (mut input, mut output) = tokio::io::split(bridge);
            let hello = framing::read_hello(&mut input).await?;
            forward(
                &mut input,
                &mut output,
                peer,
                hello,
                BridgeLimits::default(),
            )
            .await
        });
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
        let Response::Welcome { epoch, limits, .. } =
            framing::read_metadata(&mut client, &header).await.unwrap()
        else {
            panic!("not welcomed")
        };
        for id in [2, 3] {
            let request = Request::Health {
                request_id: id.into(),
                epoch,
            };
            framing::write_message(&mut client, &request, &[], limits.frame_limits().unwrap())
                .await
                .unwrap();
            let header = framing::read_header(&mut client, limits.frame_limits().unwrap())
                .await
                .unwrap();
            assert!(
                matches!(framing::read_metadata::<Response>(&mut client, &header).await.unwrap(), Response::Health {request_id, items: 0, ..} if request_id.get() == id)
            );
        }
        client.shutdown().await.unwrap();
        timeout(Duration::from_secs(2), bridge_task)
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        timeout(Duration::from_secs(2), server_task)
            .await
            .unwrap()
            .unwrap()
            .unwrap();
    }

    #[tokio::test]
    async fn a_framed_rejection_finishes_without_waiting_for_either_input_to_close() {
        let (peer, mut fake) = tokio::io::duplex(64);
        let (mut client, bridge) = tokio::io::duplex(64);
        let (release, wait) = tokio::sync::oneshot::channel();
        let fake_task = tokio::spawn(async move {
            let hello = framing::read_hello(&mut fake).await.unwrap();
            let reply = Response::Error {
                request_id: hello.request_id(),
                code: crate::session_protocol::ErrorCode::UnsupportedVersion,
                epoch: None,
            };
            framing::write_message(&mut fake, &reply, &[], FrameLimits::bootstrap())
                .await
                .unwrap();
            let _ = wait.await;
        });
        let bridge_task = tokio::spawn(async move {
            let (mut input, mut output) = tokio::io::split(bridge);
            let hello = framing::read_hello(&mut input).await?;
            forward(
                &mut input,
                &mut output,
                peer,
                hello,
                BridgeLimits::default(),
            )
            .await
        });
        let hello = Request::Hello {
            request_id: 1.into(),
            versions: vec![9],
            max_item_bytes: 1,
            max_decoded_bytes: 1,
        };
        framing::write_message(&mut client, &hello, &[], FrameLimits::bootstrap())
            .await
            .unwrap();
        let header = framing::read_header(&mut client, FrameLimits::bootstrap())
            .await
            .unwrap();
        assert!(matches!(
            framing::read_metadata::<Response>(&mut client, &header)
                .await
                .unwrap(),
            Response::Error { .. }
        ));
        timeout(Duration::from_secs(1), bridge_task)
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        release.send(()).unwrap();
        fake_task.await.unwrap();
    }

    #[tokio::test]
    async fn mismatched_welcome_ids_are_rejected_before_forwarding_ready_state() {
        let (peer, mut fake) = tokio::io::duplex(64);
        let (mut client, bridge) = tokio::io::duplex(64);
        let fake_task = tokio::spawn(async move {
            let hello = framing::read_hello(&mut fake).await.unwrap();
            let mut reply = crate::session_protocol::negotiate(
                &hello,
                uuid::Uuid::from_u128(1),
                0,
                &ShelfLimits::default(),
            )
            .unwrap();
            if let Response::Welcome { request_id, .. } = &mut reply {
                *request_id = 99.into();
            }
            framing::write_message(&mut fake, &reply, &[], FrameLimits::bootstrap())
                .await
                .unwrap();
        });
        let task = tokio::spawn(async move {
            let (mut input, mut output) = tokio::io::split(bridge);
            let hello = framing::read_hello(&mut input).await?;
            forward(
                &mut input,
                &mut output,
                peer,
                hello,
                BridgeLimits::default(),
            )
            .await
        });
        let hello = Request::Hello {
            request_id: 1.into(),
            versions: vec![1],
            max_item_bytes: 1024,
            max_decoded_bytes: 1024,
        };
        framing::write_message(&mut client, &hello, &[], FrameLimits::bootstrap())
            .await
            .unwrap();
        assert!(matches!(
            timeout(Duration::from_secs(1), task)
                .await
                .unwrap()
                .unwrap(),
            Err(WireError::InvalidMetadata)
        ));
        let mut byte = [0];
        assert_eq!(client.read(&mut byte).await.unwrap(), 0);
        fake_task.await.unwrap();
    }
}
