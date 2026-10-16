//! Build/release verification only. This tool never installs or executes a package.
use ssh_clipboard_desktop_prototype::update_policy::{Candidate, MAX_UPDATE_BYTES, Policy};
use std::io::Read;

fn bounded(path: &std::ffi::OsStr, limit: usize) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let mut bytes = Vec::new();
    std::fs::File::open(path)?
        .take(limit as u64 + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() > limit {
        return Err("verification input exceeds its size limit".into());
    }
    Ok(bytes)
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = std::env::args_os().skip(1).collect::<Vec<_>>();
    if args.len() != 4 {
        return Err(
            "usage: verify_desktop_update PUBLIC_KEY_FILE CURRENT_VERSION CANDIDATE_JSON PACKAGE"
                .into(),
        );
    }
    let key = bounded(&args[0], 4096)?;
    let candidate: Candidate = serde_json::from_slice(&bounded(&args[2], 32768)?)?;
    let policy = Policy::new(
        std::str::from_utf8(&key)?,
        args[1].to_str().ok_or("invalid current version")?,
        &candidate.platform,
    )
    .map_err(|error| format!("invalid verification policy: {error:?}"))?;
    policy
        .verify(&candidate, &bounded(&args[3], MAX_UPDATE_BYTES)?)
        .map_err(|error| format!("desktop package rejected: {error:?}"))?;
    println!("Verified signed desktop package and authenticated release identity.");
    Ok(())
}
