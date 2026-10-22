use super::tests::persisted_session;
use super::*;
use std::cell::{Cell, RefCell};

struct Startup {
    actual: Cell<Option<bool>>,
    reads: Cell<usize>,
    writes: RefCell<Vec<bool>>,
    fail: Option<bool>,
}
impl Startup {
    fn new(actual: Option<bool>) -> Self {
        Self {
            actual: Cell::new(actual),
            reads: Cell::new(0),
            writes: RefCell::new(vec![]),
            fail: None,
        }
    }
    fn access(&self) -> StartupAccess<'_> {
        StartupAccess {
            registry: self,
            available: true,
        }
    }
}
impl ssh_clipboard_desktop_prototype::startup::Registry for Startup {
    fn enabled(&self) -> Result<bool, String> {
        self.reads.set(self.reads.get() + 1);
        self.actual
            .get()
            .ok_or_else(|| "Startup state is unknown".into())
    }
    fn set_enabled(&self, enabled: bool) -> Result<(), String> {
        self.writes.borrow_mut().push(enabled);
        if self.fail == Some(enabled) {
            // Enabling may fail after a partial native write; failed restoration leaves it alone.
            if enabled {
                self.actual.set(Some(true));
            }
            return Err("Injected startup failure".into());
        }
        self.actual.set(Some(enabled));
        Ok(())
    }
}

#[test]
fn startup_only_save_commits_once_and_retains_preview_selection_and_pending_share() {
    let (_directory, store, mut session) = persisted_session();
    let original = session.saved_config.clone().unwrap();
    let startup = Startup::new(Some(true));
    let epoch = Uuid::from_u128(1);
    let selected = Uuid::from_u128(2);
    session.history.epoch = Some(epoch);
    session.history.selected = Some(selected);
    session.history.preview = session.history.preview.clone();
    session.pending_share = Some(PendingShare::new(
        epoch,
        selected,
        ShelfOrigin {
            device_id: Uuid::from_u128(3),
            name: "Fixture".into(),
        },
        ShelfRepresentation {
            id: 0,
            content_type: ShelfContent::Text,
            encoded_bytes: 6,
            dimensions: None,
            decoded_bytes: None,
        },
        crate::clipboard_payload::build_text_value("retained snapshot".into(), 1024).unwrap(),
        60000,
        None,
    ));
    let settings = session.settings.clone();
    apply_settings_draft(
        &mut session,
        &store,
        settings,
        Some(&original.revision.to_string()),
        Some(false),
        startup.access(),
        |_, _| Ok("Registered".into()),
    )
    .unwrap();
    let saved = store.load().unwrap().unwrap();
    assert_eq!(saved.revision, original.revision + 1);
    assert!(!saved.preferences.start_at_login);
    assert_eq!(saved.profiles, original.profiles);
    assert_eq!(*startup.writes.borrow(), [false]);
    assert_eq!(session.history.epoch, Some(epoch));
    assert_eq!(session.history.selected, Some(selected));
    assert!(session.history.preview.is_some());
    assert_eq!(
        session.pending_share.as_ref().unwrap().operation_id,
        selected
    );
    assert_eq!(
        session.history.preview.as_ref().unwrap().text.as_deref(),
        Some("retained snapshot")
    );
    assert!(session.connected);
}

#[test]
fn combined_apply_activates_explicit_startup_even_when_it_matches_the_saved_preference() {
    let (_directory, store, mut session) = persisted_session();
    let revision = session.saved_config.as_ref().unwrap().revision;
    let startup = Startup::new(Some(false)); // Retained saved choice is already true.
    let mut settings = session.settings.clone();
    settings.show_preview = false;
    settings.open_shortcut = "Ctrl+Shift+K".into();
    apply_settings_draft(
        &mut session,
        &store,
        settings,
        Some(&revision.to_string()),
        Some(true),
        startup.access(),
        |_, next| {
            assert_eq!(startup.actual.get(), Some(true));
            assert_eq!(next.open_shortcut, "Ctrl+Shift+K");
            Ok("Registered".into())
        },
    )
    .unwrap();
    let saved = store.load().unwrap().unwrap();
    assert!(saved.preferences.start_at_login);
    assert!(!saved.preferences.show_preview);
    assert_eq!(saved.revision, revision + 1);
    assert_eq!(*startup.writes.borrow(), [true]);
}

