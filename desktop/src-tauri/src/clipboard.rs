use crate::clipboard_payload::{ClipboardAccess, apply_payload, build_text_value};
use arboard::Clipboard;
use eyre::Result;
use image::ImageEncoder;
use ssh_clipboard::client::image::ImageData;
#[cfg(test)]
const DEFAULT_MAX_SIZE: usize = 64 * 1024 * 1024;
use crate::clipboard_payload::{CONTENT_TYPE_PNG, ClipboardPayload};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, mpsc};
use std::thread::JoinHandle;
use tokio::sync::oneshot;

struct NativeClipboard(Clipboard);

#[cfg(windows)]
fn native_file_list_probe() -> (bool, Option<usize>) {
    #[link(name = "user32")]
    unsafe extern "system" {
        fn IsClipboardFormatAvailable(format: u32) -> i32;
    }
    // CF_HDROP. arboard maps native parse errors to ContentNotAvailable on Windows.
    (unsafe { IsClipboardFormatAvailable(15) } != 0, None)
}
#[cfg(target_os = "macos")]
fn native_file_list_probe() -> (bool, Option<usize>) {
    use objc2_app_kit::{NSPasteboard, NSPasteboardTypeFileURL};
    objc2::rc::autoreleasepool(|_| {
        let pasteboard = NSPasteboard::generalPasteboard();
        // SAFETY: Apple's exported file-URL type is an immutable process-lifetime string.
        let file_type = unsafe { NSPasteboardTypeFileURL };
        let present = pasteboard
            .types()
            .is_some_and(|types| types.containsObject(file_type));
        let count = pasteboard.pasteboardItems().map(|items| {
            items
                .iter()
                .filter(|item| item.types().containsObject(file_type))
                .count()
        });
        (present, count)
    })
}
#[cfg(not(any(windows, target_os = "macos")))]
fn native_file_list_probe() -> (bool, Option<usize>) {
    (false, None)
}

type SharedCache = Arc<Mutex<Result<crate::file_cache::Cache, String>>>;
enum Captured {
    Value(ClipboardPayload),
    Files(Vec<std::path::PathBuf>),
}
pub enum Prepared {
    Value(ClipboardPayload, usize),
    Files(PreparedFiles),
}
pub struct PreparedFiles {
    staged: crate::file_cache::Staged,
    cache: SharedCache,
    retain: bool,
}
impl Drop for PreparedFiles {
    fn drop(&mut self) {
        if !self.retain
            && let Ok(cache) = self.cache.lock()
            && let Ok(cache) = cache.as_ref()
        {
            let _ = cache.remove(self.staged.id);
        }
    }
}
// Dropping an async waiter cancels its blocking traversal and does not free admission
// until the actual worker exits. Blocking filesystem calls themselves cannot be aborted.
struct Cancel(Arc<AtomicBool>);
impl Drop for Cancel {
    fn drop(&mut self) {
        self.0.store(true, Ordering::Release);
    }
}

fn apply_prepared(
    value: Prepared,
    clipboard: &mut impl ClipboardAccess,
    native_cache: &SharedCache,
) -> Result<Option<String>, String> {
    match value {
        Prepared::Value(value, limit) => {
            apply_payload(value, limit, clipboard)?;
            if let Ok(cache) = native_cache.lock()
                && let Ok(cache) = cache.as_ref()
            {
                let _ = cache.replaced(None);
            }
            Ok(None)
        }
        Prepared::Files(mut files) => {
            let mut expected = Vec::new();
            for path in &files.staged.files.paths {
                let metadata = std::fs::symlink_metadata(path)
                    .map_err(|_| "A staged file is no longer available.".to_string())?;
                let safe =
                    !metadata.file_type().is_symlink() && (metadata.is_file() || metadata.is_dir());
                #[cfg(windows)]
                let safe = {
                    use std::os::windows::fs::MetadataExt;
                    safe && metadata.file_attributes() & 0x400 == 0
                };
                if !safe {
                    return Err("A staged file is no longer safe to publish.".into());
                }
                expected.push(
                    path.canonicalize()
                        .map_err(|_| "Could not verify staged paths.".to_string())?,
                );
            }
            // Durable pins precede publication; native APIs can clear/partially write
            // before failing. Such a failure must not invalidate referenced paths.
            files.retain = true;
            clipboard.write_files(&files.staged.files.paths).map_err(|_| {
                "Could not publish the file clipboard. Downloaded copies were retained; try Copy again.".to_string()
            })?;
            let actual = clipboard.read_files().map_err(|_| {
                "Could not verify the file clipboard. Downloaded copies were retained.".to_string()
            })?;
            let actual = actual
                .unwrap_or_default()
                .iter()
                .map(|path| path.canonicalize())
                .collect::<std::io::Result<Vec<_>>>()
                .map_err(|_| "Could not verify native paths.".to_string())?;
            if actual != expected {
                return Err("The clipboard did not include every file. Downloaded copies were retained; try Copy again.".into());
            }
            let cache = files
                .cache
                .lock()
                .map_err(|_| "File cache unavailable.".to_string())?;
            cache
                .as_ref()
                .map_err(Clone::clone)?
                .replaced(Some(files.staged.id))
                .map_err(|_| {
                    "Files were copied, but cache cleanup could not finish.".to_string()
                })?;
            let renames = &files.staged.files.renames;
            Ok((!renames.is_empty()).then(|| {
                renames
                    .iter()
                    .take(20)
                    .map(|rename| {
                        let printable = |name: &str| {
                            name.chars()
                                .filter(|c| !c.is_control())
                                .take(100)
                                .collect::<String>()
                        };
                        format!(
                            "{} → {}",
                            printable(&rename.original),
                            printable(&rename.received)
                        )
                    })
                    .collect::<Vec<_>>()
                    .join("\n")
            }))
        }
    }
}

