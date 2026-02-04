use crate::agent::{
    AgentConfig, autostart, default_agent_config, load_config, store_config, validate_config,
};
use crate::agent::{agent_peek, agent_pull, agent_push};
use crate::agent::{hotkey, notify};
use eyre::{Result, WrapErr, eyre};
use global_hotkey::{GlobalHotKeyEvent, GlobalHotKeyManager, HotKeyState};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use tao::event::{Event, StartCause};
use tao::event_loop::{ControlFlow, EventLoopBuilder, EventLoopProxy};
use tokio::runtime::Runtime;
use tray_icon::menu::{CheckMenuItem, Menu, MenuEvent, MenuId, MenuItem};
use tray_icon::{Icon, TrayIcon, TrayIconBuilder};

#[derive(Debug, Clone)]
enum UserEvent {
    Menu(MenuId),
    Hotkey { id: u32, state: HotKeyState },
    OperationOk(&'static str),
    OperationErr(&'static str, String),
}

pub fn run_agent(no_tray: bool, no_hotkeys: bool, autostart: bool) -> Result<()> {
    let config = load_config().unwrap_or_else(|_| default_agent_config());
    store_config(&config).ok();
    validate_config(&config)
        .wrap_err("invalid config; run `ssh_clipboard config show` and edit")?;

    let instance = single_instance::SingleInstance::new("ssh_clipboard_agent")
        .map_err(|err| eyre!("failed to create single instance lock: {err}"))?;
    if !instance.is_single() {
        if autostart {
            return Ok(());
        }
        return Err(eyre!("ssh_clipboard agent is already running"));
    }

    let runtime = Runtime::new().wrap_err("failed to create tokio runtime")?;
    let config = Arc::new(Mutex::new(config));
    let operation_running = Arc::new(AtomicBool::new(false));

    let event_loop = EventLoopBuilder::<UserEvent>::with_user_event().build();
    let proxy = event_loop.create_proxy();

    MenuEvent::set_event_handler(Some({
        let proxy = proxy.clone();
        move |event: MenuEvent| {
            let _ = proxy.send_event(UserEvent::Menu(event.id().clone()));
        }
    }));

    GlobalHotKeyEvent::set_event_handler(Some({
        let proxy = proxy.clone();
        move |event: GlobalHotKeyEvent| {
            let _ = proxy.send_event(UserEvent::Hotkey {
                id: event.id,
                state: event.state,
            });
        }
    }));

    let mut tray_state: Option<TrayState> = None;
    let mut hotkeys: Option<Hotkeys> = None;

    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Wait;

        match event {
            Event::NewEvents(StartCause::Init) => {
                if config.lock().unwrap().autostart_enabled {
                    let _ = autostart::refresh();
                }

                if !no_tray {
                    match build_tray(config.clone()) {
                        Ok(state) => tray_state = Some(state),
                        Err(err) => notify::notify("ssh_clipboard", &format!("tray failed: {err}")),
                    }
                }

                if !no_hotkeys {
                    match register_hotkeys(config.clone()) {
                        Ok(state) => hotkeys = Some(state),
                        Err(err) => {
                            notify::notify("ssh_clipboard", &format!("hotkeys failed: {err}"))
                        }
                    }
                }

                notify::notify("ssh_clipboard", "agent started");
            }

            Event::UserEvent(UserEvent::Menu(id)) => {
                if let Some(state) = &tray_state {
                    let ctx = MenuContext {
                        tray: state,
                        hotkeys: &mut hotkeys,
                        runtime: &runtime,
                        proxy: proxy.clone(),
                        config: config.clone(),
                        running: operation_running.clone(),
                        control_flow,
                    };
                    handle_menu(id, ctx);
                }
            }

            Event::UserEvent(UserEvent::Hotkey { id, state }) => {
                if state != HotKeyState::Pressed {
                    return;
                }
                if let Some(hk) = &hotkeys {
                    if id == hk.push_id {
                        start_operation(
                            "push",
                            &runtime,
                            proxy.clone(),
                            config.clone(),
                            operation_running.clone(),
                            |cfg| async move { agent_push(&cfg).await },
                        );
                    } else if id == hk.pull_id {
                        start_operation(
                            "pull",
                            &runtime,
                            proxy.clone(),
                            config.clone(),
                            operation_running.clone(),
                            |cfg| async move { agent_pull(&cfg).await },
                        );
                    }
                }
            }

            Event::UserEvent(UserEvent::OperationOk(name)) => {
                if name != "peek" {
                    notify::notify("ssh_clipboard", &format!("{name}: ok"));
                }
            }

            Event::UserEvent(UserEvent::OperationErr(name, message)) => {
                notify::notify("ssh_clipboard error", &format!("{name}: {message}"));
            }

            _ => {}
        }
    });
}

