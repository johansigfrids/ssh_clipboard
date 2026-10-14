//! Track actual registrations separately from preferences, including failed rollback cleanup.
use crate::model::SessionSettings;
use tauri_plugin_global_shortcut::Shortcut;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    Open,
    Share,
    Copy,
}

impl Action {
    fn label(self) -> &'static str {
        match self {
            Self::Open => "Open panel",
            Self::Share => "Share current clipboard",
            Self::Copy => "Copy shared clipboard",
        }
    }
}

#[derive(Clone, Debug)]
struct Binding {
    shortcut: Shortcut,
    text: String,
    // A registration left behind by failed rollback must never activate a new action.
    action: Option<Action>,
}

pub trait Registry {
    fn register(&mut self, shortcut: Shortcut) -> Result<(), ()>;
    fn unregister(&mut self, shortcut: Shortcut) -> Result<(), ()>;
}

#[derive(Default, Clone, Debug)]
pub struct Bindings(Vec<Binding>);

impl Bindings {
    pub fn requested(settings: &SessionSettings) -> Result<Self, String> {
        let mut bindings = Vec::<Binding>::new();
        let mut add = |action: Action, text: &str| -> Result<(), String> {
            let shortcut: Shortcut = text.parse().map_err(|_| {
                format!(
                    "{}: choose a valid shortcut with a modifier and a key.",
                    action.label()
                )
            })?;
            if shortcut.mods.is_empty() {
                return Err(format!("{} needs a modifier and a key.", action.label()));
            }
            if let Some(previous) = bindings.iter().find(|b| b.shortcut.id() == shortcut.id()) {
                return Err(format!(
                    "{} and {} need different shortcuts.",
                    previous.action.unwrap().label(),
                    action.label()
                ));
            }
            bindings.push(Binding {
                shortcut,
                text: text.into(),
                action: Some(action),
            });
            Ok(())
        };
        if settings.shortcut_enabled {
            add(Action::Open, &settings.open_shortcut)?;
        }
        if settings.direct_shortcuts_enabled {
            for (action, text) in [
                (Action::Share, &settings.direct_share_shortcut),
                (Action::Copy, &settings.direct_copy_shortcut),
            ] {
                if let Some(text) = text {
                    add(action, text)?;
                }
            }
            if settings.direct_share_shortcut.is_none() && settings.direct_copy_shortcut.is_none() {
                return Err(
                    "Record at least one direct shortcut or turn direct shortcuts off.".into(),
                );
            }
        }
        Ok(Self(bindings))
    }

    pub fn action(&self, id: u32) -> Option<Action> {
        self.0
            .iter()
            .find(|b| b.shortcut.id() == id)
            .and_then(|b| b.action)
    }

    pub fn open_label(&self) -> Option<String> {
        self.0
            .iter()
            .find(|b| b.action == Some(Action::Open))
            .map(|b| b.text.clone())
    }

    pub fn suspend_direct(&mut self) {
        for binding in &mut self.0 {
            if binding.action != Some(Action::Open) {
                binding.action = None;
            }
        }
    }

    /// Register additions first; action swaps reuse existing registrations. Failed changes
    /// reconcile to the original set and retain an honest inventory if OS cleanup fails.
    pub fn replace(
        &mut self,
        desired: Self,
        registry: &mut impl Registry,
    ) -> Result<String, String> {
        let previous = self.clone();
        let result = self.reconcile(&desired, registry);
        if let Err(error) = result {
            let restored = self.reconcile(&previous, registry).is_ok();
            for binding in &mut self.0 {
                binding.action = previous.action(binding.shortcut.id());
            }
            return Err(if restored {
                error
            } else {
                format!(
                    "{error} Some shortcut registrations could not be restored. Quit and reopen before using direct shortcuts."
                )
            });
        }
        let count = desired.0.len();
        *self = desired;
        Ok(if count == 0 {
            "Disabled".into()
        } else {
            format!("Registered {count} shortcut(s)")
        })
    }