fn capture_native(
    clipboard: &mut impl ClipboardAccess,
    max_size: usize,
    max_decoded: usize,
) -> Result<Captured, String> {
    match clipboard.read_files().map_err(|_| {
        "Could not read the native file selection. Copy the files again.".to_string()
    })? {
        Some(paths) => {
            if paths.is_empty() {
                return Err("The native file selection is empty.".into());
            }
            Ok(Captured::Files(paths))
        }
        None => capture_with_limit(clipboard, max_size, max_decoded).map(Captured::Value),
    }
}

struct EncodedImage {
    bytes: Vec<u8>,
    maximum: usize,
}
impl std::io::Write for EncodedImage {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        if self.bytes.len().saturating_add(bytes.len()) > self.maximum {
            return Err(std::io::Error::other("encoded image exceeds item limit"));
        }
        self.bytes
            .try_reserve_exact(bytes.len())
            .map_err(|_| std::io::Error::other("image buffer allocation failed"))?;
        self.bytes.extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

#[cfg(test)]
fn capture(
    clipboard: &mut impl ClipboardAccess,
    max_size: usize,
) -> Result<ClipboardPayload, String> {
    capture_with_limit(clipboard, max_size, super::model::MAX_DECODED_BYTES)
}

fn capture_with_limit(
    clipboard: &mut impl ClipboardAccess,
    max_size: usize,
    max_decoded: usize,
) -> Result<ClipboardPayload, String> {
    match clipboard.read_text() {
        Ok(text) => {
            if text.len() > max_size {
                return Err(format!(
                    "This text is {:.2} MiB. The transfer limit is {:.2} MiB ({max_size} bytes). Copy less text and try again.",
                    text.len() as f64 / 1048576.0,
                    max_size as f64 / 1048576.0
                ));
            }
            build_text_value(text, max_size)
        }
        Err(text_error) => {
            let image = clipboard.read_image().map_err(|image_error| format!(
                "Could not read text or an image from the clipboard (text: {text_error}; image: {image_error})."))?;
            let decoded_bytes = image
                .width
                .checked_mul(image.height)
                .and_then(|pixels| pixels.checked_mul(4))
                .ok_or_else(|| "The clipboard image dimensions are too large.".to_string())?;
            if decoded_bytes > max_decoded {
                return Err(format!(
                    "Image {} x {} needs {:.2} MiB of pixels, above the {} MiB image limit. Copy a smaller image and try again.",
                    image.width,
                    image.height,
                    decoded_bytes as f64 / 1048576.0,
                    max_decoded / 1048576
                ));
            }
            if image.width == 0 || image.height == 0 || image.bytes.len() != decoded_bytes {
                return Err("The clipboard image has an invalid pixel buffer.".into());
            }
            let width = u32::try_from(image.width).map_err(|_| "Invalid image width.")?;
            let height = u32::try_from(image.height).map_err(|_| "Invalid image height.")?;
            let mut png = EncodedImage {
                bytes: Vec::new(),
                maximum: max_size,
            };
            // Preserve every RGBA pixel; prefer balanced compression over the fast default.
            image::codecs::png::PngEncoder::new_with_quality(
                &mut png,
                image::codecs::png::CompressionType::Default,
                image::codecs::png::FilterType::Adaptive,
            )
            .write_image(&image.bytes, width, height, image::ExtendedColorType::Rgba8)
            .map_err(|_| format!("Image {width} x {height} could not fit within the {:.2} MiB PNG transfer limit. Copy a smaller image or adjust this connection's limit.", max_size as f64 / 1048576.0))?;
            let png = png.bytes;
            if png.len() > max_size {
                return Err(format!(
                    "Image {width} x {height} is {:.2} MiB as PNG. The transfer limit is {:.2} MiB ({max_size} bytes). Copy a smaller image and try again.",
                    png.len() as f64 / 1048576.0,
                    max_size as f64 / 1048576.0
                ));
            }
            Ok(ClipboardPayload {
                content_type: CONTENT_TYPE_PNG,
                data: png,
                created_at: std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_millis() as i64,
            })
        }
    }
}

impl ClipboardAccess for NativeClipboard {
    fn read_files(&mut self) -> Result<Option<Vec<std::path::PathBuf>>> {
        let (present, count) = native_file_list_probe();
        match self.0.get().file_list() {
            Ok(paths) => {
                if present && count.is_some_and(|count| count != paths.len()) {
                    eyre::bail!("The native file list contains unreadable entries.");
                }
                Ok(Some(paths))
            }
            Err(arboard::Error::ContentNotAvailable) if !present => Ok(None),
            Err(error) => Err(error.into()),
        }
    }
    fn write_files(&mut self, paths: &[std::path::PathBuf]) -> Result<()> {
        // arboard's Windows file-list setter does not clear old clipboard formats.
        // Remove stale cut/move intent before publishing these new copy references.
        self.0.clear()?;
        Ok(self.0.set().file_list(paths)?)
    }
    fn read_text(&mut self) -> Result<String> {
        Ok(self.0.get_text()?)
    }
    fn read_image(&mut self) -> Result<ImageData<'static>> {
        let image = self.0.get_image()?;
        Ok(ImageData {
            width: image.width,
            height: image.height,
            bytes: image.bytes.into_owned().into(),
        })
    }
    fn write_text(&mut self, text: &str) -> Result<()> {
        Ok(self.0.set_text(text)?)
    }
    fn write_image(&mut self, image: ImageData<'static>) -> Result<()> {
        Ok(self.0.set_image(arboard::ImageData {
            width: image.width,
            height: image.height,
            bytes: image.bytes,
        })?)
    }
}

