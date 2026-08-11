use std::sync::Mutex;

use serde::Serialize;
use tauri::{
    menu::{Menu, MenuItem},
    tray::{MouseButton, TrayIconBuilder, TrayIconEvent},
    AppHandle, Manager,
};
use tauri_plugin_global_shortcut::{
    Code, GlobalShortcutExt, Modifiers, Shortcut, ShortcutEvent, ShortcutState,
};

struct CapState {
    f12_registered: bool,
    left_registered: bool,
    right_registered: bool,
    f12_pressed: u32,
    page_pressed: u32,
}

impl Default for CapState {
    fn default() -> Self {
        Self {
            f12_registered: false,
            left_registered: false,
            right_registered: false,
            f12_pressed: 0,
            page_pressed: 0,
        }
    }
}

struct AppState {
    cap: Mutex<CapState>,
}

#[derive(Serialize)]
struct Status {
    transparent: bool,
    always_on_top: bool,
    visible: bool,
    f12_registered: bool,
    left_registered: bool,
    right_registered: bool,
    f12_pressed: u32,
    page_pressed: u32,
    tray_ok: bool,
}

#[tauri::command]
fn quit(app: AppHandle) {
    app.exit(0);
}

#[tauri::command]
fn status(app: AppHandle) -> Status {
    let state = app.state::<AppState>();
    let cap = state.cap.lock().unwrap();
    let window = app.get_webview_window("main");
    Status {
        transparent: true,
        always_on_top: window.as_ref().map(|w| w.is_always_on_top().unwrap_or(false)).unwrap_or(false),
        visible: window.as_ref().map(|w| w.is_visible().unwrap_or(false)).unwrap_or(false),
        f12_registered: cap.f12_registered,
        left_registered: cap.left_registered,
        right_registered: cap.right_registered,
        f12_pressed: cap.f12_pressed,
        page_pressed: cap.page_pressed,
        tray_ok: true,
    }
}

fn toggle_main(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        if w.is_visible().unwrap_or(false) {
            let _ = w.hide();
        } else {
            let _ = w.show();
            let _ = w.set_focus();
        }
    }
}

fn setup_tray(app: &AppHandle) -> tauri::Result<()> {
    let toggle = MenuItem::with_id(app, "toggle", "显示/隐藏窗口", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "退出", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&toggle, &quit])?;
    let icon = app
        .default_window_icon()
        .cloned()
        .ok_or_else(|| tauri::Error::AssetNotFound("default icon".into()))?;
    TrayIconBuilder::with_id("cap-check-tray")
        .icon(icon)
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "toggle" => toggle_main(app),
            "quit" => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                ..
            } = event
            {
                toggle_main(tray.app_handle());
            }
        })
        .build(app)?;
    Ok(())
}

fn shortcut_handler(app: &AppHandle, shortcut: &Shortcut, event: ShortcutEvent) {
    if event.state() != ShortcutState::Pressed {
        return;
    }
    let state = app.state::<AppState>();
    if shortcut.matches(Modifiers::empty(), Code::F12) {
        state.cap.lock().unwrap().f12_pressed += 1;
        toggle_main(app);
    } else if shortcut.matches(Modifiers::empty(), Code::ArrowLeft)
        || shortcut.matches(Modifiers::empty(), Code::ArrowRight)
    {
        state.cap.lock().unwrap().page_pressed += 1;
    }
}

fn setup_shortcuts(app: &AppHandle) -> tauri::Result<()> {
    use tauri_plugin_global_shortcut::Builder;

    let shortcuts = [
        Shortcut::new(Some(Modifiers::empty()), Code::F12),
        Shortcut::new(Some(Modifiers::empty()), Code::ArrowLeft),
        Shortcut::new(Some(Modifiers::empty()), Code::ArrowRight),
    ];

    let builder: Builder<tauri::Wry> = Builder::new();
    let plugin = builder
        .with_shortcuts(shortcuts)
        .map(|b| b.with_handler(shortcut_handler));
    match plugin {
        Ok(plugin) => app.plugin(plugin.build())?,
        Err(e) => log::error!("快捷键插件创建失败: {e}"),
    }

    let gs = app.global_shortcut();
    let state = app.state::<AppState>();
    let mut cap = state.cap.lock().unwrap();
    cap.f12_registered = gs.is_registered(shortcuts[0]);
    cap.left_registered = gs.is_registered(shortcuts[1]);
    cap.right_registered = gs.is_registered(shortcuts[2]);
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .manage(AppState {
            cap: Mutex::new(CapState::default()),
        })
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            if let Some(w) = app.get_webview_window("main") {
                let _ = w.show();
                let _ = w.set_focus();
            }
        }))
        .invoke_handler(tauri::generate_handler![status, quit])
        .setup(|app| {
            if cfg!(debug_assertions) {
                app.handle().plugin(
                    tauri_plugin_log::Builder::default()
                        .level(log::LevelFilter::Info)
                        .build(),
                )?;
            }
            setup_tray(app.handle())?;
            setup_shortcuts(app.handle())?;
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
