//! Release ownership explicitly before closing the handle. A forked child/duplicated
//! descriptor can otherwise keep the same open-file-description lock alive until exec.
use std::fs::{File, TryLockError};

#[derive(Debug)]
pub(crate) struct ExclusiveLock(File);
impl ExclusiveLock {
    pub fn acquire(file: File) -> Result<Self, TryLockError> {
        file.try_lock()?;
        Ok(Self(file))
    }
}
impl Drop for ExclusiveLock {
    fn drop(&mut self) {
        let _ = self.0.unlock();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn retained_descriptors_do_not_keep_finished_ownership_or_release_the_next_writer() {
        let file = tempfile::NamedTempFile::new().unwrap();
        let open = || {
            File::options()
                .read(true)
                .write(true)
                .open(file.path())
                .unwrap()
        };
        let first = open();
        let retained = first.try_clone().unwrap();
        let lock = ExclusiveLock::acquire(first).unwrap();
        assert!(matches!(
            ExclusiveLock::acquire(open()),
            Err(TryLockError::WouldBlock)
        ));
        drop(lock);
        let next = ExclusiveLock::acquire(open())
            .expect("the previous owner must unlock even while its duplicate remains open");
        drop(retained);
        assert!(matches!(
            ExclusiveLock::acquire(open()),
            Err(TryLockError::WouldBlock)
        ));
        drop(next);
        assert!(ExclusiveLock::acquire(open()).is_ok());
    }
}