// The owner thread, rather than its async caller, releases admission. Dropping a
// cancelled caller must not allow a later clipboard write to queue behind its read.
struct Permit(Arc<AtomicBool>);

impl Drop for Permit {
    fn drop(&mut self) {
        self.0.store(false, Ordering::Release);
    }
}

enum Job {
    Read(
        usize,
        usize,
        oneshot::Sender<Result<(Captured, Permit), String>>,
        Permit,
    ),
    Write(
        Prepared,
        oneshot::Sender<Result<Option<String>, String>>,
        Permit,
    ),
    Stop,
}

pub struct ClipboardWorker {
    sender: mpsc::SyncSender<Job>,
    thread: Mutex<Option<JoinHandle<()>>>,
    busy: Arc<AtomicBool>,
    cache: SharedCache,
}

impl ClipboardWorker {
    pub fn start() -> Result<Self> {
        let worker = Self::start_with(|| Ok(NativeClipboard(Clipboard::new()?)))?;
        let cache = (|| -> Result<_> {
            let store = crate::configuration::store()?;
            let path = store.path();
            let name = path
                .file_name()
                .ok_or_else(|| eyre::eyre!("Missing settings filename."))?
                .to_string_lossy();
            let parent = path
                .parent()
                .ok_or_else(|| eyre::eyre!("Missing settings directory."))?;
            std::fs::create_dir_all(parent)?;
            crate::file_cache::Cache::open(parent.join(format!("{name}.received-files")))
        })()
        .map_err(|_| {
            "The received-file cache is unavailable. Check local storage and restart the app."
                .to_string()
        });
        *worker
            .cache
            .lock()
            .map_err(|_| eyre::eyre!("File cache unavailable."))? = cache;
        // The native owner already shares this same cache object.
        Ok(worker)
    }

