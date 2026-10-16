#[cfg(windows)]
fn main() {
    use std::path::Path;

    if std::env::var_os("CARGO_FEATURE_EXTERNAL_RESOURCE_OWNER").is_some() {
        return;
    }

    let icon_path = Path::new("assets").join("icon.ico");
    if !icon_path.exists() {
        println!(
            "cargo:warning=icon not found at {}; Windows exe will use default icon",
            icon_path.display()
        );
        return;
    }

    winres::WindowsResource::new()
        .set_icon(icon_path.to_str().unwrap())
        .compile()
        .expect("failed to embed Windows icon");
}

#[cfg(not(windows))]
fn main() {}
