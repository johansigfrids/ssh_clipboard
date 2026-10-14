//! Explicit startup registration, with rollback to actual OS state if saving fails.
use serde::Serialize;

pub trait Registry {
    fn enabled(&self) -> Result<bool, String>;
    fn set_enabled(&self, enabled: bool) -> Result<(), String>;
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Status {
    pub available: bool,
    pub enabled: Option<bool>,
    pub desired: bool,
    pub detail: &'static str,
}

pub fn status(registry: &impl Registry, installed: bool, desired: bool) -> Status {
    if !installed {
        return Status {
            available: false,
            enabled: None,
            desired,
            detail: "Start at login is available in installed release builds.",
        };
    }
    match registry.enabled() {
        Ok(enabled) => Status {
            available: true,
            enabled: Some(enabled),
            desired,
            detail: if enabled == desired {
                "Startup entry reflects the saved choice."
            } else {
                "The saved choice differs from this app's startup entry. Choose whether to enable it here."
            },
        },
        Err(_) => Status {
            available: true,
            enabled: None,
            desired,
            detail: "Could not read this app's startup entry.",
        },
    }
}

pub fn change<T>(
    registry: &impl Registry,
    desired: bool,
    commit: impl FnOnce() -> Result<T, String>,
) -> Result<T, String> {
    let previous = registry.enabled()?;
    let restore =
        || registry.set_enabled(previous).is_ok() && registry.enabled().ok() == Some(previous);
    if previous != desired
        && (registry.set_enabled(desired).is_err() || registry.enabled().ok() != Some(desired))
    {
        let message = if restore() {
            "Could not change startup. The previous entry was restored; saved settings were left unchanged."
        } else {
            "Could not change or restore startup. Saved settings were left unchanged; the startup entry needs repair."
        };
        return Err(message.into());
    }
    match commit() {
        Ok(value) => Ok(value),
        Err(error) => {
            let restored = previous == desired || restore();
            Err(format!(
                "{error} {}",
                if restored {
                    "The previous startup entry was preserved."
                } else {
                    "The startup entry could not be restored and needs repair."
                }
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::{Cell, RefCell};
    struct Fake {
        actual: Cell<bool>,
        calls: RefCell<Vec<bool>>,
        fail_enable: bool,
        fail_restore: bool,
    }
    impl Registry for Fake {
        fn enabled(&self) -> Result<bool, String> {
            Ok(self.actual.get())
        }
        fn set_enabled(&self, enabled: bool) -> Result<(), String> {
            self.calls.borrow_mut().push(enabled);
            if (enabled && self.fail_enable) || (!enabled && self.fail_restore) {
                return Err("synthetic failure".into());
            }
            self.actual.set(enabled);
            Ok(())
        }
    }
    fn registry() -> Fake {
        Fake {
            actual: Cell::new(false),
            calls: RefCell::new(vec![]),
            fail_enable: false,
            fail_restore: false,
        }
    }
    #[test]
    fn reading_a_retained_enabled_preference_never_registers_startup() {
        let registry = registry();
        let status = status(&registry, true, true);
        assert_eq!(status.enabled, Some(false));
        assert!(status.desired);
        assert!(registry.calls.borrow().is_empty());
    }
    #[test]
    fn save_failure_restores_actual_state_even_when_the_saved_preference_was_enabled() {
        let registry = registry();
        assert!(
            change(&registry, true, || Err::<(), _>("save failed".into()))
                .unwrap_err()
                .contains("preserved")
        );
        assert!(!registry.actual.get());
        assert_eq!(*registry.calls.borrow(), vec![true, false]);
    }
    #[test]
    fn registration_failure_never_saves_and_failed_restore_is_reported() {
        let mut registry = registry();
        registry.fail_enable = true;
        registry.fail_restore = true;
        let saved = Cell::new(false);
        let error = change(&registry, true, || {
            saved.set(true);
            Ok(())
        })
        .unwrap_err();
        assert!(!saved.get());
        assert!(error.contains("needs repair"));
    }
    #[test]
    fn explicit_change_saves_once_and_unchanged_registration_is_not_rewritten() {
        let registry = registry();
        assert_eq!(change(&registry, true, || Ok(7)).unwrap(), 7);
        assert!(registry.actual.get());
        change(&registry, true, || Ok(())).unwrap();
        assert_eq!(*registry.calls.borrow(), vec![true]);
    }
}