    fn reconcile(&mut self, desired: &Self, registry: &mut impl Registry) -> Result<(), String> {
        for binding in &desired.0 {
            if !self
                .0
                .iter()
                .any(|b| b.shortcut.id() == binding.shortcut.id())
            {
                registry.register(binding.shortcut).map_err(|_| format!("{} could not be registered. It may be in use; choose another or disable it.", binding.action.map_or("Shortcut", Action::label)))?;
                let mut pending = binding.clone();
                pending.action = None;
                self.0.push(pending);
            }
        }
        let mut index = 0;
        while index < self.0.len() {
            let binding = &self.0[index];
            if desired
                .0
                .iter()
                .any(|b| b.shortcut.id() == binding.shortcut.id())
            {
                index += 1;
            } else {
                registry
                    .unregister(binding.shortcut)
                    .map_err(|_| "Could not remove an existing shortcut.".to_string())?;
                self.0.remove(index);
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[derive(Default)]
    struct Fake {
        registered: HashSet<u32>,
        reject_add: Option<u32>,
        reject_remove: Option<u32>,
        calls: usize,
    }
    impl Registry for Fake {
        fn register(&mut self, shortcut: Shortcut) -> Result<(), ()> {
            self.calls += 1;
            if self.reject_add == Some(shortcut.id()) || !self.registered.insert(shortcut.id()) {
                Err(())
            } else {
                Ok(())
            }
        }
        fn unregister(&mut self, shortcut: Shortcut) -> Result<(), ()> {
            self.calls += 1;
            if self.reject_remove == Some(shortcut.id()) {
                Err(())
            } else {
                self.registered.remove(&shortcut.id());
                Ok(())
            }
        }
    }
    fn id(text: &str) -> u32 {
        text.parse::<Shortcut>().unwrap().id()
    }
    fn defaults() -> SessionSettings {
        SessionSettings {
            shortcut_enabled: true,
            ..Default::default()
        }
    }
    fn direct() -> SessionSettings {
        SessionSettings {
            direct_shortcuts_enabled: true,
            direct_share_shortcut: Some("Control+Alt+C".into()),
            direct_copy_shortcut: Some("Control+Alt+V".into()),
            ..defaults()
        }
    }

    #[test]
    fn retained_bindings_stay_inert_until_enabled_and_aliases_conflict() {
        let mut settings = direct();
        settings.direct_share_shortcut = Some(settings.open_shortcut.clone());
        settings.direct_shortcuts_enabled = false;
        let bindings = Bindings::requested(&settings).unwrap();
        assert_eq!(bindings.0.len(), 1);
        settings.direct_shortcuts_enabled = true;
        assert!(
            Bindings::requested(&settings)
                .unwrap_err()
                .contains("different shortcuts")
        );
        settings.open_shortcut = "Ctrl+Alt+V".into();
        settings.direct_share_shortcut = None;
        assert!(Bindings::requested(&settings).is_err());
    }

    #[test]
    fn swapping_actions_does_not_reregister_the_same_keys() {
        let mut settings = direct();
        let mut bindings = Bindings::default();
        let mut os = Fake::default();
        bindings
            .replace(Bindings::requested(&settings).unwrap(), &mut os)
            .unwrap();
        let calls = os.calls;
        std::mem::swap(
            &mut settings.direct_share_shortcut,
            &mut settings.direct_copy_shortcut,
        );
        bindings
            .replace(Bindings::requested(&settings).unwrap(), &mut os)
            .unwrap();
        assert_eq!(os.calls, calls);
        assert_eq!(bindings.action(id("Ctrl+Alt+V")), Some(Action::Share));
    }

    #[test]
    fn partial_registration_failure_restores_original_os_set_and_actions() {
        let mut bindings = Bindings::default();
        let mut os = Fake::default();
        bindings
            .replace(Bindings::requested(&defaults()).unwrap(), &mut os)
            .unwrap();
        let original = os.registered.clone();
        os.reject_add = Some(id("Ctrl+Alt+V"));
        assert!(
            bindings
                .replace(Bindings::requested(&direct()).unwrap(), &mut os)
                .is_err()
        );
        assert_eq!(os.registered, original);
        assert_eq!(bindings.action(id("Ctrl+Alt+C")), None);
        assert!(bindings.open_label().is_some());
    }

    #[test]
    fn failed_cleanup_never_activates_a_new_share_binding() {
        let mut bindings = Bindings::default();
        let mut os = Fake {
            reject_add: Some(id("Ctrl+Alt+V")),
            reject_remove: Some(id("Ctrl+Alt+C")),
            ..Default::default()
        };
        let error = bindings
            .replace(Bindings::requested(&direct()).unwrap(), &mut os)
            .unwrap_err();
        assert!(error.contains("Quit and reopen"));
        assert_eq!(bindings.action(id("Ctrl+Alt+C")), None);
        assert!(os.registered.contains(&id("Ctrl+Alt+C")));
        assert!(
            bindings
                .0
                .iter()
                .any(|b| b.shortcut.id() == id("Ctrl+Alt+C"))
        );
    }

    #[test]
    fn failed_old_key_removal_restores_old_actions_after_adding_new_keys() {
        let mut bindings = Bindings::default();
        let mut os = Fake::default();
        bindings
            .replace(Bindings::requested(&defaults()).unwrap(), &mut os)
            .unwrap();
        let original = os.registered.clone();
        os.reject_remove = original.iter().copied().next();
        let settings = SessionSettings {
            open_shortcut: "Ctrl+Alt+P".into(),
            ..defaults()
        };
        assert!(
            bindings
                .replace(Bindings::requested(&settings).unwrap(), &mut os)
                .is_err()
        );
        assert_eq!(os.registered, original);
        assert_eq!(bindings.action(id("Ctrl+Alt+P")), None);
        assert!(bindings.open_label().is_some());
    }
}