struct TrayState {
    _tray: TrayIcon,
    menu_ids: MenuIds,
    autostart: CheckMenuItem,
}

struct MenuIds {
    push: MenuId,
    pull: MenuId,
    peek: MenuId,
    autostart: MenuId,
    restore_defaults: MenuId,
    show_config: MenuId,
    quit: MenuId,
}

fn build_tray(config: Arc<Mutex<AgentConfig>>) -> Result<TrayState> {
    let menu = Menu::new();
    let push = MenuItem::new("Push", true, None);
    let pull = MenuItem::new("Pull", true, None);
    let peek = MenuItem::new("Peek", true, None);

    let enabled = config.lock().unwrap().autostart_enabled;
    let autostart = CheckMenuItem::new("Start at login", true, enabled, None);

    let restore_defaults = MenuItem::new("Restore Defaults", true, None);
    let show_config = MenuItem::new("Show Config Path", true, None);
    let quit = MenuItem::new("Quit", true, None);

    menu.append_items(&[
        &push,
        &pull,
        &peek,
        &autostart,
        &restore_defaults,
        &show_config,
        &quit,
    ])
    .map_err(|err| eyre!(err.to_string()))?;

    let tray = TrayIconBuilder::new()
        .with_tooltip("ssh_clipboard")
        .with_menu(Box::new(menu))
        .with_icon(load_tray_icon()?)
        .build()
        .map_err(|err| eyre!(err.to_string()))?;

    let autostart_id = autostart.id().clone();
    Ok(TrayState {
        _tray: tray,
        autostart,
        menu_ids: MenuIds {
            push: push.id().clone(),
            pull: pull.id().clone(),
            peek: peek.id().clone(),
            autostart: autostart_id,
            restore_defaults: restore_defaults.id().clone(),
            show_config: show_config.id().clone(),
            quit: quit.id().clone(),
        },
    })
}

struct MenuContext<'a> {
    tray: &'a TrayState,
    hotkeys: &'a mut Option<Hotkeys>,
    runtime: &'a Runtime,
    proxy: EventLoopProxy<UserEvent>,
    config: Arc<Mutex<AgentConfig>>,
    running: Arc<AtomicBool>,
    control_flow: &'a mut ControlFlow,
}

fn handle_menu(id: MenuId, ctx: MenuContext) {
    if id == ctx.tray.menu_ids.quit {
        *ctx.control_flow = ControlFlow::Exit;
        return;
    }

    if id == ctx.tray.menu_ids.show_config {
        match crate::agent::config_path() {
            Ok(path) => notify::notify("ssh_clipboard", &format!("config: {}", path.display())),
            Err(err) => notify::notify("ssh_clipboard", &format!("config path error: {err}")),
        }
        return;
    }

    if id == ctx.tray.menu_ids.restore_defaults {
        let mut cfg = ctx.config.lock().unwrap();
        let preserved_target = cfg.target.clone();
        let preserved_port = cfg.port;
        let preserved_identity = cfg.identity_file.clone();
        let preserved_ssh_options = cfg.ssh_options.clone();
        let preserved_max_size = cfg.max_size;
        let preserved_timeout = cfg.timeout_ms;
        let preserved_autostart = cfg.autostart_enabled;

        *cfg = default_agent_config();
        cfg.target = preserved_target;
        cfg.port = preserved_port;
        cfg.identity_file = preserved_identity;
        cfg.ssh_options = preserved_ssh_options;
        cfg.max_size = preserved_max_size;
        cfg.timeout_ms = preserved_timeout;
        cfg.autostart_enabled = preserved_autostart;

        let _ = store_config(&cfg);
        if let Some(hk) = ctx.hotkeys.as_mut()
            && let Err(err) = hk.update_from_config(&cfg)
        {
            notify::notify("ssh_clipboard", &format!("hotkey update failed: {err}"));
        }
        notify::notify("ssh_clipboard", "restored defaults");
        return;
    }

    if id == ctx.tray.menu_ids.autostart {
        let enable = ctx.tray.autostart.is_checked();
        {
            let mut cfg = ctx.config.lock().unwrap();
            cfg.autostart_enabled = enable;
            let _ = store_config(&cfg);
        }
        let result = if enable {
            autostart::enable()
        } else {
            autostart::disable()
        };
        match result {
            Ok(()) => notify::notify(
                "ssh_clipboard",
                if enable {
                    "autostart enabled"
                } else {
                    "autostart disabled"
                },
            ),
            Err(err) => notify::notify("ssh_clipboard", &format!("autostart error: {err}")),
        }
        return;
    }

    if id == ctx.tray.menu_ids.push {
        start_operation(
            "push",
            ctx.runtime,
            ctx.proxy.clone(),
            ctx.config.clone(),
            ctx.running.clone(),
            |cfg| async move { agent_push(&cfg).await },
        );
        return;
    }
    if id == ctx.tray.menu_ids.pull {
        start_operation(
            "pull",
            ctx.runtime,
            ctx.proxy.clone(),
            ctx.config.clone(),
            ctx.running.clone(),
            |cfg| async move { agent_pull(&cfg).await },
        );
        return;
    }
    if id == ctx.tray.menu_ids.peek {
        start_operation(
            "peek",
            ctx.runtime,
            ctx.proxy,
            ctx.config,
            ctx.running,
            |cfg| async move {
                let result = agent_peek(&cfg).await?;
                notify::notify("ssh_clipboard peek", &result);
                Ok(())
            },
        );
    }
}