    pub(crate) fn start_with<C: ClipboardAccess + 'static>(
        mut create: impl FnMut() -> Result<C> + Send + 'static,
    ) -> Result<Self> {
        let (sender, receiver) = mpsc::sync_channel(1);
        let cache: SharedCache = Arc::new(Mutex::new(Err(
            "Received-file cache is unavailable.".into()
        )));
        let native_cache = cache.clone();
        let thread = std::thread::Builder::new()
            .name("clipboard-owner".into())
            .spawn(move || {
                let mut clipboard = None;
                while let Ok(job) = receiver.recv() {
                    if matches!(job, Job::Stop) {
                        break;
                    }
                    // Create once on demand; keep ownership after the panel is hidden.
                    let init = if clipboard.is_none() {
                        create().map(|native| {
                            clipboard = Some(native);
                        })
                    } else {
                        Ok(())
                    };
                    match job {
                        Job::Read(max_size, max_decoded, reply, permit) => {
                            let result = init
                                .map_err(|err| format!("Clipboard unavailable: {err}"))
                                .and_then(|()| {
                                    capture_native(
                                        clipboard.as_mut().unwrap(),
                                        max_size,
                                        max_decoded,
                                    )
                                });
                            let _ = reply.send(result.map(|value| (value, permit)));
                        }
                        Job::Write(value, reply, permit) => {
                            let result = init
                                .map_err(|err| format!("Clipboard unavailable: {err}"))
                                .and_then(|()| {
                                    apply_prepared(
                                        value,
                                        clipboard.as_mut().unwrap(),
                                        &native_cache,
                                    )
                                });
                            drop(permit);
                            let _ = reply.send(result);
                        }
                        Job::Stop => break,
                    }
                }
                // Native clipboard is dropped on its owner thread at exit.
            })?;
        Ok(Self {
            sender,
            thread: Mutex::new(Some(thread)),
            busy: Arc::new(AtomicBool::new(false)),
            cache,
        })
    }

    fn reserve(&self) -> Result<Permit, String> {
        self.busy
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .map(|_| Permit(self.busy.clone()))
            .map_err(|_| {
                "Clipboard worker is still finishing an operation. Try again when it finishes."
                    .into()
            })
    }

    pub async fn read_with_limit(
        &self,
        max_size: usize,
        max_decoded: usize,
    ) -> Result<ClipboardPayload, String> {
        let permit = self.reserve()?;
        let (reply, receive) = oneshot::channel();
        self.sender
            .try_send(Job::Read(max_size, max_decoded, reply, permit))
            .map_err(|_| "Clipboard worker is busy or unavailable.".to_string())?;
        let (captured, permit) = receive
            .await
            .map_err(|_| "Clipboard worker stopped.".to_string())??;
        match captured {
            Captured::Value(value) => {
                drop(permit);
                Ok(value)
            }
            Captured::Files(paths) => {
                let cancel = Cancel(Arc::new(AtomicBool::new(false)));
                let cancelled = cancel.0.clone();
                tokio::task::spawn_blocking(move || {
                    let _permit = permit;
                    let data = ssh_clipboard::files::capture(&paths, max_size, &cancelled)
                        .map_err(|e| e.to_string())?;
                    Ok(ClipboardPayload {
                        content_type: ssh_clipboard::session_protocol::ContentType::Files,
                        data,
                        created_at: std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap_or_default()
                            .as_millis() as i64,
                    })
                })
                .await
                .map_err(|_| "File capture worker stopped.".to_string())?
            }
        }
    }

    pub async fn prepare(
        &self,
        value: ClipboardPayload,
        max_decoded: usize,
    ) -> Result<Prepared, String> {
        if value.content_type != ssh_clipboard::session_protocol::ContentType::Files {
            return Ok(Prepared::Value(value, max_decoded));
        }
        let permit = self.reserve()?;
        let cache = self.cache.clone();
        let cancel = Cancel(Arc::new(AtomicBool::new(false)));
        let cancelled = cancel.0.clone();
        tokio::task::spawn_blocking(move || {
            let _permit=permit;
            let staged=cache.lock().map_err(|_|"File cache unavailable.".to_string())?.as_ref().map_err(Clone::clone)?.stage(&value.data,&cancelled).map_err(|_|"Could not stage received files. Check free space, the cache quota, and filename/path limits.".to_string())?;
            Ok(Prepared::Files(PreparedFiles{staged,cache,retain:false}))
        }).await.map_err(|_|"File staging worker stopped.".to_string())?
    }

    pub async fn write_prepared(&self, value: Prepared) -> Result<Option<String>, String> {
        let permit = self.reserve()?;
        let (reply, receive) = oneshot::channel();
        self.sender
            .try_send(Job::Write(value, reply, permit))
            .map_err(|_| "Clipboard worker is busy or unavailable.".to_string())?;
        receive
            .await
            .map_err(|_| "Clipboard worker stopped.".to_string())?
    }

    #[cfg(test)]
    pub async fn write_with_limit(
        &self,
        value: ClipboardPayload,
        max_decoded: usize,
    ) -> Result<(), String> {
        self.write_prepared(self.prepare(value, max_decoded).await?)
            .await
            .map(|_| ())
    }

    pub fn is_busy(&self) -> bool {
        self.busy.load(Ordering::Acquire)
    }

    pub fn stop(&self) {
        if let Ok(mut thread) = self.thread.lock()
            && let Some(thread) = thread.take()
        {
            let _ = self.sender.send(Job::Stop);
            let _ = thread.join();
        }
    }
}

