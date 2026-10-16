//! Bounded direct children; callers provide argv, never a shell command string.
use eyre::{Result, bail};
use std::process::Stdio;
use std::time::Duration;
use tokio::io::{AsyncRead, AsyncReadExt};
use tokio::process::Command;

pub async fn output(command: &mut Command, limit: usize, deadline: Duration) -> Result<Vec<u8>> {
    let mut child = command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .map_err(|_| eyre::eyre!("maintenance tool could not be started"))?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| eyre::eyre!("missing tool output"))?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| eyre::eyre!("missing tool diagnostics"))?;
    let result = tokio::time::timeout(deadline, async {
        let (bytes, _, status) =
            tokio::try_join!(bounded(stdout, limit), bounded(stderr, 8192), async {
                Ok::<_, eyre::Report>(child.wait().await?)
            })?;
        if !status.success() {
            bail!("maintenance tool failed; check connectivity or the user service locally");
        }
        Ok(bytes)
    })
    .await;
    match result {
        Ok(Ok(bytes)) => Ok(bytes),
        failed => {
            let _ = tokio::time::timeout(Duration::from_secs(2), child.kill()).await;
            match failed {
                Ok(Err(error)) => Err(error),
                _ => Err(eyre::eyre!(
                    "maintenance tool timed out; its direct child was stopped"
                )),
            }
        }
    }
}

async fn bounded(mut stream: impl AsyncRead + Unpin, limit: usize) -> Result<Vec<u8>> {
    let mut bytes = Vec::new();
    let mut chunk = [0u8; 16384];
    loop {
        let count = stream.read(&mut chunk).await?;
        if count == 0 {
            return Ok(bytes);
        }
        if count > limit.saturating_sub(bytes.len()) {
            bail!("maintenance tool output exceeded its limit");
        }
        bytes.try_reserve_exact(count)?;
        bytes.extend_from_slice(&chunk[..count]);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn large_output_and_stalled_children_fail_with_bounded_cleanup() {
        let mut large = Command::new("head");
        large.args(["-c", "100000", "/dev/zero"]);
        assert!(
            output(&mut large, 4096, Duration::from_secs(2))
                .await
                .unwrap_err()
                .to_string()
                .contains("limit")
        );
        let before = std::time::Instant::now();
        let mut stalled = Command::new("sleep");
        stalled.arg("30");
        assert!(
            output(&mut stalled, 1024, Duration::from_millis(50))
                .await
                .unwrap_err()
                .to_string()
                .contains("timed out")
        );
        assert!(before.elapsed() < Duration::from_secs(3));
    }
}