struct Hotkeys {
    _manager: GlobalHotKeyManager,
    push: global_hotkey::hotkey::HotKey,
    pull: global_hotkey::hotkey::HotKey,
    push_id: u32,
    pull_id: u32,
}

fn register_hotkeys(config: Arc<Mutex<AgentConfig>>) -> Result<Hotkeys> {
    let manager = GlobalHotKeyManager::new().map_err(|err| eyre!(err.to_string()))?;
    let cfg = config.lock().unwrap().clone();

    let push = hotkey::parse_hotkey(&cfg.hotkeys.push)?;
    let pull = hotkey::parse_hotkey(&cfg.hotkeys.pull)?;
    let push_id = push.id();
    let pull_id = pull.id();

    manager
        .register_all(&[push, pull])
        .map_err(|err| eyre!(err.to_string()))?;

    Ok(Hotkeys {
        _manager: manager,
        push,
        pull,
        push_id,
        pull_id,
    })
}

impl Hotkeys {
    fn update_from_config(&mut self, cfg: &AgentConfig) -> Result<()> {
        let new_push = hotkey::parse_hotkey(&cfg.hotkeys.push)?;
        let new_pull = hotkey::parse_hotkey(&cfg.hotkeys.pull)?;

        self._manager
            .unregister_all(&[self.push, self.pull])
            .map_err(|err| eyre!(err.to_string()))?;
        self._manager
            .register_all(&[new_push, new_pull])
            .map_err(|err| eyre!(err.to_string()))?;

        self.push = new_push;
        self.pull = new_pull;
        self.push_id = new_push.id();
        self.pull_id = new_pull.id();
        Ok(())
    }
}

fn start_operation<F, Fut>(
    name: &'static str,
    runtime: &Runtime,
    proxy: EventLoopProxy<UserEvent>,
    config: Arc<Mutex<AgentConfig>>,
    running: Arc<AtomicBool>,
    f: F,
) where
    F: FnOnce(AgentConfig) -> Fut + Send + 'static,
    Fut: std::future::Future<Output = Result<()>> + Send + 'static,
{
    if running.swap(true, Ordering::SeqCst) {
        let _ = proxy.send_event(UserEvent::OperationErr(name, "already running".to_string()));
        return;
    }

    let cfg = config.lock().unwrap().clone();
    runtime.spawn(async move {
        let result = f(cfg).await;
        running.store(false, Ordering::SeqCst);
        match result {
            Ok(()) => {
                let _ = proxy.send_event(UserEvent::OperationOk(name));
            }
            Err(err) => {
                let _ = proxy.send_event(UserEvent::OperationErr(name, err.to_string()));
            }
        }
    });
}

fn load_tray_icon() -> Result<Icon> {
    static ICON_PNG: &[u8] =
        include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/assets/icon.png"));
    match icon_from_png(ICON_PNG) {
        Ok(icon) => Ok(icon),
        Err(err) => {
            tracing::warn!("failed to load tray icon from PNG: {err}");
            fallback_icon()
        }
    }
}

fn icon_from_png(bytes: &[u8]) -> Result<Icon> {
    let image = image::load_from_memory(bytes).wrap_err("decode png icon failed")?;
    let rgba = image.to_rgba8();
    let (width, height) = rgba.dimensions();
    Icon::from_rgba(rgba.into_raw(), width, height).map_err(|err| eyre!(err.to_string()))
}

fn fallback_icon() -> Result<Icon> {
    let size = 32u32;
    let mut rgba = Vec::with_capacity((size * size * 4) as usize);
    for y in 0..size {
        for x in 0..size {
            let border = x < 2 || y < 2 || x >= size - 2 || y >= size - 2;
            if border {
                rgba.extend_from_slice(&[0, 0, 0, 255]);
            } else {
                rgba.extend_from_slice(&[80, 160, 255, 255]);
            }
        }
    }
    Icon::from_rgba(rgba, size, size).map_err(|err| eyre!(err.to_string()))
}
