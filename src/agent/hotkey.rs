use eyre::{Result, eyre};
use global_hotkey::hotkey::HotKey;

pub fn parse_hotkey(binding: &str) -> Result<HotKey> {
    binding
        .parse::<HotKey>()
        .map_err(|err| eyre!("invalid hotkey binding `{binding}`: {err}"))
}
