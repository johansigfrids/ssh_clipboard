//! Authenticate raw Linux server packages before staging or execution.
use crate::server_releases::{Verifier, validate_version};
use base64::{Engine, engine::general_purpose::STANDARD};
use eyre::{Result, WrapErr, bail};
use minisign_verify::{PublicKey, Signature};

pub const MAX_PACKAGE_BYTES: usize = 256 * 1024 * 1024;

pub fn native_platform() -> Result<&'static str> {
    match std::env::consts::ARCH {
        "x86_64" => Ok("linux-x86_64"),
        "aarch64" => Ok("linux-aarch64"),
        _ => bail!("this Linux architecture has no server update package"),
    }
}

pub fn artifact_name(version: &str, platform: &str) -> Result<String> {
    validate_version(version)?;
    if !matches!(platform, "linux-x86_64" | "linux-aarch64") {
        bail!("unsupported server package platform");
    }
    Ok(format!("SSHClipboardServer-stable-{platform}-{version}"))
}

pub struct Policy {
    key: PublicKey,
    platform: String,
}
impl Policy {
    pub fn new(public_key: &str, platform: &str) -> Result<Self> {
        if public_key.is_empty() || public_key.len() > 4096 {
            bail!("invalid server signing key size");
        }
        artifact_name("1.0.0", platform)?;
        let decoded = STANDARD
            .decode(public_key.trim())
            .wrap_err("invalid server signing key encoding")?;
        let key = PublicKey::decode(std::str::from_utf8(&decoded)?)
            .wrap_err("invalid server signing key")?;
        Ok(Self {
            key,
            platform: platform.into(),
        })
    }
}

impl Verifier for Policy {
    fn verify(&self, version: &str, signature: &str, bytes: &[u8]) -> Result<()> {
        let filename = artifact_name(version, &self.platform)?;
        if bytes.len() < 64
            || bytes.len() > MAX_PACKAGE_BYTES
            || signature.is_empty()
            || signature.len() > 16384
        {
            bail!("invalid server package or signature size");
        }
        let decoded = STANDARD
            .decode(signature.trim())
            .wrap_err("invalid server signature encoding")?;
        let signature = Signature::decode(std::str::from_utf8(&decoded)?)
            .wrap_err("invalid server signature")?;
        self.key
            .verify(bytes, &signature, true)
            .wrap_err("server package signature rejected")?;
        // Global verification above authenticates both trusted fields. Never parse them first.
        let field = |prefix: &str| -> Result<&str> {
            let mut fields = signature
                .trusted_comment()
                .split('\t')
                .filter_map(|part| part.strip_prefix(prefix));
            let value = fields
                .next()
                .ok_or_else(|| eyre::eyre!("missing signed server package identity"))?;
            if fields.next().is_some() {
                bail!("duplicate signed server package identity");
            }
            Ok(value)
        };
        if field("version:")? != version || field("file:")? != filename {
            bail!("signed server version/platform/channel does not match");
        }
        // Signatures identify the build; also reject a signed package unusable on this host.
        let machine = match self.platform.as_str() {
            "linux-x86_64" => 62,
            "linux-aarch64" => 183,
            _ => unreachable!(),
        };
        if &bytes[..4] != b"\x7fELF"
            || bytes[4] != 2
            || bytes[5] != 1
            || bytes[6] != 1
            || !matches!(u16::from_le_bytes([bytes[16], bytes[17]]), 2 | 3)
            || u16::from_le_bytes([bytes[18], bytes[19]]) != machine
            || u16::from_le_bytes([bytes[52], bytes[53]]) != 64
        {
            bail!("signed package is not a matching Linux ELF executable");
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> serde_json::Value {
        serde_json::from_str(include_str!("../tests/fixtures/server-signing.json")).unwrap()
    }
    fn artifact(fixture: &serde_json::Value, name: &str) -> (String, String, Vec<u8>) {
        let entry = &fixture["artifacts"][name];
        (
            entry["version"].as_str().unwrap().into(),
            entry["signature"].as_str().unwrap().into(),
            STANDARD.decode(entry["bytes"].as_str().unwrap()).unwrap(),
        )
    }
    #[test]
    fn authentic_server_packages_are_bound_to_platform_version_and_channel() {
        let fixture = fixture();
        let key = fixture["key_a"].as_str().unwrap();
        let policy = Policy::new(key, "linux-x86_64").unwrap();
        let (version, signature, bytes) = artifact(&fixture, "release");
        policy.verify(&version, &signature, &bytes).unwrap();
        assert!(policy.verify("9.0.0", &signature, &bytes).is_err());
        let (version, signature, bytes) = artifact(&fixture, "beta");
        assert!(policy.verify(&version, &signature, &bytes).is_err());
        let (version, signature, bytes) = artifact(&fixture, "arm");
        assert!(policy.verify(&version, &signature, &bytes).is_err());
        Policy::new(key, "linux-aarch64")
            .unwrap()
            .verify(&version, &signature, &bytes)
            .unwrap();
        let (version, signature, bytes) = artifact(&fixture, "unversioned");
        assert!(policy.verify(&version, &signature, &bytes).is_err());
    }
    #[test]
    fn altered_bytes_wrong_keys_missing_signatures_and_wrong_formats_are_rejected() {
        let fixture = fixture();
        let policy = Policy::new(fixture["key_a"].as_str().unwrap(), "linux-x86_64").unwrap();
        let (version, signature, mut bytes) = artifact(&fixture, "release");
        bytes[64] ^= 1;
        assert!(policy.verify(&version, &signature, &bytes).is_err());
        bytes[64] ^= 1;
        assert!(
            Policy::new(fixture["key_b"].as_str().unwrap(), "linux-x86_64")
                .unwrap()
                .verify(&version, &signature, &bytes)
                .is_err()
        );
        assert!(policy.verify(&version, "", &bytes).is_err());
        let (version, signature, bytes) = artifact(&fixture, "wrong_elf");
        assert!(policy.verify(&version, &signature, &bytes).is_err());
    }
    #[test]
    fn crypto_admission_and_reverified_rollback_join_without_executing_synthetic_bytes() {
        use crate::server_releases::{Outcome, Service, Store};
        use std::os::unix::fs::PermissionsExt;
        struct Fake {
            failed: bool,
        }
        impl Service for Fake {
            fn restart(&mut self) -> Result<()> {
                Ok(())
            }
            fn health(&mut self) -> Result<()> {
                if !self.failed {
                    self.failed = true;
                    bail!("synthetic health failure");
                }
                Ok(())
            }
            fn stop(&mut self) -> Result<()> {
                Ok(())
            }
        }
        let fixture = fixture();
        let policy = Policy::new(fixture["key_a"].as_str().unwrap(), "linux-x86_64").unwrap();
        let directory = tempfile::tempdir().unwrap();
        std::fs::set_permissions(directory.path(), std::fs::Permissions::from_mode(0o700)).unwrap();
        let store = Store::open(&directory.path().join("managed")).unwrap();
        for name in ["old", "release"] {
            let (version, signature, bytes) = artifact(&fixture, name);
            store.stage(&version, &signature, &bytes, &policy).unwrap();
        }
        let mut service = Fake { failed: true };
        assert_eq!(
            store.activate("1.0.0", &policy, &mut service).unwrap(),
            Outcome::Activated
        );
        service.failed = false;
        assert_eq!(
            store.activate("2.0.0", &policy, &mut service).unwrap(),
            Outcome::RestoredPrevious
        );
        assert_eq!(store.active().unwrap().as_deref(), Some("1.0.0"));
        assert!(!store.needs_recovery().unwrap());
    }
}
