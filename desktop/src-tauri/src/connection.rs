use serde::{Deserialize, Serialize};
use ssh_clipboard::configuration::TransportSettings;

/// Decimal strings preserve all saved integer values across the JavaScript boundary.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TransportDraft {
    pub ssh_options: Vec<String>,
    pub ssh_binary: String,
    pub timeout_ms: String,
}

impl Default for TransportDraft {
    fn default() -> Self {
        Self::from(&TransportSettings::default())
    }
}

impl From<&TransportSettings> for TransportDraft {
    fn from(settings: &TransportSettings) -> Self {
        Self {
            ssh_options: settings.ssh_options.clone(),
            ssh_binary: settings
                .ssh_bin
                .as_ref()
                .map(|p| p.to_string_lossy().into_owned())
                .unwrap_or_default(),
            timeout_ms: settings.timeout_ms.to_string(),
        }
    }
}

fn integer<T: std::str::FromStr>(text: &str, field: &str) -> Result<T, String> {
    if text.is_empty() || !text.bytes().all(|b| b.is_ascii_digit()) {
        return Err(format!("{field} must be a positive whole number."));
    }
    text.parse()
        .map_err(|_| format!("{field} is too large for this computer."))
}

impl TransportDraft {
    pub fn to_shared(&self) -> Result<TransportSettings, String> {
        let settings = TransportSettings {
            ssh_options: self.ssh_options.clone(),
            ssh_bin: (!self.ssh_binary.is_empty()).then(|| self.ssh_binary.clone().into()),
            timeout_ms: integer(&self.timeout_ms, "Timeout")?,
        };
        settings.validate()?;
        Ok(settings)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn advanced_values_round_trip_without_javascript_integer_rounding() {
        let saved = TransportSettings {
            ssh_options: vec![
                "ProxyJump=jump-host".into(),
                "ServerAliveInterval=20".into(),
            ],
            ssh_bin: Some("path with spaces/ssh".into()),
            timeout_ms: u64::MAX,
        };
        let draft = TransportDraft::from(&saved);
        assert_eq!(draft.timeout_ms, "18446744073709551615");
        assert_eq!(draft.to_shared().unwrap(), saved);
    }

    #[test]
    fn invalid_policy_is_rejected_before_it_can_reach_the_store_or_ssh() {
        for invalid in ["", "0", "-1", "1.5", "1e3", " 7000", "18446744073709551616"] {
            let draft = TransportDraft {
                timeout_ms: invalid.into(),
                ..Default::default()
            };
            assert!(draft.to_shared().is_err(), "{invalid:?}");
        }
        let draft = TransportDraft {
            ssh_options: vec!["Option=one\nother".into()],
            ..Default::default()
        };
        assert!(draft.to_shared().unwrap_err().contains("one line"));
    }
}
