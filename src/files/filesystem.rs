use super::{Entry, Kind, MAX_DEPTH, MAX_ENTRIES, encode, validate};
use eyre::{Result, ensure};
use serde::Serialize;
use std::collections::HashSet;
use std::fs::{self, File, Metadata, OpenOptions};
use std::io::{Read, Write};
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

fn check(cancel: &AtomicBool) -> Result<()> {
    ensure!(!cancel.load(Ordering::Acquire), "File operation cancelled.");
    Ok(())
}

fn regular(metadata: &Metadata) -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        if metadata.file_attributes() & 0x400 != 0 {
            return false;
        }
    }
    !metadata.file_type().is_symlink() && (metadata.is_file() || metadata.is_dir())
}

// Keep directory handles alive throughout traversal. Windows denies delete sharing;
// Unix opens each component relative to its already-held parent with O_NOFOLLOW.
struct Source {
    file: File,
    path: PathBuf,
    _parents: Vec<File>,
}

#[cfg(unix)]
fn child_file(parent: &File, name: &std::ffi::OsStr) -> Result<File> {
    use std::os::{
        fd::{AsRawFd, FromRawFd},
        unix::ffi::OsStrExt,
    };
    let name = std::ffi::CString::new(name.as_bytes())?;
    // SAFETY: parent remains live, the name is NUL terminated, and the returned fd
    // is owned exactly once. NONBLOCK prevents a replaced FIFO from blocking open.
    let fd = unsafe {
        libc::openat(
            parent.as_raw_fd(),
            name.as_ptr(),
            libc::O_RDONLY | libc::O_NOFOLLOW | libc::O_CLOEXEC | libc::O_NONBLOCK,
        )
    };
    if fd < 0 {
        return Err(std::io::Error::last_os_error().into());
    }
    Ok(unsafe { File::from_raw_fd(fd) })
}

#[cfg(windows)]
fn windows_file(path: &Path) -> Result<File> {
    use std::os::windows::fs::OpenOptionsExt;
    // FILE_FLAG_OPEN_REPARSE_POINT | FILE_FLAG_BACKUP_SEMANTICS, FILE_SHARE_READ.
    let file = OpenOptions::new()
        .read(true)
        .share_mode(1)
        .custom_flags(0x0220_0000)
        .open(path)?;
    ensure!(
        regular(&file.metadata()?),
        "Links, junctions and special files are unsupported."
    );
    Ok(file)
}

fn source(path: &Path) -> Result<Source> {
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()?.join(path)
    };
    #[cfg(unix)]
    let mut file = File::open("/")?;
    #[cfg(windows)]
    let mut file = {
        let mut root = PathBuf::new();
        for part in absolute
            .components()
            .take_while(|part| matches!(part, Component::Prefix(_) | Component::RootDir))
        {
            root.push(part.as_os_str());
        }
        windows_file(&root)?
    };
    let mut parents = Vec::new();
    let mut current = PathBuf::new();
    for part in absolute.components() {
        if matches!(part, Component::ParentDir) {
            ensure!(
                file.metadata()?.is_dir(),
                "A source path traverses a regular file."
            );
            if let Some(parent) = parents.pop() {
                file = parent;
                current.pop();
            }
            continue;
        }
        current.push(part.as_os_str());
        if let Component::Normal(name) = part {
            #[cfg(unix)]
            let next = child_file(&file, name)?;
            #[cfg(windows)]
            let next = {
                let _ = name;
                windows_file(&current)?
            };
            ensure!(
                regular(&next.metadata()?),
                "Links, junctions and special files are unsupported."
            );
            parents.push(file);
            file = next;
        }
    }
    Ok(Source {
        file,
        #[cfg(windows)]
        path: current.canonicalize()?,
        #[cfg(not(windows))]
        path: current,
        _parents: parents,
    })
}

