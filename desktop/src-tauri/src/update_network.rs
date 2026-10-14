//! Bounded HTTPS only. Never uses the framework's unbounded JSON/package readers.
use crate::{update_feed::Buffer, update_policy::Rejection};
use std::time::Duration;

fn remote(url: &url::Url) -> bool {
    url.scheme() == "https"
        && url.username().is_empty()
        && url.password().is_none()
        && matches!(
            url.host_str(),
            Some(
                "github.com"
                    | "release-assets.githubusercontent.com"
                    | "objects.githubusercontent.com"
            )
        )
        && url.port_or_known_default() == Some(443)
}

pub async fn download(
    url: &str,
    limit: usize,
    deadline: Duration,
    mut progress: impl FnMut(usize, Option<u64>),
) -> Result<Vec<u8>, String> {
    let _ = rustls::crypto::ring::default_provider().install_default();
    let url = url::Url::parse(url).map_err(|_| "Invalid update location.")?;
    if !remote(&url) {
        return Err("Update location is not trusted HTTPS.".into());
    }
    let client = reqwest::Client::builder()
        .https_only(true)
        .timeout(deadline)
        .connect_timeout(Duration::from_secs(10))
        .redirect(reqwest::redirect::Policy::custom(|attempt| {
            if attempt.previous().len() >= 5 || !remote(attempt.url()) {
                attempt.error("Update redirect rejected")
            } else {
                attempt.follow()
            }
        }))
        .user_agent("SSHClipboard-Update")
        .build()
        .map_err(|_| "Could not initialize update transport.")?;
    let mut response = client
        .get(url)
        .send()
        .await
        .map_err(|_| "Update download failed or timed out. Try again when connected.")?
        .error_for_status()
        .map_err(|_| "The release service did not provide this update. Try again later.")?;
    read(&mut response, limit, &mut progress).await
}

fn limit_error(_: Rejection) -> String {
    "The update exceeds the download limit. Nothing was installed.".into()
}

async fn read(
    response: &mut reqwest::Response,
    limit: usize,
    progress: &mut impl FnMut(usize, Option<u64>),
) -> Result<Vec<u8>, String> {
    let total = response.content_length();
    let mut buffer = Buffer::new(limit, total).map_err(limit_error)?;
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|_| "The update download was interrupted. Nothing was installed.")?
    {
        buffer.append(&chunk).map_err(limit_error)?;
        progress(buffer.len(), total);
    }
    Ok(buffer.finish())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    #[tokio::test]
    async fn streamed_overflow_and_interruption_discard_partial_downloads() {
        let _ = rustls::crypto::ring::default_provider().install_default();
        for response in [
            "HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n5\r\n12345\r\n4\r\n6789\r\n0\r\n\r\n",
            "HTTP/1.1 200 OK\r\nContent-Length: 8\r\n\r\n123",
        ] {
            let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
            let address = listener.local_addr().unwrap();
            let server = tokio::spawn(async move {
                let (mut stream, _) = listener.accept().await.unwrap();
                let mut request = [0; 4096];
                let _ = stream.read(&mut request).await;
                stream.write_all(response.as_bytes()).await.unwrap();
            });
            // Private test transport only; production download() rejects plaintext/loopback.
            let mut response = reqwest::Client::builder()
                .no_proxy()
                .timeout(Duration::from_secs(2))
                .build()
                .unwrap()
                .get(format!("http://{address}"))
                .send()
                .await
                .unwrap();
            assert!(read(&mut response, 8, &mut |_, _| {}).await.is_err());
            server.await.unwrap();
        }
    }
    #[test]
    fn redirect_hosts_and_schemes_fail_closed() {
        for location in [
            "http://github.com/a",
            "https://evil.example/a",
            "https://github.com:444/a",
            "https://user@github.com/a",
            "https://127.0.0.1/a",
        ] {
            assert!(!remote(&url::Url::parse(location).unwrap()));
        }
        assert!(remote(
            &url::Url::parse("https://release-assets.githubusercontent.com/a?token=redirect")
                .unwrap()
        ));
    }
}
