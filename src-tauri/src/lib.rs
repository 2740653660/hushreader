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

/// 老板键默认键（F12 在这台验证机上被其他程序占用，改为 F9，见 docs/DECISIONS.md D-023）。
const BOSS_CODE: Code = Code::F9;

struct CapState {
    boss_registered: bool,
    left_registered: bool,
    right_registered: bool,
    boss_conflict: bool,
    left_conflict: bool,
    right_conflict: bool,
    boss_pressed: u32,
    page_pressed: u32,
}

impl Default for CapState {
    fn default() -> Self {
        Self {
            boss_registered: false,
            left_registered: false,
            right_registered: false,
            boss_conflict: false,
            left_conflict: false,
            right_conflict: false,
            boss_pressed: 0,
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
    boss_registered: bool,
    left_registered: bool,
    right_registered: bool,
    boss_conflict: bool,
    left_conflict: bool,
    right_conflict: bool,
    boss_pressed: u32,
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
        boss_registered: cap.boss_registered,
        left_registered: cap.left_registered,
        right_registered: cap.right_registered,
        boss_conflict: cap.boss_conflict,
        left_conflict: cap.left_conflict,
        right_conflict: cap.right_conflict,
        boss_pressed: cap.boss_pressed,
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
    if shortcut.matches(Modifiers::empty(), BOSS_CODE) {
        state.cap.lock().unwrap().boss_pressed += 1;
        toggle_main(app);
    } else if shortcut.matches(Modifiers::empty(), Code::ArrowLeft)
        || shortcut.matches(Modifiers::empty(), Code::ArrowRight)
    {
        state.cap.lock().unwrap().page_pressed += 1;
    }
}

/// 逐个注册全局快捷键：某个键被其他程序占用时只标记冲突并跳过，
/// 不影响其他键和程序启动（D-023）。
fn setup_shortcuts(app: &AppHandle) -> tauri::Result<()> {
    use tauri_plugin_global_shortcut::Builder;

    // 不通过 with_shortcuts 预注册任何键，插件安装必然成功。
    app.plugin(Builder::new().build())?;

    let shortcuts = [
        Shortcut::new(Some(Modifiers::empty()), BOSS_CODE),
        Shortcut::new(Some(Modifiers::empty()), Code::ArrowLeft),
        Shortcut::new(Some(Modifiers::empty()), Code::ArrowRight),
    ];

    let gs = app.global_shortcut();
    let state = app.state::<AppState>();
    let mut cap = state.cap.lock().unwrap();
    for i in 0..shortcuts.len() {
        let shortcut = shortcuts[i];
        match gs.on_shortcut(shortcut, shortcut_handler) {
            Ok(()) => {}
            Err(e) => {
                log::warn!("快捷键 {shortcut:?} 注册失败（可能被其他程序占用）: {e}");
                if shortcut.matches(Modifiers::empty(), BOSS_CODE) {
                    cap.boss_conflict = true;
                } else if shortcut.matches(Modifiers::empty(), Code::ArrowLeft) {
                    cap.left_conflict = true;
                } else if shortcut.matches(Modifiers::empty(), Code::ArrowRight) {
                    cap.right_conflict = true;
                }
            }
        }
    }
    cap.boss_registered = gs.is_registered(shortcuts[0]);
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