#[cfg(unix)]
fn names(source: &Source) -> Result<Vec<std::ffi::OsString>> {
    use std::os::{fd::AsRawFd, unix::ffi::OsStrExt};
    // Open a separate directory description: dup would share the iteration offset.
    let directory = child_file(&source.file, std::ffi::OsStr::new("."))?;
    let fd = unsafe { libc::dup(directory.as_raw_fd()) };
    ensure!(fd >= 0, "Could not enumerate folder.");
    let stream = unsafe { libc::fdopendir(fd) };
    if stream.is_null() {
        unsafe { libc::close(fd) };
        return Err(std::io::Error::last_os_error().into());
    }
    struct Stream(*mut libc::DIR);
    impl Drop for Stream {
        fn drop(&mut self) {
            unsafe {
                libc::closedir(self.0);
            }
        }
    }
    let stream = Stream(stream);
    let mut result = Vec::new();
    loop {
        #[cfg(target_os = "linux")]
        let errno = unsafe { libc::__errno_location() };
        #[cfg(target_os = "macos")]
        let errno = unsafe { libc::__error() };
        unsafe {
            *errno = 0;
        }
        let entry = unsafe { libc::readdir(stream.0) };
        if entry.is_null() {
            if unsafe { *errno } != 0 {
                return Err(std::io::Error::last_os_error().into());
            }
            break;
        }
        let name = unsafe { std::ffi::CStr::from_ptr((*entry).d_name.as_ptr()) }.to_bytes();
        if name != b"." && name != b".." {
            ensure!(
                result.len() < MAX_ENTRIES,
                "Folder exceeds the entry limit."
            );
            result.push(std::ffi::OsStr::from_bytes(name).to_os_string());
        }
    }
    result.sort();
    Ok(result)
}

#[cfg(windows)]
fn names(source: &Source) -> Result<Vec<std::ffi::OsString>> {
    let mut result = Vec::new();
    for entry in fs::read_dir(&source.path)? {
        ensure!(
            result.len() < MAX_ENTRIES,
            "Folder exceeds the entry limit."
        );
        result.push(entry?.file_name());
    }
    result.sort();
    Ok(result)
}

struct Capture<'a> {
    entries: Vec<Entry>,
    data: Vec<u8>,
    maximum: usize,
    cancel: &'a AtomicBool,
}
impl Capture<'_> {
    fn visit(&mut self, mut source: Source, parent: Option<usize>, depth: usize) -> Result<()> {
        check(self.cancel)?;
        ensure!(
            depth <= MAX_DEPTH && self.entries.len() < MAX_ENTRIES,
            "File selection exceeds tree limits."
        );
        let before = source.file.metadata()?;
        ensure!(regular(&before), "Links and special files are unsupported.");
        let name = source
            .path
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or_else(|| {
                eyre::eyre!("Source names must be UTF-8; a filesystem root cannot be shared.")
            })?
            .to_owned();
        let index = self.entries.len();
        self.entries.push(Entry {
            parent,
            name,
            kind: if before.is_dir() {
                Kind::Directory
            } else {
                Kind::File
            },
            length: 0,
        });
        if before.is_dir() {
            for name in names(&source)? {
                check(self.cancel)?;
                #[cfg(unix)]
                let file = child_file(&source.file, &name)?;
                #[cfg(windows)]
                let file = windows_file(&source.path.join(&name))?;
                self.visit(
                    Source {
                        file,
                        path: source.path.join(name),
                        _parents: Vec::new(),
                    },
                    Some(index),
                    depth + 1,
                )?;
            }
        } else {
            ensure!(
                before.len() <= self.maximum.saturating_sub(self.data.len()) as u64,
                "File selection exceeds the item limit."
            );
            let start = self.data.len();
            let mut chunk = [0; 64 * 1024];
            loop {
                check(self.cancel)?;
                let count = source.file.read(&mut chunk)?;
                if count == 0 {
                    break;
                }
                ensure!(
                    self.data.len().saturating_add(count) <= self.maximum,
                    "File selection exceeds the item limit."
                );
                self.data.try_reserve(count)?;
                self.data.extend_from_slice(&chunk[..count]);
            }
            ensure!(
                (self.data.len() - start) as u64 == before.len(),
                "A source file changed during capture. Copy it again."
            );
            self.entries[index].length = (self.data.len() - start).try_into()?;
        }
        let after = source.file.metadata()?;
        ensure!(
            before.len() == after.len() && before.modified()? == after.modified()?,
            "A source changed during capture. Copy it again."
        );
        Ok(())
    }
}

