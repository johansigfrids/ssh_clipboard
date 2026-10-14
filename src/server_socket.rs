//! Linux socket ownership. Keep the lock inode stable and never unlink a live endpoint.
use eyre::{Result, WrapErr, eyre};
use std::fs::{DirBuilder, OpenOptions};
use std::os::unix::fs::{DirBuilderExt, FileTypeExt, MetadataExt, OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::time::Duration;
use tokio::net::{UnixListener, UnixStream};

#[derive(Debug)]
pub(crate) struct SocketBinding {
    pub listener: UnixListener,
    _lock: crate::file_lock::ExclusiveLock,
    path: PathBuf,
    identity: (u64, u64),
}

impl SocketBinding {
    pub async fn bind(path: &Path) -> Result<Self> {
        let uid = unsafe { libc::getuid() };
        let path = if path.is_absolute() {
            path.to_owned()
        } else {
            std::env::current_dir()?.join(path)
        };
        let parent = path
            .parent()
            .ok_or_else(|| eyre!("socket needs a private parent directory"))?;
        DirBuilder::new()
            .recursive(true)
            .mode(0o700)
            .create(parent)?;
        let directory = std::fs::symlink_metadata(parent)?;
        if !directory.is_dir() || directory.uid() != uid || directory.mode() & 0o077 != 0 {
            return Err(eyre!(
                "socket parent must be an owner-only directory owned by this user"
            ));
        }
        let mut lock_path = path.as_os_str().to_owned();
        lock_path.push(".lock");
        let lock = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW)
            .open(Path::new(&lock_path))
            .wrap_err("open socket ownership lock")?;
        let lock_metadata = lock.metadata()?;
        if !lock_metadata.is_file()
            || lock_metadata.uid() != uid
            || lock_metadata.mode() & 0o077 != 0
            || lock_metadata.nlink() != 1
        {
            return Err(eyre!("socket ownership lock is not a private regular file"));
        }
        let lock = crate::file_lock::ExclusiveLock::acquire(lock)
            .map_err(|_| eyre!("another daemon owns this socket; leave its endpoint untouched"))?;
        match std::fs::symlink_metadata(&path) {
            Ok(stale) => {
                if !stale.file_type().is_socket() || stale.uid() != uid {
                    return Err(eyre!(
                        "existing socket path is not this user's socket; refusing replacement"
                    ));
                }
                match tokio::time::timeout(Duration::from_millis(200), UnixStream::connect(&path))
                    .await
                {
                    Ok(Err(error))
                        if matches!(
                            error.kind(),
                            std::io::ErrorKind::ConnectionRefused | std::io::ErrorKind::NotFound
                        ) => {}
                    _ => return Err(eyre!("existing socket may be active; refusing replacement")),
                }
                match std::fs::symlink_metadata(&path) {
                    Ok(current)
                        if current.file_type().is_socket()
                            && current.uid() == uid
                            && current.dev() == stale.dev()
                            && current.ino() == stale.ino() =>
                    {
                        std::fs::remove_file(&path)?
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                    _ => return Err(eyre!("socket path changed during stale-socket inspection")),
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }
        let listener = UnixListener::bind(&path).wrap_err("bind private socket")?;
        let metadata = std::fs::symlink_metadata(&path)?;
        let binding = Self {
            listener,
            _lock: lock,
            path,
            identity: (metadata.dev(), metadata.ino()),
        };
        std::fs::set_permissions(&binding.path, std::fs::Permissions::from_mode(0o600))?;
        Ok(binding)
    }
}

impl Drop for SocketBinding {
    fn drop(&mut self) {
        if let Ok(metadata) = std::fs::symlink_metadata(&self.path)
            && metadata.file_type().is_socket()
            && (metadata.dev(), metadata.ino()) == self.identity
        {
            let _ = std::fs::remove_file(&self.path);
        }
        // Do not remove the lock file: replacement could let two daemons lock different inodes.
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn duplicate_startup_preserves_the_active_socket_and_lock_inode() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("runtime/shelf.sock");
        let binding = SocketBinding::bind(&path).await.unwrap();
        let inode = std::fs::metadata(&path).unwrap().ino();
        assert!(SocketBinding::bind(&path).await.is_err());
        assert_eq!(std::fs::metadata(&path).unwrap().ino(), inode);
        assert!(UnixStream::connect(&path).await.is_ok());
        let lock_path = path.with_file_name("shelf.sock.lock");
        let lock_inode = std::fs::metadata(&lock_path).unwrap().ino();
        drop(binding);
        assert!(!path.exists());
        let _next = SocketBinding::bind(&path).await.unwrap();
        assert_eq!(std::fs::metadata(lock_path).unwrap().ino(), lock_inode);
    }
    #[tokio::test]
    async fn active_unlocked_sockets_are_preserved_but_stale_sockets_can_recover() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::set_permissions(dir.path(), std::fs::Permissions::from_mode(0o700)).unwrap();
        let path = dir.path().join("shelf.sock");
        let old = UnixListener::bind(&path).unwrap();
        assert!(SocketBinding::bind(&path).await.is_err());
        assert!(path.exists());
        drop(old);
        let _binding = SocketBinding::bind(&path).await.unwrap();
        assert_eq!(std::fs::metadata(&path).unwrap().mode() & 0o777, 0o600);
    }
    #[tokio::test]
    async fn regular_files_symlinks_and_replaced_paths_are_never_deleted() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::set_permissions(dir.path(), std::fs::Permissions::from_mode(0o700)).unwrap();
        let path = dir.path().join("shelf.sock");
        std::fs::write(&path, b"keep").unwrap();
        assert!(SocketBinding::bind(&path).await.is_err());
        assert_eq!(std::fs::read(&path).unwrap(), b"keep");
        std::fs::remove_file(&path).unwrap();
        std::os::unix::fs::symlink(dir.path().join("missing"), &path).unwrap();
        assert!(SocketBinding::bind(&path).await.is_err());
        assert!(
            std::fs::symlink_metadata(&path)
                .unwrap()
                .file_type()
                .is_symlink()
        );
        std::fs::remove_file(&path).unwrap();
        let binding = SocketBinding::bind(&path).await.unwrap();
        std::fs::remove_file(&path).unwrap();
        std::fs::write(&path, b"replacement").unwrap();
        drop(binding);
        assert_eq!(std::fs::read(&path).unwrap(), b"replacement");
    }
    #[tokio::test]
    async fn broad_existing_parent_permissions_are_rejected_without_chmod() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::set_permissions(dir.path(), std::fs::Permissions::from_mode(0o755)).unwrap();
        assert!(
            SocketBinding::bind(&dir.path().join("shelf.sock"))
                .await
                .is_err()
        );
        assert_eq!(std::fs::metadata(dir.path()).unwrap().mode() & 0o777, 0o755);
    }
}