impl Drop for ClipboardWorker {
    fn drop(&mut self) {
        self.stop();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{
        Arc,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    };

    /// Opt-in real clipboard probe, restricted to synthetic files in an owned fixture.
    /// It deliberately survives process exit so Explorer/Finder can paste afterwards.
    #[tokio::test]
    #[ignore = "requires explicit native clipboard acceptance and owned fixture environment"]
    async fn native_file_clipboard_probe() {
        use std::path::PathBuf;
        let root = PathBuf::from(
            std::env::var_os("SSH_CLIPBOARD_NATIVE_FILES_ROOT").expect("owned fixture root"),
        )
        .canonicalize()
        .unwrap();
        assert_eq!(
            std::fs::read(root.join("owner")).unwrap(),
            b"ssh-clipboard-native-file-fixture-v1\n"
        );
        let mode = std::env::var("SSH_CLIPBOARD_NATIVE_FILES_MODE")
            .expect("capture, recapture, publish or inspect");
        assert!(["capture", "recapture", "publish", "inspect"].contains(&mode.as_str()));
        struct OwnedNative {
            inner: NativeClipboard,
            source: Option<PathBuf>,
        }
        impl ClipboardAccess for OwnedNative {
            fn read_files(&mut self) -> Result<Option<Vec<PathBuf>>> {
                let paths = self.inner.read_files().inspect_err(|error| {
                    eprintln!("Native fixture adapter error: {error}");
                })?;
                eprintln!("Native fixture file list available: {}", paths.is_some());
                if let Some(source) = &self.source {
                    let canonical = paths
                        .as_ref()
                        .ok_or_else(|| {
                            eyre::eyre!("Copy the owned fixture folder in Explorer/Finder first.")
                        })?
                        .iter()
                        .map(|p| p.canonicalize())
                        .collect::<std::io::Result<Vec<_>>>()?;
                    eyre::ensure!(
                        canonical.len() == 1 && canonical[0] == *source,
                        "Only the owned fixture folder may be captured."
                    );
                }
                Ok(paths)
            }
            fn write_files(&mut self, paths: &[PathBuf]) -> Result<()> {
                self.inner.write_files(paths)
            }
            fn read_text(&mut self) -> Result<String> {
                eyre::bail!("Native file probe refuses text.")
            }
            fn read_image(&mut self) -> Result<ImageData<'static>> {
                eyre::bail!("Native file probe refuses images.")
            }
            fn write_text(&mut self, _: &str) -> Result<()> {
                eyre::bail!("Native file probe refuses text.")
            }
            fn write_image(&mut self, _: ImageData<'static>) -> Result<()> {
                eyre::bail!("Native file probe refuses images.")
            }
        }
        let source = if mode == "capture" {
            Some(root.join("source").canonicalize().unwrap())
        } else if mode == "recapture" {
            let cache = root.join("cache").canonicalize().unwrap();
            let mut native = NativeClipboard(Clipboard::new().unwrap());
            let paths = native
                .read_files()
                .unwrap()
                .expect("owned published references");
            assert_eq!(
                paths.len(),
                1,
                "Recapture requires one owned synthetic root."
            );
            let path = paths[0].canonicalize().unwrap();
            assert!(
                path.starts_with(&cache),
                "Recapture cannot read another application's files."
            );
            Some(path)
        } else {
            None
        };
        let worker = ClipboardWorker::start_with(move || {
            Ok(OwnedNative {
                inner: NativeClipboard(Clipboard::new()?),
                source: source.clone(),
            })
        })
        .unwrap();
        *worker.cache.lock().unwrap() =
            Ok(crate::file_cache::Cache::open(root.join("cache")).unwrap());
        match mode.as_str() {
            "capture" | "recapture" => {
                let value = worker
                    .read_with_limit(DEFAULT_MAX_SIZE, super::super::model::MAX_DECODED_BYTES)
                    .await
                    .unwrap();
                assert_eq!(
                    value.content_type,
                    ssh_clipboard::session_protocol::ContentType::Files
                );
                let bundle = ssh_clipboard::files::validate(&value.data).unwrap();
                println!(
                    "Captured {} ({} encoded bytes)",
                    bundle.label(),
                    value.data.len()
                );
                use std::io::Write;
                let mut output = std::fs::OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .open(root.join(if mode == "capture" {
                        "outbound.scbf"
                    } else {
                        "recaptured.scbf"
                    }))
                    .unwrap();
                output.write_all(&value.data).unwrap();
                output.sync_all().unwrap();
            }
            "publish" => {
                let data = std::fs::read(root.join("inbound.scbf")).unwrap();
                let bundle = ssh_clipboard::files::validate(&data).unwrap();
                assert!(
                    bundle
                        .entries
                        .iter()
                        .filter(|e| e.parent.is_none())
                        .all(|e| e.name == "source"),
                    "Only synthetic fixture roots may be published."
                );
                let value = ClipboardPayload {
                    content_type: ssh_clipboard::session_protocol::ContentType::Files,
                    data,
                    created_at: 0,
                };
                let renamed = worker
                    .write_prepared(
                        worker
                            .prepare(value, super::super::model::MAX_DECODED_BYTES)
                            .await
                            .unwrap(),
                    )
                    .await
                    .unwrap();
                println!(
                    "Published native file references; renamed={}",
                    renamed.is_some()
                );
            }
            "inspect" => {
                // Open the cache in a fresh process before checking durable native references.
                let cache = root.join("cache").canonicalize().unwrap();
                let mut native = NativeClipboard(Clipboard::new().unwrap());
                let paths = native
                    .read_files()
                    .unwrap()
                    .expect("native file references");
                assert!(!paths.is_empty());
                assert!(
                    paths
                        .iter()
                        .all(|path| path.canonicalize().is_ok_and(|p| p.starts_with(&cache))),
                    "Native references must still point into the owned cache."
                );
                println!("Native references survived process exit and cache reopen.");
            }
            _ => unreachable!(),
        }
    }