pub fn capture(paths: &[PathBuf], maximum: usize, cancel: &AtomicBool) -> Result<Vec<u8>> {
    ensure!(
        maximum <= crate::session_protocol::MAX_PAYLOAD_BYTES,
        "File capture limit exceeds 256 MiB."
    );
    ensure!(
        !paths.is_empty() && paths.len() <= MAX_ENTRIES,
        "Choose between 1 and 4096 files/folders."
    );
    let sources = paths
        .iter()
        .map(|path| {
            check(cancel)?;
            let opened = source(path)?;
            Ok((opened.path, opened.file.metadata()?.is_dir()))
        })
        .collect::<Result<Vec<_>>>()?;
    let mut seen = HashSet::new();
    let mut capture = Capture {
        entries: Vec::new(),
        data: Vec::new(),
        maximum,
        cancel,
    };
    for (index, (path, _)) in sources.iter().enumerate() {
        check(cancel)?;
        if !seen.insert(path.clone())
            || sources
                .iter()
                .enumerate()
                .any(|(other, (candidate, directory))| {
                    other != index && path != candidate && path.starts_with(candidate) && *directory
                })
        {
            continue;
        }
        // At most one root and its ancestor/traversal chain stays open at a time.
        capture.visit(source(path)?, None, 1)?;
    }
    encode(&capture.entries, &capture.data, maximum)
}

#[derive(Clone, Debug, Serialize)]
pub struct Rename {
    pub original: String,
    pub received: String,
}
#[derive(Debug, Serialize)]
pub struct Materialized {
    pub paths: Vec<PathBuf>,
    pub renames: Vec<Rename>,
}

fn truncate(value: &str, maximum: usize) -> &str {
    let mut end = value.len().min(maximum);
    while !value.is_char_boundary(end) {
        end -= 1;
    }
    &value[..end]
}
fn portable(name: &str, attempt: usize) -> String {
    let mut name: String = name
        .chars()
        .map(|c| {
            if c.is_control() || "<>:\"/\\|?*".contains(c) {
                '_'
            } else {
                c
            }
        })
        .collect();
    name = name.trim_end_matches([' ', '.']).to_owned();
    let base = name
        .split('.')
        .next()
        .unwrap_or("")
        .trim_end_matches(' ')
        .to_uppercase();
    let numbered = base
        .strip_prefix("COM")
        .or_else(|| base.strip_prefix("LPT"));
    if name.is_empty()
        || ["CON", "PRN", "AUX", "NUL"].contains(&base.as_str())
        || numbered.is_some_and(|n| {
            ["1", "2", "3", "4", "5", "6", "7", "8", "9", "¹", "²", "³"].contains(&n)
        })
    {
        name.insert(0, '_');
    }
    let (stem, extension) = name
        .rsplit_once('.')
        .filter(|(stem, _)| !stem.is_empty())
        .map(|(stem, ext)| (stem, format!(".{}", truncate(ext, 40))))
        .unwrap_or((&name, String::new()));
    let suffix = if attempt == 1 {
        String::new()
    } else {
        format!(" ({attempt})")
    };
    format!(
        "{}{}{}",
        truncate(stem, 180 - extension.len() - suffix.len()),
        suffix,
        extension
    )
}

