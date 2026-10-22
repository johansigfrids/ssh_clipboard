//! Linux stdio adapter. The CLI must exit explicitly after this returns, because a
//! cancelled Tokio stdin read can otherwise keep a blocking runtime worker alive.
use crate::session_bridge::{self, BridgeLimits};
use crate::session_protocol::framing::{self, FrameLimits};
use crate::session_protocol::{ErrorCode, Response};
use eyre::Result;
use std::path::PathBuf;
use tokio::net::UnixStream;

pub async fn run(path: PathBuf, limits: BridgeLimits) -> Result<i32> {
    limits.validate()?;
    let mut input = tokio::io::stdin();
    let mut output = tokio::io::stdout();
    let hello =
        tokio::time::timeout(limits.handshake_timeout, framing::read_hello(&mut input)).await??;
    let stream = match tokio::time::timeout(limits.handshake_timeout, UnixStream::connect(path))
        .await
    {
        Ok(Ok(stream)) if crate::server_socket::verify_peer_credentials(&stream).is_ok() => stream,
        _ => {
            let response = Response::Error {
                request_id: hello.request_id(),
                code: ErrorCode::DaemonUnavailable,
                epoch: None,
            };
            tokio::time::timeout(
                limits.write_timeout,
                framing::write_message(&mut output, &response, &[], FrameLimits::bootstrap()),
            )
            .await??;
            return Ok(4);
        }
    };
    session_bridge::forward(&mut input, &mut output, stream, hello, limits).await?;
    Ok(0)
}
