use arboard::Clipboard;
use eyre::Result;
use image::ImageEncoder;
use ssh_clipboard::client::image::ImageData;
use ssh_clipboard::client_actions::{
    ClipboardAccess, apply_pull_response_with_clipboard, build_text_value,
};
use ssh_clipboard::protocol::{
    CONTENT_TYPE_PNG, ClipboardValue, DEFAULT_MAX_SIZE, Response, ResponseKind,
};
use std::sync::{Mutex, mpsc};
use std::thread::JoinHandle;
use tokio::sync::oneshot;

struct NativeClipboard(Clipboard);

fn capture(clipboard: &mut impl ClipboardAccess) -> Result<ClipboardValue, String> {
    match clipboard.read_text() {
        Ok(text) => {
            if text.len() > DEFAULT_MAX_SIZE {
                return Err(format!(
                    "This text is {:.2} MiB. The prototype transfer limit is 10 MiB. Copy less text and try again.",
                    text.len() as f64 / 1048576.0
                ));
            }
            build_text_value(text, DEFAULT_MAX_SIZE).map_err(|err| err.message)
        }
        Err(text_error) => {
            let image = clipboard.read_image().map_err(|image_error| format!(
                "Could not read text or an image from the clipboard (text: {text_error}; image: {image_error})."))?;
            let decoded_bytes = image
                .width
                .checked_mul(image.height)
                .and_then(|pixels| pixels.checked_mul(4))
                .ok_or_else(|| "The clipboard image dimensions are too large.".to_string())?;
            if decoded_bytes > super::model::MAX_DECODED_BYTES {
                return Err(format!(
                    "Image {} x {} needs {:.2} MiB of pixels, above the 64 MiB image limit. Copy a smaller image and try again.",
                    image.width,
                    image.height,
                    decoded_bytes as f64 / 1048576.0
                ));
            }
            if image.width == 0 || image.height == 0 || image.bytes.len() != decoded_bytes {
                return Err("The clipboard image has an invalid pixel buffer.".into());
            }
            let width = u32::try_from(image.width).map_err(|_| "Invalid image width.")?;
            let height = u32::try_from(image.height).map_err(|_| "Invalid image height.")?;
            let mut png = Vec::new();
            // Preserve every RGBA pixel; prefer balanced compression over the fast default.
            image::codecs::png::PngEncoder::new_with_quality(
                &mut png,
                image::codecs::png::CompressionType::Default,
                image::codecs::png::FilterType::Adaptive,
            )
            .write_image(&image.bytes, width, height, image::ExtendedColorType::Rgba8)
            .map_err(|_| "Could not encode the clipboard image as PNG.".to_string())?;
            if png.len() > DEFAULT_MAX_SIZE {
                return Err(format!(
                    "Image {width} x {height} is {:.2} MiB as PNG. The prototype transfer limit is 10 MiB. Copy a smaller image and try again.",
                    png.len() as f64 / 1048576.0
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

enum Job {
    Read(oneshot::Sender<Result<ClipboardValue, String>>),
    Write(ClipboardValue, oneshot::Sender<Result<(), String>>),
    Stop,
}

pub struct ClipboardWorker {
    sender: mpsc::SyncSender<Job>,
    thread: Mutex<Option<JoinHandle<()>>>,
}

impl ClipboardWorker {
    pub fn start() -> Result<Self> {
        Self::start_with(|| Ok(NativeClipboard(Clipboard::new()?)))
    }

    fn start_with<C: ClipboardAccess + 'static>(
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
                        Job::Read(reply) => {
                            let result = init
                                .map_err(|err| format!("Clipboard unavailable: {err}"))
                                .and_then(|()| capture(clipboard.as_mut().unwrap()));
                            let _ = reply.send(result);
                        }
                        Job::Write(value, reply) => {
                            let result = init
                                .map_err(|err| format!("Clipboard unavailable: {err}"))
                                .and_then(|()| {
                                    apply_pull_response_with_clipboard(
                                        Response {
                                            request_id: 0,
                                            kind: ResponseKind::Value { value },
                                        },
                                        super::model::MAX_DECODED_BYTES,
                                        clipboard.as_mut().unwrap(),
                                    )
                                    .map_err(|err| err.message)
                                });
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
        })
    }

    pub async fn read(&self) -> Result<ClipboardValue, String> {
        let (reply, receive) = oneshot::channel();
        self.sender
            .try_send(Job::Read(reply))
            .map_err(|_| "Clipboard worker is busy or unavailable.".to_string())?;
        receive
            .await
            .map_err(|_| "Clipboard worker stopped.".to_string())?
    }

    pub async fn write(&self, value: ClipboardValue) -> Result<(), String> {
        let (reply, receive) = oneshot::channel();
        self.sender
            .try_send(Job::Write(value, reply))
            .map_err(|_| "Clipboard worker is busy or unavailable.".to_string())?;
        receive
            .await
            .map_err(|_| "Clipboard worker stopped.".to_string())?
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
        let value = capture(&mut clipboard).unwrap();
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
        let error = capture(&mut clipboard).unwrap_err();
        assert!(error.contains("8192 x 8192"));
        assert!(error.contains("256.00 MiB of pixels"));
        assert!(error.contains("64 MiB image limit"));
    }

    #[test]
    fn oversized_text_reports_measured_size_and_a_next_action() {
        let mut clipboard = OwnedClipboard {
            text: "a".repeat(DEFAULT_MAX_SIZE + 1048576),
            dropped: Arc::new(AtomicBool::new(false)),
        };
        let error = capture(&mut clipboard).unwrap_err();
        assert!(error.contains("11.00 MiB"));
        assert!(error.contains("10 MiB"));
        assert!(error.contains("Copy less text"));
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
        assert_eq!(worker.read().await.unwrap().data, b"initial");
        worker
            .write(ssh_clipboard::client_actions::build_text_value("copied".into(), 1024).unwrap())
            .await
            .unwrap();
        assert_eq!(worker.read().await.unwrap().data, b"copied");
        assert_eq!(created.load(Ordering::SeqCst), 1);
        assert!(!dropped.load(Ordering::SeqCst));
        worker.stop();
        assert!(dropped.load(Ordering::SeqCst));
        assert!(worker.read().await.is_err());
        worker.stop(); // Explicit shutdown followed by Drop must be safe.
    }
}
