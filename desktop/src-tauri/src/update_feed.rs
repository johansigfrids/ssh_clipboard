//! Untrusted feed parsing and allocation limits, independent of the installer.
use crate::update_policy::{Candidate, Policy, Rejection};
use serde::Deserialize;
use std::collections::BTreeMap;

pub const MAX_FEED_BYTES: usize = 64 * 1024;
pub const MAX_NOTES_BYTES: usize = 16 * 1024;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Feed {
    version: String,
    #[serde(default)]
    notes: String,
    #[serde(default)]
    pub_date: Option<String>,
    platforms: BTreeMap<String, Asset>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Asset {
    url: String,
    signature: String,
}

pub struct Release {
    pub candidate: Candidate,
    pub notes: String,
}

pub fn parse(
    bytes: &[u8],
    public_key: &str,
    current: &str,
    platform: &str,
) -> Result<Option<Release>, Rejection> {
    if bytes.len() > MAX_FEED_BYTES {
        return Err(Rejection::Size);
    }
    let feed: Feed = serde_json::from_slice(bytes).map_err(|_| Rejection::Metadata)?;
    if feed.notes.len() > MAX_NOTES_BYTES
        || feed.pub_date.as_ref().is_some_and(|date| date.len() > 64)
        || feed.platforms.len() > 6
    {
        return Err(Rejection::Metadata);
    }
    crate::update_policy::artifact_name(&feed.version, platform)?;
    let current_version = semver::Version::parse(current).map_err(|_| Rejection::Version)?;
    let version = semver::Version::parse(&feed.version).map_err(|_| Rejection::Version)?;
    // An older feed is never offered, even if its signature is genuine.
    if version <= current_version {
        return Ok(None);
    }
    let asset = feed.platforms.get(platform).ok_or(Rejection::Platform)?;
    let candidate = Candidate {
        version: feed.version,
        platform: platform.into(),
        url: asset.url.clone(),
        signature: asset.signature.clone(),
    };
    Policy::new(public_key, current, platform)?.check_candidate(&candidate)?;
    Ok(Some(Release {
        candidate,
        notes: feed.notes,
    }))
}

pub struct Buffer {
    bytes: Vec<u8>,
    limit: usize,
}
impl Buffer {
    pub fn new(limit: usize, announced: Option<u64>) -> Result<Self, Rejection> {
        if announced.is_some_and(|size| size > limit as u64) {
            return Err(Rejection::Size);
        }
        Ok(Self {
            bytes: Vec::new(),
            limit,
        })
    }
    pub fn append(&mut self, chunk: &[u8]) -> Result<(), Rejection> {
        if chunk.len() > self.limit.saturating_sub(self.bytes.len()) {
            return Err(Rejection::Size);
        }
        self.bytes
            .try_reserve_exact(chunk.len())
            .map_err(|_| Rejection::Size)?;
        self.bytes.extend_from_slice(chunk);
        Ok(())
    }
    pub fn len(&self) -> usize {
        self.bytes.len()
    }
    pub fn is_empty(&self) -> bool {
        self.bytes.is_empty()
    }
    pub fn finish(self) -> Vec<u8> {
        self.bytes
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn declared_and_streamed_limits_fail_before_appending_and_do_not_trust_short_lengths() {
        assert!(Buffer::new(8, Some(9)).is_err());
        let mut buffer = Buffer::new(8, Some(1)).unwrap();
        buffer.append(b"12345").unwrap();
        assert_eq!(buffer.append(b"6789"), Err(Rejection::Size));
        assert_eq!(buffer.finish(), b"12345");
        let mut buffer = Buffer::new(8, None).unwrap();
        buffer.append(b"12345678").unwrap();
        assert_eq!(buffer.append(b"9"), Err(Rejection::Size));
    }
    #[test]
    fn feed_selection_is_bounded_and_untrusted_locations_are_never_offered() {
        let fixture: serde_json::Value =
            serde_json::from_str(include_str!("../tests/fixtures/update-signing.json")).unwrap();
        let key = fixture["key_a"].as_str().unwrap();
        let mut feed = serde_json::json!({"version":"2.0.0","notes":"<script>inert text</script>","platforms":{"windows-x86_64":{"url":format!("{}/desktop-v2.0.0/{}",crate::update_policy::RELEASE_ROOT,crate::update_policy::artifact_name("2.0.0","windows-x86_64").unwrap()),"signature":fixture["artifacts"]["release"]["signature"]}}});
        assert!(
            parse(
                &serde_json::to_vec(&feed).unwrap(),
                key,
                "2.0.0",
                "windows-x86_64"
            )
            .unwrap()
            .is_none()
        );
        assert!(
            parse(
                &serde_json::to_vec(&feed).unwrap(),
                key,
                "1.0.0",
                "windows-x86_64"
            )
            .unwrap()
            .is_some()
        );
        feed["platforms"]["windows-x86_64"]["url"] = "http://127.0.0.1/package.exe".into();
        assert!(matches!(
            parse(
                &serde_json::to_vec(&feed).unwrap(),
                key,
                "1.0.0",
                "windows-x86_64"
            ),
            Err(Rejection::Location)
        ));
        assert!(matches!(
            parse(
                &vec![b' '; MAX_FEED_BYTES + 1],
                key,
                "1.0.0",
                "windows-x86_64"
            ),
            Err(Rejection::Size)
        ));
        feed["notes"] = "x".repeat(MAX_NOTES_BYTES + 1).into();
        assert!(matches!(
            parse(
                &serde_json::to_vec(&feed).unwrap(),
                key,
                "1.0.0",
                "windows-x86_64"
            ),
            Err(Rejection::Metadata)
        ));
    }
}
