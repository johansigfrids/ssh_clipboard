//! Verify exact package bytes and authenticated version/platform/channel bindings.
use base64::{Engine, engine::general_purpose::STANDARD};
use minisign_verify::{PublicKey, Signature};
use semver::Version;

pub const MAX_UPDATE_BYTES: usize = 256 * 1024 * 1024;
pub const CHANNEL: &str = "stable";
pub const RELEASE_ROOT: &str = "https://github.com/johansigfrids/ssh_clipboard/releases/download";

#[derive(Debug, PartialEq, Eq)]
pub enum Rejection {
    Key,
    Metadata,
    Version,
    Platform,
    Location,
    Size,
    Signature,
    Binding,
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Candidate {
    pub version: String,
    pub platform: String,
    pub url: String,
    pub signature: String,
}

pub struct Policy {
    key: PublicKey,
    current: Version,
    platform: String,
}

pub fn artifact_name(version: &str, platform: &str) -> Result<String, Rejection> {
    if version.len() > 64 {
        return Err(Rejection::Version);
    }
    let parsed = Version::parse(version).map_err(|_| Rejection::Version)?;
    if !parsed.pre.is_empty() || !parsed.build.is_empty() || parsed.to_string() != version {
        return Err(Rejection::Version);
    }
    let extension = match platform {
        "windows-x86_64" | "windows-aarch64" => "-setup.exe",
        "darwin-x86_64" | "darwin-aarch64" => ".app.tar.gz",
        "linux-x86_64" | "linux-aarch64" => ".AppImage",
        _ => return Err(Rejection::Platform),
    };
    Ok(format!(
        "SSHClipboard-{CHANNEL}-{platform}-{version}{extension}"
    ))
}

impl Policy {
    pub fn new(public_key: &str, current: &str, platform: &str) -> Result<Self, Rejection> {
        if public_key.len() > 4096 {
            return Err(Rejection::Key);
        }
        let key_text = STANDARD
            .decode(public_key.trim())
            .map_err(|_| Rejection::Key)?;
        let key_text = std::str::from_utf8(&key_text).map_err(|_| Rejection::Key)?;
        let key = PublicKey::decode(key_text).map_err(|_| Rejection::Key)?;
        let current = Version::parse(current).map_err(|_| Rejection::Version)?;
        artifact_name("1.0.0", platform)?;
        Ok(Self {
            key,
            current,
            platform: platform.into(),
        })
    }

    /// This checks untrusted selectors only. Acceptance still requires verify(bytes).
    pub fn check_candidate(&self, candidate: &Candidate) -> Result<(), Rejection> {
        if candidate.platform != self.platform {
            return Err(Rejection::Platform);
        }
        let name = artifact_name(&candidate.version, &candidate.platform)?;
        if Version::parse(&candidate.version).map_err(|_| Rejection::Version)? <= self.current {
            return Err(Rejection::Version);
        }
        let expected = format!("{RELEASE_ROOT}/desktop-v{}/{name}", candidate.version);
        if candidate.url != expected {
            return Err(Rejection::Location);
        }
        let url = url::Url::parse(&candidate.url).map_err(|_| Rejection::Location)?;
        if url.scheme() != "https"
            || url.host_str() != Some("github.com")
            || !url.username().is_empty()
            || url.password().is_some()
            || url.query().is_some()
            || url.fragment().is_some()
        {
            return Err(Rejection::Location);
        }
        if candidate.signature.is_empty() || candidate.signature.len() > 16384 {
            return Err(Rejection::Metadata);
        }
        Ok(())
    }

