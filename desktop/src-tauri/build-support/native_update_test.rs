//! Compile-time-only transport fixture for the separate native acceptance app.
pub const IDENTIFIER: &str = "io.github.johansigfrids.sshclipboard.update-test";
pub const PRODUCT: &str = "SSH Clipboard Update Test";

pub fn validate(
    origin: &str,
    identifier: &str,
    product: &str,
    installed: bool,
) -> Result<(), &'static str> {
    if identifier != IDENTIFIER || product != PRODUCT || !installed {
        return Err("native updater fixtures require the isolated installed test identity");
    }
    let (port, token) = origin
        .strip_prefix("http://127.0.0.1:")
        .and_then(|tail| tail.split_once('/'))
        .ok_or("invalid native test origin")?;
    let number: u16 = port.parse().map_err(|_| "invalid native test port")?;
    if number < 1024
        || number.to_string() != port
        || token.len() != 32
        || !token
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err("invalid native test origin");
    }
    Ok(())
}

pub fn resolve(origin: &str, source: &str) -> Result<String, &'static str> {
    validate(origin, IDENTIFIER, PRODUCT, true)?;
    let path = source
        .strip_prefix("https://github.com/johansigfrids/ssh_clipboard/releases/")
        .ok_or("native fixture refuses non-canonical update sources")?;
    if !(path.starts_with("latest/download/desktop-stable-")
        || path.starts_with("download/desktop-v"))
        || path.contains("..")
        || !path
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"/._-".contains(&byte))
    {
        return Err("native fixture refuses non-canonical update sources");
    }
    Ok(format!("{origin}/{path}"))
}
