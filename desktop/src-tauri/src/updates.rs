//! Update discovery/download state is independent of clipboard/session locks.
use serde::Serialize;
#[cfg(feature = "installed-desktop")]
use ssh_clipboard_desktop_prototype::{update_feed, update_network, update_policy};
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter};

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct View {
    pub available: bool,
    pub current_version: String,
    pub offered_version: Option<String>,
    pub notes: String,
    pub phase: &'static str,
    pub detail: String,
    pub busy: bool,
    pub downloaded_bytes: usize,
    pub total_bytes: Option<u64>,
}

#[derive(Default)]
struct Cache {
    #[cfg(feature = "installed-desktop")]
    release: Option<update_feed::Release>,
    #[cfg(feature = "installed-desktop")]
    verified: Option<Vec<u8>>,
}
struct Inner {
    view: Mutex<View>,
    cache: tokio::sync::Mutex<Cache>,
}
#[derive(Clone)]
pub struct Updates(Arc<Inner>);

impl Updates {
    pub fn new(version: String) -> Self {
        let available = cfg!(all(feature = "installed-desktop", not(debug_assertions)))
            && (!cfg!(target_os = "linux") || std::env::var_os("APPIMAGE").is_some());
        Self(Arc::new(Inner {
            view: Mutex::new(View {
                available, current_version: version, offered_version: None, notes: String::new(),
                phase: if available {"idle"}else{"unavailable"}, busy: false,
                detail: if available {"Updates are checked quietly. Installation always needs your choice."}else{"Updates are available in installed release builds. Linux packages use their package manager."}.into(),
                downloaded_bytes: 0, total_bytes: None,
            }), cache: tokio::sync::Mutex::new(Cache::default()),
        }))
    }
    pub fn view(&self) -> View {
        self.0
            .view
            .lock()
            .unwrap_or_else(|poison| poison.into_inner())
            .clone()
    }
    fn publish(&self, app: &AppHandle, change: impl FnOnce(&mut View)) {
        let mut view = self
            .0
            .view
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        change(&mut view);
        let _ = app.emit("update-state", &*view);
    }
    fn begin(
        &self,
        app: &AppHandle,
        phase: &'static str,
        detail: &str,
    ) -> Result<Progress, String> {
        if !self.view().available {
            return Err("Updates are unavailable in this build.".into());
        }
        if self.view().busy {
            return Err("An update operation is already running.".into());
        }
        self.publish(app, |view| {
            view.busy = true;
            view.phase = phase;
            view.detail = detail.into();
        });
        Ok(Progress {
            updates: self.clone(),
            app: app.clone(),
            finished: false,
        })
    }
    #[cfg(feature = "installed-desktop")]
    fn policy(&self) -> Result<update_policy::Policy, String> {
        update_policy::Policy::new(
            option_env!("SSH_CLIPBOARD_UPDATE_PUBLIC_KEY").unwrap_or(""),
            &self.view().current_version,
            platform(),
        )
        .map_err(|_| "This build has no valid update trust configuration.".into())
    }

    pub async fn check(&self, app: &AppHandle) -> Result<View, String> {
        let mut cache = self
            .0
            .cache
            .try_lock()
            .map_err(|_| "An update operation is already running.")?;
        let mut progress = self.begin(app, "checking", "Checking for updates…")?;
        #[cfg(feature = "installed-desktop")]
        {
            cache.release = None;
            cache.verified = None;
            self.publish(app, |view| {
                view.offered_version = None;
                view.notes.clear();
                view.downloaded_bytes = 0;
                view.total_bytes = None;
            });
            let result: Result<Option<update_feed::Release>, String> = async {
                self.policy()?;
                let feed = option_env!("SSH_CLIPBOARD_UPDATE_FEED")
                    .ok_or("This build has no update feed.")?;
                let bytes = update_network::download(
                    feed,
                    update_feed::MAX_FEED_BYTES,
                    std::time::Duration::from_secs(20),
                    |_, _| {},
                )
                .await?;
                update_feed::parse(
                    &bytes,
                    option_env!("SSH_CLIPBOARD_UPDATE_PUBLIC_KEY").unwrap_or(""),
                    &self.view().current_version,
                    platform(),
                )
                .map_err(|_| "The release metadata was rejected. Nothing was installed.".into())
            }
            .await;
            match result {
                Ok(release) => {
                    self.publish(app, |view| {
                        view.phase = if release.is_some() {
                            "offered"
                        } else {
                            "current"
                        };
                        view.offered_version = release
                            .as_ref()
                            .map(|value| value.candidate.version.clone());
                        view.notes = release
                            .as_ref()
                            .map_or_else(String::new, |value| value.notes.clone());
                        view.detail = if release.is_some() {
                            "An update is available. Download to verify it before installing."
                        } else {
                            "You have the latest available stable version."
                        }
                        .into();
                    });
                    cache.release = release;
                }
                Err(error) => {
                    progress.fail(&error);
                    return Err(error);
                }
            }
        }
        #[cfg(not(feature = "installed-desktop"))]
        {
            let _ = &mut cache;
        }
        progress.finish();
        Ok(self.view())
    }