/// Populate an exclusively owned empty directory. No source metadata or links are restored.
pub fn materialize(bytes: &[u8], root: &Path, cancel: &AtomicBool) -> Result<Materialized> {
    let bundle = validate(bytes)?;
    let metadata = fs::symlink_metadata(root)?;
    ensure!(
        metadata.is_dir() && regular(&metadata) && fs::read_dir(root)?.next().is_none(),
        "Extraction requires an empty private staging directory."
    );
    let mut received: Vec<PathBuf> = Vec::new();
    let mut originals: Vec<Vec<String>> = Vec::new();
    let mut siblings = HashSet::new();
    let mut result = Materialized {
        paths: Vec::new(),
        renames: Vec::new(),
    };
    let mut offset = 0;
    for entry in &bundle.entries {
        check(cancel)?;
        let parent = entry
            .parent
            .map(|id| received[id].clone())
            .unwrap_or_default();
        let mut original = entry
            .parent
            .map(|id| originals[id].clone())
            .unwrap_or_default();
        original.push(entry.name.clone());
        let mut attempt = 1;
        let (relative, mut output) = loop {
            ensure!(
                attempt <= MAX_ENTRIES + 1,
                "Could not assign a unique filename."
            );
            let name = portable(&entry.name, attempt);
            let relative = parent.join(&name);
            if siblings.contains(&(parent.clone(), name.to_lowercase())) {
                attempt += 1;
                continue;
            }
            let path = root.join(&relative);
            let created = if entry.kind == Kind::Directory {
                fs::create_dir(&path).map(|()| None)
            } else {
                OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .open(&path)
                    .map(Some)
            };
            match created {
                Ok(output) => {
                    siblings.insert((parent.clone(), name.to_lowercase()));
                    break (relative, output);
                }
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                    attempt += 1;
                }
                Err(error) => return Err(error.into()),
            }
        };
        if let Some(output) = output.as_mut() {
            for chunk in bundle.data[offset..offset + entry.length as usize].chunks(64 * 1024) {
                check(cancel)?;
                output.write_all(chunk)?;
            }
            output.sync_all()?;
        }
        offset += entry.length as usize;
        let original_name = original.join("/");
        let received_name = relative
            .components()
            .map(|c| c.as_os_str().to_string_lossy())
            .collect::<Vec<_>>()
            .join("/");
        if original_name != received_name {
            result.renames.push(Rename {
                original: original_name,
                received: received_name,
            });
        }
        if entry.parent.is_none() {
            result.paths.push(relative.clone());
        }
        originals.push(original);
        received.push(relative);
    }
    Ok(result)
}

