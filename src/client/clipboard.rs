use arboard::{Clipboard, ImageData};
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

pub fn read_image() -> Result<ImageData<'static>> {
    let mut clipboard = Clipboard::new().map_err(|err| eyre!("clipboard init failed: {err}"))?;
    let image = clipboard
        .get_image()
        .map_err(|err| eyre!("clipboard image read failed: {err}"))?;
    Ok(ImageData {
        width: image.width,
        height: image.height,
        bytes: image.bytes.into_owned().into(),
    })
}

pub fn write_image(image: ImageData<'static>) -> Result<()> {
    let mut clipboard = Clipboard::new().map_err(|err| eyre!("clipboard init failed: {err}"))?;
    clipboard
        .set_image(image)
        .map_err(|err| eyre!("clipboard image write failed: {err}"))?;
    Ok(())
}
