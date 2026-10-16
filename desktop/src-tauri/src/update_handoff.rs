//! One-use loopback metadata handoff to the framework installer API.
//! Remote reads are bounded HTTPS in update_network; the framework never fetches remote JSON.
use crate::update_policy::Candidate;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

pub struct Bridge {
    pub url: url::Url,
    task: tokio::task::JoinHandle<()>,
}
impl Bridge {
    pub async fn new(candidate: &Candidate) -> Result<Self, String> {
        let body=serde_json::to_vec(&serde_json::json!({"version":candidate.version,"platforms":{candidate.platform.clone():{"url":candidate.url,"signature":candidate.signature}}})).map_err(|_|"Could not prepare installer metadata.")?;
        if body.len() > crate::update_feed::MAX_FEED_BYTES {
            return Err("Installer metadata exceeds its limit.".into());
        }
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .map_err(|_| "Could not prepare local installer handoff.")?;
        let address = listener
            .local_addr()
            .map_err(|_| "Could not read installer handoff address.")?;
        let route = format!("/{}", uuid::Uuid::new_v4());
        let url = url::Url::parse(&format!("http://{address}{route}"))
            .map_err(|_| "Could not prepare installer handoff URL.")?;
        let task = tokio::spawn(async move {
            let _=tokio::time::timeout(std::time::Duration::from_secs(5),async {
                let (mut stream,_)=listener.accept().await?;
                let mut request=Vec::new();
                loop {
                    let mut chunk=[0;512];let read=stream.read(&mut chunk).await?;
                    if read==0 || request.len()+read>4096 {return Ok::<_,std::io::Error>(());}
                    request.extend_from_slice(&chunk[..read]);
                    if request.windows(4).any(|part|part==b"\r\n\r\n") {break;}
                }
                let expected=format!("GET {route} HTTP/1.1\r\n");
                if !request.starts_with(expected.as_bytes()) {return Ok(());}
                let header=format!("HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nCache-Control: no-store\r\nConnection: close\r\n\r\n",body.len());
                stream.write_all(header.as_bytes()).await?;
                stream.write_all(&body).await?;
                stream.shutdown().await
            }).await;
        });
        Ok(Self { url, task })
    }
}
impl Drop for Bridge {
    fn drop(&mut self) {
        self.task.abort();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn candidate() -> Candidate {
        Candidate {
            version: "2.0.0".into(),
            platform: "windows-x86_64".into(),
            url: "https://github.com/example/unused".into(),
            signature: "synthetic".into(),
        }
    }
    #[tokio::test]
    async fn local_handoff_is_one_use_and_contains_only_selected_metadata() {
        let _ = rustls::crypto::ring::default_provider().install_default();
        let bridge = Bridge::new(&candidate()).await.unwrap();
        let client = reqwest::Client::builder()
            .no_proxy()
            .timeout(std::time::Duration::from_secs(2))
            .build()
            .unwrap();
        let bytes = client
            .get(bridge.url.clone())
            .send()
            .await
            .unwrap()
            .bytes()
            .await
            .unwrap();
        assert!(bytes.len() < 1024);
        let value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(value["version"], "2.0.0");
        assert_eq!(value["platforms"].as_object().unwrap().len(), 1);
        assert!(client.get(bridge.url.clone()).send().await.is_err());
    }
    #[tokio::test]
    async fn wrong_route_and_dropped_handoffs_cannot_serve_metadata() {
        let _ = rustls::crypto::ring::default_provider().install_default();
        let bridge = Bridge::new(&candidate()).await.unwrap();
        let mut wrong = bridge.url.clone();
        wrong.set_path("/wrong");
        let client = reqwest::Client::builder()
            .no_proxy()
            .timeout(std::time::Duration::from_secs(2))
            .build()
            .unwrap();
        assert!(client.get(wrong).send().await.is_err());
        let bridge = Bridge::new(&candidate()).await.unwrap();
        let url = bridge.url.clone();
        drop(bridge);
        tokio::task::yield_now().await;
        assert!(client.get(url).send().await.is_err());
    }
}