#[test]
fn unrelated_apply_does_not_read_or_repair_external_unknown_or_unavailable_startup() {
    for (actual, available) in [(None, true), (Some(false), true), (None, false)] {
        let (_directory, store, mut session) = persisted_session();
        let revision = session.saved_config.as_ref().unwrap().revision.to_string();
        let startup = Startup::new(actual);
        let mut settings = session.settings.clone();
        settings.show_preview = false;
        let mut access = startup.access();
        access.available = available;
        apply_settings_draft(
            &mut session,
            &store,
            settings,
            Some(&revision),
            None,
            access,
            |_, _| Ok("Registered".into()),
        )
        .unwrap();
        assert!(store.load().unwrap().unwrap().preferences.start_at_login);
        assert_eq!(startup.reads.get(), 0);
        assert!(startup.writes.borrow().is_empty());
    }
}

#[test]
fn validation_conflicts_and_unavailable_startup_fail_before_native_activation() {
    for fault in [
        "invalid",
        "invalid shortcut",
        "revision",
        "disk revision",
        "read only",
        "unavailable",
    ] {
        if fault == "invalid shortcut" && is_wayland() {
            continue; // Wayland retains inactive shortcut preferences without parsing them.
        }
        let (_directory, store, mut session) = persisted_session();
        let original = session.saved_config.clone().unwrap();
        let startup = Startup::new(Some(false));
        let mut settings = session.settings.clone();
        let mut revision = original.revision.to_string();
        if fault == "invalid" {
            settings.profile_name.clear();
        }
        if fault == "invalid shortcut" {
            settings.open_shortcut = "not a shortcut".into();
        }
        if fault == "revision" {
            revision = "999".into();
        }
        if fault == "read only" {
            session.settings_notice = Some("Repair settings".into());
        }
        if fault == "disk revision" {
            let mut external = original.clone();
            external.device.name = "External edit".into();
            store
                .prepare(Some(&original), external)
                .unwrap()
                .commit()
                .unwrap();
        }
        let disk = store.load().unwrap();
        let mut access = startup.access();
        access.available = fault != "unavailable";
        assert!(
            apply_settings_draft(
                &mut session,
                &store,
                settings,
                Some(&revision),
                Some(true),
                access,
                |_, _| panic!("must not activate shortcuts after {fault}")
            )
            .is_err(),
            "{fault}"
        );
        assert_eq!(startup.reads.get(), 0, "{fault}");
        assert!(startup.writes.borrow().is_empty());
        assert_eq!(store.load().unwrap(), disk);
        assert_eq!(session.saved_config, Some(original));
    }
}

#[test]
fn unknown_or_partially_failed_startup_abandons_combined_edits_before_shortcuts() {
    for actual in [None, Some(false)] {
        let (_directory, store, mut session) = persisted_session();
        let original = session.saved_config.clone().unwrap();
        let mut startup = Startup::new(actual);
        startup.fail = Some(true);
        let mut settings = session.settings.clone();
        settings.show_preview = false;
        assert!(
            apply_settings_draft(
                &mut session,
                &store,
                settings,
                Some(&original.revision.to_string()),
                Some(true),
                startup.access(),
                |_, _| panic!("startup failure must not activate shortcuts")
            )
            .is_err()
        );
        assert_eq!(startup.actual.get(), actual);
        assert_eq!(
            *startup.writes.borrow(),
            if actual.is_some() {
                vec![true, false]
            } else {
                vec![]
            }
        );
        assert_eq!(store.load().unwrap(), Some(original.clone()));
        assert_eq!(session.saved_config, Some(original));
    }
}

