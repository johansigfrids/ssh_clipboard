use arboard::Clipboard;
use eyre::{Result, eyre};

pub fn read_text() -> Result<String> {
    let mut clipboard = Clipboard::new().map_err(|err| eyre!("clipboard init failed: {err}"))?;
    clipboard
        .get_text()
        .map_err(|err| eyre!("clipboard read failed: {err}"))
}

pub fn write_text(text: &str) -> Result<()> {
    let mut clipboard = Clipboard::new().map_err(|err| eyre!("clipboard init failed: {err}"))?;
    clipboard
        .set_text(text.to_string())
        .map_err(|err| eyre!("clipboard write failed: {err}"))?;
    Ok(())
}