    #[derive(Default)]
    struct FileState {
        paths: Option<Vec<std::path::PathBuf>>,
        partial: bool,
        fail: bool,
        read_error: bool,
        text_reads: usize,
    }
    struct FileClipboard(Arc<Mutex<FileState>>);
    impl ClipboardAccess for FileClipboard {
        fn read_files(&mut self) -> Result<Option<Vec<std::path::PathBuf>>> {
            let state = self.0.lock().unwrap();
            if state.read_error {
                eyre::bail!("bad native file list");
            }
            Ok(state.paths.clone())
        }
        fn write_files(&mut self, paths: &[std::path::PathBuf]) -> Result<()> {
            let mut state = self.0.lock().unwrap();
            state.paths = Some(if state.partial {
                paths[..1].to_vec()
            } else {
                paths.to_vec()
            });
            if state.fail {
                eyre::bail!("native publication failed after changing clipboard");
            }
            Ok(())
        }
        fn read_text(&mut self) -> Result<String> {
            self.0.lock().unwrap().text_reads += 1;
            Ok("filename representation".into())
        }
        fn read_image(&mut self) -> Result<ImageData<'static>> {
            eyre::bail!("unused")
        }
        fn write_text(&mut self, _: &str) -> Result<()> {
            self.0.lock().unwrap().paths = None;
            Ok(())
        }
        fn write_image(&mut self, _: ImageData<'static>) -> Result<()> {
            self.0.lock().unwrap().paths = None;
            Ok(())
        }
    }
    #[tokio::test]
    async fn file_capture_precedes_text_and_freezes_bytes_without_fallback_on_failure() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("source");
        std::fs::write(&source, [0, 255]).unwrap();
        let source = source.canonicalize().unwrap();
        let state = Arc::new(Mutex::new(FileState {
            paths: Some(vec![source.clone()]),
            ..Default::default()
        }));
        let factory = state.clone();
        let worker =
            ClipboardWorker::start_with(move || Ok(FileClipboard(factory.clone()))).unwrap();
        let value = worker.read_with_limit(4096, 4096).await.unwrap();
        assert_eq!(
            value.content_type,
            ssh_clipboard::session_protocol::ContentType::Files
        );
        std::fs::write(&source, b"changed").unwrap();
        assert_eq!(
            ssh_clipboard::files::validate(&value.data).unwrap().data,
            [0, 255]
        );
        std::fs::remove_file(source).unwrap();
        assert!(worker.read_with_limit(4096, 4096).await.is_err());
        state.lock().unwrap().read_error = true;
        assert!(worker.read_with_limit(4096, 4096).await.is_err());
        assert_eq!(state.lock().unwrap().text_reads, 0);
    }
    #[tokio::test]
    async fn staging_is_explicit_and_partial_or_failed_native_publications_keep_files() {
        let directory = tempfile::tempdir().unwrap();
        let state = Arc::new(Mutex::new(FileState::default()));
        let factory = state.clone();
        let worker =
            ClipboardWorker::start_with(move || Ok(FileClipboard(factory.clone()))).unwrap();
        *worker.cache.lock().unwrap() =
            Ok(crate::file_cache::Cache::open(directory.path().join("cache")).unwrap());
        let data = ssh_clipboard::files::encode(
            &[
                ssh_clipboard::files::Entry {
                    parent: None,
                    name: "CON.txt".into(),
                    kind: ssh_clipboard::files::Kind::File,
                    length: 2,
                },
                ssh_clipboard::files::Entry {
                    parent: None,
                    name: "other".into(),
                    kind: ssh_clipboard::files::Kind::File,
                    length: 0,
                },
            ],
            &[0, 255],
            4096,
        )
        .unwrap();
        let value = ClipboardPayload {
            content_type: ssh_clipboard::session_protocol::ContentType::Files,
            data,
            created_at: 0,
        };
        let prepared = worker.prepare(value.clone(), 4096).await.unwrap();
        let Prepared::Files(files) = &prepared else {
            panic!("no staged files")
        };
        let staged_path = files.staged.files.paths[0].clone();
        assert!(state.lock().unwrap().paths.is_none());
        drop(prepared);
        assert!(
            !staged_path.exists(),
            "cancellation before publication removes staging"
        );
        for (partial, fail) in [(true, false), (false, true), (false, false)] {
            state.lock().unwrap().partial = partial;
            state.lock().unwrap().fail = fail;
            let result = worker
                .write_prepared(worker.prepare(value.clone(), 4096).await.unwrap())
                .await;
            assert_eq!(result.is_err(), partial || fail);
            let paths = state.lock().unwrap().paths.clone().unwrap();
            assert!(paths.iter().all(|path| path.exists()));
            assert_eq!(std::fs::read(&paths[0]).unwrap(), [0, 255]);
            if let Ok(Some(renamed)) = result {
                assert!(renamed.contains("CON.txt → _CON.txt"));
                assert!(!renamed.contains("cache"));
            }
        }
    }

    struct OwnedClipboard {
        text: String,
        dropped: Arc<AtomicBool>,
    }

    impl ClipboardAccess for OwnedClipboard {
        fn read_text(&mut self) -> Result<String> {
            Ok(self.text.clone())
        }
        fn read_image(&mut self) -> Result<ImageData<'static>> {
            Err(eyre::eyre!("no image"))
        }
        fn write_text(&mut self, text: &str) -> Result<()> {
            self.text = text.into();
            Ok(())
        }
        fn write_image(&mut self, _: ImageData<'static>) -> Result<()> {
            Err(eyre::eyre!("no image"))
        }
    }

    impl Drop for OwnedClipboard {
        fn drop(&mut self) {
            self.dropped.store(true, Ordering::SeqCst);
        }
    }

    struct ImageClipboard(Option<ImageData<'static>>);

    impl ClipboardAccess for ImageClipboard {
        fn read_text(&mut self) -> Result<String> {
            Err(eyre::eyre!("no text"))
        }
        fn read_image(&mut self) -> Result<ImageData<'static>> {
            self.0.take().ok_or_else(|| eyre::eyre!("no image"))
        }
        fn write_text(&mut self, _: &str) -> Result<()> {
            Err(eyre::eyre!("unused"))
        }
        fn write_image(&mut self, _: ImageData<'static>) -> Result<()> {
            Err(eyre::eyre!("unused"))
        }
    }

    #[test]
    fn captured_png_preserves_dimensions_colors_and_alpha() {
        let pixels: Vec<u8> = (0..64 * 16)
            .flat_map(|n| [n as u8, (n / 64) as u8, 127, (n % 256) as u8])
            .collect();
        let mut clipboard = ImageClipboard(Some(ImageData {
            width: 64,
            height: 16,
            bytes: pixels.clone().into(),
        }));
        let value = capture(&mut clipboard, DEFAULT_MAX_SIZE).unwrap();
        assert_eq!(value.content_type, CONTENT_TYPE_PNG);
        let decoded = ssh_clipboard::client::image::decode_png(
            &value.data,
            super::super::model::MAX_DECODED_BYTES,
        )
        .unwrap();
        assert_eq!((decoded.width, decoded.height), (64, 16));
        assert_eq!(decoded.bytes.as_ref(), pixels.as_slice());
    }

    #[test]
    fn oversized_pixel_dimensions_are_rejected_before_encoding() {
        let mut clipboard = ImageClipboard(Some(ImageData {
            width: 8192,
            height: 8192,
            bytes: Vec::new().into(),
        }));
        let error =
            capture_with_limit(&mut clipboard, DEFAULT_MAX_SIZE, 64 * 1024 * 1024).unwrap_err();
        assert!(error.contains("8192 x 8192"));
        assert!(error.contains("256.00 MiB of pixels"));
        assert!(error.contains("64 MiB image limit"));
    }

    #[test]
    fn capture_honors_both_the_selected_pixel_budget_and_encoded_writer_limit() {
        let image = || {
            ImageClipboard(Some(ImageData {
                width: 1,
                height: 1,
                bytes: vec![1, 2, 3, 255].into(),
            }))
        };
        assert!(capture_with_limit(&mut image(), 1024, 3).is_err());
        let value = capture_with_limit(&mut image(), 1024, 4).unwrap();
        assert_eq!(
            ssh_clipboard::client::image::decode_png(&value.data, 1024)
                .unwrap()
                .bytes
                .as_ref(),
            &[1, 2, 3, 255]
        );
        let error = capture_with_limit(&mut image(), 8, 4).unwrap_err();
        assert!(error.contains("PNG transfer limit"));
    }

    #[test]
    fn oversized_text_reports_measured_size_and_a_next_action() {
        let mut clipboard = OwnedClipboard {
            text: "a".repeat((10 * 1024 * 1024) + 1048576),
            dropped: Arc::new(AtomicBool::new(false)),
        };
        let error = capture(&mut clipboard, 10 * 1024 * 1024).unwrap_err();
        assert!(error.contains("11.00 MiB"));
        assert!(error.contains("10485760 bytes"));
        assert!(error.contains("Copy less text"));
    }

    #[tokio::test]
    async fn cancelled_read_keeps_admission_until_native_work_finishes() {
        struct GatedClipboard {
            started: Option<oneshot::Sender<()>>,
            release: mpsc::Receiver<()>,
            writes: Arc<AtomicUsize>,
        }
        impl ClipboardAccess for GatedClipboard {
            fn read_text(&mut self) -> Result<String> {
                if let Some(started) = self.started.take() {
                    let _ = started.send(());
                    self.release.recv()?;
                }
                Ok("captured once".into())
            }
            fn read_image(&mut self) -> Result<ImageData<'static>> {
                Err(eyre::eyre!("unused"))
            }
            fn write_text(&mut self, _: &str) -> Result<()> {
                self.writes.fetch_add(1, Ordering::SeqCst);
                Ok(())
            }
            fn write_image(&mut self, _: ImageData<'static>) -> Result<()> {
                Err(eyre::eyre!("unused"))
            }
        }
        // Release even when an assertion fails, so joining the native worker cannot hang.
        struct Release(Option<mpsc::Sender<()>>);
        impl Drop for Release {
            fn drop(&mut self) {
                if let Some(sender) = self.0.take() {
                    let _ = sender.send(());
                }
            }
        }
        let (started, receive_started) = oneshot::channel();
        let (release, receive_release) = mpsc::channel();
        let writes = Arc::new(AtomicUsize::new(0));
        let mut native = Some(GatedClipboard {
            started: Some(started),
            release: receive_release,
            writes: writes.clone(),
        });
        let worker = ClipboardWorker::start_with(move || Ok(native.take().unwrap())).unwrap();
        let release = Release(Some(release));
        {
            let read = worker.read_with_limit(1024, 256 * 1024 * 1024);
            tokio::pin!(read);
            tokio::select! {
                result = &mut read => panic!("native read must still be blocked: {result:?}"),
                result = receive_started => result.unwrap(),
            }
            // Cancel the waiting future while its native owner is still inside read_text.
        }
        let value = build_text_value("explicit write".into(), 1024).unwrap();
        assert!(
            worker
                .read_with_limit(1024, 256 * 1024 * 1024)
                .await
                .unwrap_err()
                .contains("still finishing")
        );
        assert!(
            worker
                .write_with_limit(value.clone(), 256 * 1024 * 1024)
                .await
                .unwrap_err()
                .contains("still finishing")
        );
        assert_eq!(writes.load(Ordering::SeqCst), 0);
        drop(release);
        tokio::time::timeout(std::time::Duration::from_secs(2), async {
            while worker.busy.load(Ordering::Acquire) {
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
        assert_eq!(
            writes.load(Ordering::SeqCst),
            0,
            "cancelled work must not queue a write"
        );
        worker
            .write_with_limit(value, 256 * 1024 * 1024)
            .await
            .unwrap();
        assert_eq!(writes.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn clipboard_owner_survives_operations_and_is_dropped_on_stop() {
        let created = Arc::new(AtomicUsize::new(0));
        let dropped = Arc::new(AtomicBool::new(false));
        let factory_count = created.clone();
        let factory_drop = dropped.clone();
        let worker = ClipboardWorker::start_with(move || {
            factory_count.fetch_add(1, Ordering::SeqCst);
            Ok(OwnedClipboard {
                text: "initial".into(),
                dropped: factory_drop.clone(),
            })
        })
        .unwrap();
        assert_eq!(
            created.load(Ordering::SeqCst),
            0,
            "Opening the app must not access the clipboard"
        );
        assert_eq!(
            worker
                .read_with_limit(DEFAULT_MAX_SIZE, 256 * 1024 * 1024)
                .await
                .unwrap()
                .data,
            b"initial"
        );
        worker
            .write_with_limit(
                crate::clipboard_payload::build_text_value("copied".into(), 1024).unwrap(),
                256 * 1024 * 1024,
            )
            .await
            .unwrap();
        assert_eq!(
            worker
                .read_with_limit(DEFAULT_MAX_SIZE, 256 * 1024 * 1024)
                .await
                .unwrap()
                .data,
            b"copied"
        );
        assert_eq!(created.load(Ordering::SeqCst), 1);
        assert!(!dropped.load(Ordering::SeqCst));
        worker.stop();
        assert!(dropped.load(Ordering::SeqCst));
        assert!(
            worker
                .read_with_limit(DEFAULT_MAX_SIZE, 256 * 1024 * 1024)
                .await
                .is_err()
        );
        worker.stop(); // Explicit shutdown followed by Drop must be safe.
    }
}
