use arboard::Clipboard;
use eyre::Result;
use image::ImageEncoder;
use ssh_clipboard::client::image::ImageData;
use ssh_clipboard::client_actions::{
    ClipboardAccess, apply_pull_response_with_clipboard, build_text_value,
};
#[cfg(test)]
use ssh_clipboard::protocol::DEFAULT_MAX_SIZE;
use ssh_clipboard::protocol::{CONTENT_TYPE_PNG, ClipboardValue, Response, ResponseKind};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, mpsc};
use std::thread::JoinHandle;
use tokio::sync::oneshot;

struct NativeClipboard(Clipboard);

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
) -> Result<ClipboardValue, String> {
    capture_with_limit(clipboard, max_size, super::model::MAX_DECODED_BYTES)
}

fn capture_with_limit(
    clipboard: &mut impl ClipboardAccess,
    max_size: usize,
    max_decoded: usize,
) -> Result<ClipboardValue, String> {
    match clipboard.read_text() {
        Ok(text) => {
            if text.len() > max_size {
                return Err(format!(
                    "This text is {:.2} MiB. The transfer limit is {:.2} MiB ({max_size} bytes). Copy less text and try again.",
                    text.len() as f64 / 1048576.0,
                    max_size as f64 / 1048576.0
                ));
            }
            build_text_value(text, max_size).map_err(|err| err.message)
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
            Ok(ClipboardValue {
                content_type: CONTENT_TYPE_PNG.into(),
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
        oneshot::Sender<Result<ClipboardValue, String>>,
        Permit,
    ),
    Write(
        ClipboardValue,
        usize,
        oneshot::Sender<Result<(), String>>,
        Permit,
    ),
    Stop,
}

pub struct ClipboardWorker {
    sender: mpsc::SyncSender<Job>,
    thread: Mutex<Option<JoinHandle<()>>>,
    busy: Arc<AtomicBool>,
}

impl ClipboardWorker {
    pub fn start() -> Result<Self> {
        Self::start_with(|| Ok(NativeClipboard(Clipboard::new()?)))
    }

    pub(crate) fn start_with<C: ClipboardAccess + 'static>(
        mut create: impl FnMut() -> Result<C> + Send + 'static,
    ) -> Result<Self> {
        let (sender, receiver) = mpsc::sync_channel(1);
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
                                    capture_with_limit(
                                        clipboard.as_mut().unwrap(),
                                        max_size,
                                        max_decoded,
                                    )
                                });
                            drop(permit);
                            let _ = reply.send(result);
                        }
                        Job::Write(value, max_decoded, reply, permit) => {
                            let result = init
                                .map_err(|err| format!("Clipboard unavailable: {err}"))
                                .and_then(|()| {
                                    apply_pull_response_with_clipboard(
                                        Response {
                                            request_id: 0,
                                            kind: ResponseKind::Value { value },
                                        },
                                        max_decoded,
                                        clipboard.as_mut().unwrap(),
                                    )
                                    .map_err(|err| err.message)
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

    pub async fn read(&self, max_size: usize) -> Result<ClipboardValue, String> {
        self.read_with_limit(max_size, super::model::MAX_DECODED_BYTES)
            .await
    }

    pub async fn read_with_limit(
        &self,
        max_size: usize,
        max_decoded: usize,
    ) -> Result<ClipboardValue, String> {
        let permit = self.reserve()?;
        let (reply, receive) = oneshot::channel();
        self.sender
            .try_send(Job::Read(max_size, max_decoded, reply, permit))
            .map_err(|_| "Clipboard worker is busy or unavailable.".to_string())?;
        receive
            .await
            .map_err(|_| "Clipboard worker stopped.".to_string())?
    }

    pub async fn write(&self, value: ClipboardValue) -> Result<(), String> {
        self.write_with_limit(value, super::model::MAX_DECODED_BYTES)
            .await
    }

    pub async fn write_with_limit(
        &self,
        value: ClipboardValue,
        max_decoded: usize,
    ) -> Result<(), String> {
        let permit = self.reserve()?;
        let (reply, receive) = oneshot::channel();
        self.sender
            .try_send(Job::Write(value, max_decoded, reply, permit))
            .map_err(|_| "Clipboard worker is busy or unavailable.".to_string())?;
        receive
            .await
            .map_err(|_| "Clipboard worker stopped.".to_string())?
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
        let error = capture(&mut clipboard, DEFAULT_MAX_SIZE).unwrap_err();
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
            text: "a".repeat(DEFAULT_MAX_SIZE + 1048576),
            dropped: Arc::new(AtomicBool::new(false)),
        };
        let error = capture(&mut clipboard, DEFAULT_MAX_SIZE).unwrap_err();
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
            let read = worker.read(1024);
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
                .read(1024)
                .await
                .unwrap_err()
                .contains("still finishing")
        );
        assert!(
            worker
                .write(value.clone())
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
        worker.write(value).await.unwrap();
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
            worker.read(DEFAULT_MAX_SIZE).await.unwrap().data,
            b"initial"
        );
        worker
            .write(ssh_clipboard::client_actions::build_text_value("copied".into(), 1024).unwrap())
            .await
            .unwrap();
        assert_eq!(worker.read(DEFAULT_MAX_SIZE).await.unwrap().data, b"copied");
        assert_eq!(created.load(Ordering::SeqCst), 1);
        assert!(!dropped.load(Ordering::SeqCst));
        worker.stop();
        assert!(dropped.load(Ordering::SeqCst));
        assert!(worker.read(DEFAULT_MAX_SIZE).await.is_err());
        worker.stop(); // Explicit shutdown followed by Drop must be safe.
    }
}