#[test]
fn shortcut_failure_after_startup_restores_actual_state_without_committing() {
    let (_directory, store, mut session) = persisted_session();
    let original = session.saved_config.clone().unwrap();
    let startup = Startup::new(Some(false));
    let settings = session.settings.clone();
    let error = apply_settings_draft(
        &mut session,
        &store,
        settings,
        Some(&original.revision.to_string()),
        Some(true),
        startup.access(),
        |_, _| {
            assert_eq!(startup.actual.get(), Some(true));
            Err("Shortcut conflict".into())
        },
    )
    .unwrap_err();
    assert!(error.contains("Shortcut conflict"));
    assert_eq!(*startup.writes.borrow(), [true, false]);
    assert_eq!(startup.actual.get(), Some(false));
    assert_eq!(store.load().unwrap(), Some(original.clone()));
    assert_eq!(session.saved_config, Some(original));
}

#[test]
fn commit_failure_rolls_back_both_activations_and_retains_saved_configuration() {
    let (directory, store, mut session) = persisted_session();
    let path = directory.path().join("settings.json");
    let bytes = std::fs::read(&path).unwrap();
    let original = session.saved_config.clone().unwrap();
    let startup = Startup::new(Some(false));
    let mut settings = session.settings.clone();
    settings.open_shortcut = "Ctrl+Shift+K".into();
    let mut activations = vec![];
    assert!(
        apply_settings_draft(
            &mut session,
            &store,
            settings,
            Some(&original.revision.to_string()),
            Some(true),
            startup.access(),
            |_, next| {
                activations.push(next.open_shortcut.clone());
                if activations.len() == 1 {
                    std::fs::remove_file(&path).unwrap();
                    std::fs::create_dir(&path).unwrap();
                } else {
                    std::fs::remove_dir(&path).unwrap();
                    std::fs::write(&path, &bytes).unwrap();
                }
                Ok("Registered".into())
            }
        )
        .is_err()
    );
    assert_eq!(
        activations,
        ["Ctrl+Shift+K", "CommandOrControl+Alt+Shift+S"]
    );
    assert_eq!(*startup.writes.borrow(), [true, false]);
    assert_eq!(startup.actual.get(), Some(false));
    assert_eq!(store.load().unwrap(), Some(original.clone()));
    assert_eq!(session.saved_config, Some(original));
    assert!(session.history.preview.is_some());
}

#[test]
fn failed_startup_and_shortcut_restoration_report_both_repair_needs() {
    let (directory, store, mut session) = persisted_session();
    let path = directory.path().join("settings.json");
    let original = session.saved_config.clone().unwrap();
    let mut startup = Startup::new(Some(false));
    startup.fail = Some(false);
    let mut settings = session.settings.clone();
    settings.direct_shortcuts_enabled = true;
    settings.direct_share_shortcut = Some("Ctrl+Alt+C".into());
    let mut first = true;
    let error = apply_settings_draft(
        &mut session,
        &store,
        settings,
        Some(&original.revision.to_string()),
        Some(true),
        startup.access(),
        |session, next| {
            if first {
                first = false;
                session.shortcuts = Bindings::requested(next).unwrap();
                std::fs::remove_file(&path).unwrap();
                std::fs::create_dir(&path).unwrap();
                Ok("Registered".into())
            } else {
                Err("Injected shortcut restoration failure".into())
            }
        },
    )
    .unwrap_err();
    assert!(error.contains("Direct actions are paused"));
    assert!(error.contains("startup entry could not be restored"));
    assert_eq!(session.saved_config, Some(original));
    assert_eq!(startup.actual.get(), Some(true));
    let share: Shortcut = "Ctrl+Alt+C".parse().unwrap();
    assert_eq!(session.shortcuts.action(share.id()), None);
}