    pub fn verify(&self, candidate: &Candidate, bytes: &[u8]) -> Result<(), Rejection> {
        self.check_candidate(candidate)?;
        if bytes.is_empty() || bytes.len() > MAX_UPDATE_BYTES {
            return Err(Rejection::Size);
        }
        let decoded = STANDARD
            .decode(candidate.signature.trim())
            .map_err(|_| Rejection::Signature)?;
        let decoded = std::str::from_utf8(&decoded).map_err(|_| Rejection::Signature)?;
        let signature = Signature::decode(decoded).map_err(|_| Rejection::Signature)?;
        self.key
            .verify(bytes, &signature, true)
            .map_err(|_| Rejection::Signature)?;
        // Only a verified global signature authenticates the trusted comment.
        let comment = signature.trusted_comment();
        let field = |name: &str| -> Result<&str, Rejection> {
            let mut values = comment
                .split('\t')
                .filter_map(|field| field.strip_prefix(name));
            let value = values.next().ok_or(Rejection::Binding)?;
            if values.next().is_some() {
                return Err(Rejection::Binding);
            }
            Ok(value)
        };
        if field("version:")? != candidate.version
            || field("file:")? != artifact_name(&candidate.version, &self.platform)?
        {
            return Err(Rejection::Binding);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> serde_json::Value {
        serde_json::from_str(include_str!("../tests/fixtures/update-signing.json")).unwrap()
    }
    fn fixture_candidate(fixture: &serde_json::Value, name: &str) -> (Candidate, Vec<u8>) {
        let value = &fixture["artifacts"][name];
        let version = value["version"].as_str().unwrap();
        let file = value["file"].as_str().unwrap();
        (
            Candidate {
                version: version.into(),
                platform: "windows-x86_64".into(),
                url: format!("{RELEASE_ROOT}/desktop-v{version}/{file}"),
                signature: value["signature"].as_str().unwrap().into(),
            },
            STANDARD.decode(value["bytes"].as_str().unwrap()).unwrap(),
        )
    }
    #[test]
    fn authentic_package_is_bound_to_version_platform_and_channel() {
        let fixture = fixture();
        let key = fixture["key_a"].as_str().unwrap();
        let policy = Policy::new(key, "1.0.0", "windows-x86_64").unwrap();
        let (mut candidate, bytes) = fixture_candidate(&fixture, "release");
        policy.verify(&candidate, &bytes).unwrap();
        candidate.version = "9.0.0".into();
        candidate.url = format!(
            "{RELEASE_ROOT}/desktop-v9.0.0/{}",
            artifact_name("9.0.0", "windows-x86_64").unwrap()
        );
        assert_eq!(policy.verify(&candidate, &bytes), Err(Rejection::Binding));
        let (unversioned, bytes) = fixture_candidate(&fixture, "unversioned_signature");
        assert_eq!(policy.verify(&unversioned, &bytes), Err(Rejection::Binding));
    }
    #[test]
    fn changed_bytes_wrong_key_missing_signature_and_replays_are_rejected() {
        let fixture = fixture();
        let policy = Policy::new(
            fixture["key_a"].as_str().unwrap(),
            "1.0.0",
            "windows-x86_64",
        )
        .unwrap();
        let (mut candidate, mut bytes) = fixture_candidate(&fixture, "release");
        bytes[0] ^= 1;
        assert_eq!(policy.verify(&candidate, &bytes), Err(Rejection::Signature));
        bytes[0] ^= 1;
        let wrong = Policy::new(
            fixture["key_b"].as_str().unwrap(),
            "1.0.0",
            "windows-x86_64",
        )
        .unwrap();
        assert_eq!(wrong.verify(&candidate, &bytes), Err(Rejection::Signature));
        let current = Policy::new(
            fixture["key_a"].as_str().unwrap(),
            "2.0.0",
            "windows-x86_64",
        )
        .unwrap();
        assert_eq!(current.verify(&candidate, &bytes), Err(Rejection::Version));
        candidate.signature.clear();
        assert_eq!(policy.verify(&candidate, &bytes), Err(Rejection::Metadata));
    }
    #[test]
    fn signed_other_channel_or_platform_cannot_be_relabelled() {
        let fixture = fixture();
        let policy = Policy::new(
            fixture["key_a"].as_str().unwrap(),
            "1.0.0",
            "windows-x86_64",
        )
        .unwrap();
        let (mut candidate, bytes) = fixture_candidate(&fixture, "beta");
        candidate.url = format!(
            "{RELEASE_ROOT}/desktop-v2.0.0/{}",
            artifact_name("2.0.0", "windows-x86_64").unwrap()
        );
        assert_eq!(policy.verify(&candidate, &bytes), Err(Rejection::Binding));
        let (mut candidate, bytes) = fixture_candidate(&fixture, "other_platform");
        candidate.url = format!(
            "{RELEASE_ROOT}/desktop-v2.0.0/{}",
            artifact_name("2.0.0", "windows-x86_64").unwrap()
        );
        assert_eq!(policy.verify(&candidate, &bytes), Err(Rejection::Binding));
    }
    #[test]
    fn old_key_trusts_a_transition_payload_that_contains_the_new_key() {
        let fixture = fixture();
        let old = Policy::new(
            fixture["key_a"].as_str().unwrap(),
            "1.0.0",
            "windows-x86_64",
        )
        .unwrap();
        let (transition, bytes) = fixture_candidate(&fixture, "transition");
        old.verify(&transition, &bytes).unwrap();
        assert!(
            std::str::from_utf8(&bytes)
                .unwrap()
                .contains(fixture["key_b"].as_str().unwrap())
        );
        let new = Policy::new(
            fixture["key_b"].as_str().unwrap(),
            "2.0.0",
            "windows-x86_64",
        )
        .unwrap();
        let (next, bytes) = fixture_candidate(&fixture, "next");
        new.verify(&next, &bytes).unwrap();
        assert_eq!(old.verify(&next, &bytes), Err(Rejection::Signature));
    }
}