/// Atomic directory publication must never replace even an empty competing directory.
pub fn publish_directory(staged: &Path, destination: &Path) -> Result<()> {
    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStrExt;
        #[link(name = "kernel32")]
        unsafe extern "system" {
            fn MoveFileExW(from: *const u16, to: *const u16, flags: u32) -> i32;
        }
        let from = staged
            .as_os_str()
            .encode_wide()
            .chain(Some(0))
            .collect::<Vec<_>>();
        let to = destination
            .as_os_str()
            .encode_wide()
            .chain(Some(0))
            .collect::<Vec<_>>();
        // Do not set MOVEFILE_REPLACE_EXISTING (std::fs::rename does).
        if unsafe { MoveFileExW(from.as_ptr(), to.as_ptr(), 8) } == 0 {
            return Err(std::io::Error::last_os_error().into());
        }
    }
    #[cfg(unix)]
    {
        use std::os::unix::ffi::OsStrExt;
        let from = std::ffi::CString::new(staged.as_os_str().as_bytes())?;
        let to = std::ffi::CString::new(destination.as_os_str().as_bytes())?;
        #[cfg(target_os = "linux")]
        let status = unsafe {
            libc::renameat2(
                libc::AT_FDCWD,
                from.as_ptr(),
                libc::AT_FDCWD,
                to.as_ptr(),
                libc::RENAME_NOREPLACE,
            )
        };
        #[cfg(target_os = "macos")]
        let status = unsafe { libc::renamex_np(from.as_ptr(), to.as_ptr(), libc::RENAME_EXCL) };
        if status != 0 {
            return Err(std::io::Error::last_os_error().into());
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn captures_hidden_binary_empty_folders_and_overlapping_roots() {
        let source_dir = tempfile::tempdir().unwrap();
        let source = source_dir.path().canonicalize().unwrap();
        fs::create_dir(source.as_path().join("empty")).unwrap();
        fs::write(source.as_path().join(".hidden"), [0, 255, 3]).unwrap();
        let cancel = AtomicBool::new(false);
        let bytes = capture(
            &[
                source.as_path().join(".hidden"),
                source.as_path().into(),
                source.as_path().into(),
            ],
            4096,
            &cancel,
        )
        .unwrap();
        let bundle = validate(&bytes).unwrap();
        assert_eq!(bundle.entries.len(), 3);
        let output = tempfile::tempdir().unwrap();
        let result = materialize(&bytes, output.path(), &cancel).unwrap();
        assert_eq!(result.paths.len(), 1);
        assert_eq!(
            fs::read(output.path().join(&result.paths[0]).join(".hidden")).unwrap(),
            [0, 255, 3]
        );
        assert!(output.path().join(&result.paths[0]).join("empty").is_dir());
        let relative = capture(&[source.join("empty/../.hidden")], 4096, &cancel).unwrap();
        assert_eq!(validate(&relative).unwrap().data, [0, 255, 3]);
        assert!(capture(&[source.as_path().into()], 16, &cancel).is_err());
        cancel.store(true, Ordering::Release);
        assert!(capture(&[source.as_path().into()], 4096, &cancel).is_err());
    }
    #[test]
    fn portable_renames_are_stable_preserve_extensions_and_do_not_clobber() {
        let names = [
            "CON.txt",
            "a:b.txt",
            "a?b.txt",
            "README",
            "readme",
            "trailing. ",
            "a\\b",
            "é",
        ];
        let entries = names
            .iter()
            .map(|name| Entry {
                parent: None,
                name: (*name).into(),
                kind: Kind::File,
                length: 0,
            })
            .collect::<Vec<_>>();
        let bytes = encode(&entries, &[], 4096).unwrap();
        let output = tempfile::tempdir().unwrap();
        let result = materialize(&bytes, output.path(), &AtomicBool::new(false)).unwrap();
        assert_eq!(result.paths[0], Path::new("_CON.txt"));
        assert_eq!(result.paths[2], Path::new("a_b (2).txt"));
        assert_eq!(result.paths[4], Path::new("readme (2)"));
        assert!(!result.renames.is_empty());
        assert!(materialize(&bytes, output.path(), &AtomicBool::new(false)).is_err());
        assert!(portable(&format!("{}.txt", "é".repeat(200)), 2).len() <= 180);
    }
    #[test]
    fn publication_refuses_existing_empty_directory() {
        let parent = tempfile::tempdir().unwrap();
        let from = parent.path().join("from");
        let to = parent.path().join("to");
        fs::create_dir(&from).unwrap();
        fs::create_dir(&to).unwrap();
        assert!(publish_directory(&from, &to).is_err());
        assert!(from.exists());
    }
    #[test]
    fn renamed_parents_keep_descendants_and_literal_names_inside_the_tree() {
        let entries = [
            Entry {
                parent: None,
                name: "CON".into(),
                kind: Kind::Directory,
                length: 0,
            },
            Entry {
                parent: Some(0),
                name: "C:\\file".into(),
                kind: Kind::File,
                length: 1,
            },
            Entry {
                parent: Some(0),
                name: "c:_file".into(),
                kind: Kind::File,
                length: 1,
            },
        ];
        let bytes = encode(&entries, b"ab", 4096).unwrap();
        let root = tempfile::tempdir().unwrap();
        let result = materialize(&bytes, root.path(), &AtomicBool::new(false)).unwrap();
        assert_eq!(fs::read(root.path().join("_CON/C__file")).unwrap(), b"a");
        assert_eq!(
            fs::read(root.path().join("_CON/c__file (2)")).unwrap(),
            b"b"
        );
        assert!(
            result
                .renames
                .iter()
                .any(|r| r.original == "CON/C:\\file" && r.received == "_CON/C__file")
        );
    }
    #[cfg(unix)]
    #[test]
    fn never_follows_source_links_or_linked_ancestors() {
        use std::os::unix::fs::symlink;
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir(dir.path().join("real")).unwrap();
        fs::write(dir.path().join("real/file"), b"secret").unwrap();
        symlink(dir.path().join("real"), dir.path().join("link")).unwrap();
        for path in [
            dir.path().join("link"),
            dir.path().join("link/file"),
            dir.path().to_path_buf(),
        ] {
            assert!(capture(&[path], 4096, &AtomicBool::new(false)).is_err());
        }
    }
}