    pub async fn download(&self, app: &AppHandle, expected: &str) -> Result<View, String> {
        let mut cache = self
            .0
            .cache
            .try_lock()
            .map_err(|_| "An update operation is already running.")?;
        let mut progress = self.begin(app, "downloading", "Downloading update…")?;
        #[cfg(feature = "installed-desktop")]
        {
            let result = async {
                let candidate = cache.release.as_ref()
                    .filter(|release| release.candidate.version == expected)
                    .ok_or("This update offer changed. Check again.")?.candidate.clone();
                cache.verified = None;
                let mut last = std::time::Instant::now();
                let bytes = update_network::download(&candidate.url, update_policy::MAX_UPDATE_BYTES,
                    std::time::Duration::from_secs(300), |received, total| {
                    if last.elapsed() >= std::time::Duration::from_millis(200) {
                        self.publish(app, |view| {
                            view.downloaded_bytes = received;
                            view.total_bytes = total;
                        });
                        last = std::time::Instant::now();
                    }
                }).await?;
                self.policy()?.verify(&candidate, &bytes)
                    .map_err(|_| "The downloaded package failed signature or release-identity verification. Nothing was installed.")?;
                self.publish(app, |view| {
                    view.phase = "ready";
                    view.detail = "Signature and release identity verified. Install when ready; the app will restart.".into();
                    view.downloaded_bytes = bytes.len();
                    view.total_bytes = Some(bytes.len() as u64);
                });
                cache.verified = Some(bytes);
                Ok::<_, String>(())
            }.await;
            if let Err(error) = result {
                progress.fail(&error);
                return Err(error);
            }
        }
        #[cfg(not(feature = "installed-desktop"))]
        {
            let _ = (&mut cache, expected);
        }
        progress.finish();
        Ok(self.view())
    }

    #[cfg(feature = "installed-desktop")]
    pub async fn take_verified(
        &self,
        app: &AppHandle,
        expected: &str,
    ) -> Result<(Progress, update_policy::Candidate, Vec<u8>), String> {
        let mut cache = self
            .0
            .cache
            .try_lock()
            .map_err(|_| "An update operation is already running.")?;
        let release = cache
            .release
            .as_ref()
            .filter(|value| value.candidate.version == expected)
            .ok_or("This update offer changed. Check again.")?;
        let candidate = release.candidate.clone();
        let bytes = cache
            .verified
            .take()
            .ok_or("Download and verify the update first.")?;
        self.policy()?
            .verify(&candidate, &bytes)
            .map_err(|_| "The package no longer satisfies update policy. Download it again.")?;
        let progress = self.begin(app, "installing", "Preparing the verified installer…")?;
        Ok((progress, candidate, bytes))
    }
}

pub struct Progress {
    updates: Updates,
    app: AppHandle,
    finished: bool,
}
impl Progress {
    pub fn finish(&mut self) {
        self.finished = true;
        self.updates.publish(&self.app, |view| view.busy = false);
    }
    #[cfg(feature = "installed-desktop")]
    pub fn fail(&mut self, error: &str) {
        self.finished = true;
        self.updates.publish(&self.app, |view| {
            view.busy = false;
            view.phase = "failed";
            view.detail = error.into();
        });
    }
}
impl Drop for Progress {
    fn drop(&mut self) {
        if !self.finished {
            self.updates.publish(&self.app, |view| {
                view.busy = false;
                view.phase = "failed";
                view.downloaded_bytes = 0;
                view.total_bytes = None;
                view.detail =
                    "Update operation stopped. Nothing was installed. Check or download again."
                        .into();
            });
        }
    }
}

#[cfg(feature = "installed-desktop")]
pub fn platform() -> &'static str {
    match (std::env::consts::OS, std::env::consts::ARCH) {
        ("windows", "x86_64") => "windows-x86_64",
        ("windows", "aarch64") => "windows-aarch64",
        ("macos", "x86_64") => "darwin-x86_64",
        ("macos", "aarch64") => "darwin-aarch64",
        ("linux", "x86_64") => "linux-x86_64",
        ("linux", "aarch64") => "linux-aarch64",
        _ => "unsupported",
    }
}
