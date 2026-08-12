use serde::{Deserialize, Serialize};
use tauri::{
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    AppHandle, Manager, PhysicalPosition, PhysicalSize, RunEvent, WebviewUrl, WebviewWindow,
    WebviewWindowBuilder,
};
use tauri_plugin_autostart::MacosLauncher;

mod sonovel;

/// 主窗口（书架）标签。
const MAIN_LABEL: &str = "main";
/// 悬浮阅读窗口标签。
const READER_LABEL: &str = "reader";

/// 悬浮窗尺寸限制（逻辑像素）。
const READER_MIN_WIDTH: f64 = 280.0;
const READER_MAX_WIDTH: f64 = 1180.0;
const READER_MIN_HEIGHT: f64 = 22.0;
const READER_MAX_HEIGHT: f64 = 500.0;

/// 悬浮窗逻辑坐标（Tauri 侧一律使用逻辑像素，缩放由系统换算）。
#[derive(Serialize, Deserialize, Clone, Copy, Default)]
pub struct Bounds {
    x: f64,
    y: f64,
    width: f64,
    height: f64,
}

// ---------------------------------------------------------------------------
// 文件读写
// ---------------------------------------------------------------------------

/// 读取文本文件：自动识别 UTF-8 BOM，失败时按 GBK 解码（兼容旧版 TXT 体验）。
#[tauri::command]
fn read_text_file(path: String) -> Result<String, String> {
    let bytes = std::fs::read(&path).map_err(|e| e.to_string())?;
    if bytes.starts_with(&[0xEF, 0xBB, 0xBF]) {
        return Ok(String::from_utf8_lossy(&bytes[3..]).into_owned());
    }
    match String::from_utf8(bytes.clone()) {
        Ok(s) => Ok(s),
        Err(_) => {
            let (decoded, _, _) = encoding_rs::GBK.decode(&bytes);
            Ok(decoded.into_owned())
        }
    }
}

/// 读取文件原始字节（EPUB/MOBI 解析用）。
#[tauri::command]
fn read_file_binary(path: String) -> Result<Vec<u8>, String> {
    std::fs::read(&path).map_err(|e| e.to_string())
}

/// 读取文件修改时间（毫秒时间戳），失败返回 null。
#[tauri::command]
fn get_file_modified_time(path: String) -> Result<Option<u64>, String> {
    let meta = std::fs::metadata(&path).map_err(|e| e.to_string())?;
    let mtime = meta
        .modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_millis() as u64);
    Ok(mtime)
}

/// 写入文本文件（备份/导出用）。
#[tauri::command]
fn write_text_file(path: String, content: String) -> Result<(), String> {
    std::fs::write(&path, content).map_err(|e| e.to_string())
}

// ---------------------------------------------------------------------------
// 文件对话框
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
struct DialogFilter {
    name: String,
    extensions: Vec<String>,
}

/// 打开文件选择对话框（支持多选），返回选中的文件路径列表。
#[tauri::command]
fn pick_open_files(
    app: AppHandle,
    title: Option<String>,
    filters: Vec<DialogFilter>,
    multiple: bool,
) -> Result<Vec<String>, String> {
    use tauri_plugin_dialog::DialogExt;
    let mut dlg = app.dialog().file();
    if let Some(t) = &title {
        dlg = dlg.set_title(t);
    }
    for f in &filters {
        let exts: Vec<&str> = f.extensions.iter().map(String::as_str).collect();
        dlg = dlg.add_filter(&f.name, &exts);
    }
    let picked = if multiple {
        dlg.blocking_pick_files()
    } else {
        dlg.blocking_pick_file().map(|p| vec![p])
    };
    Ok(picked
        .unwrap_or_default()
        .into_iter()
        .filter_map(|p| p.into_path().ok())
        .map(|p| p.to_string_lossy().into_owned())
        .collect())
}

/// 保存文件对话框，返回用户选择的路径（取消返回 null）。
#[tauri::command]
fn pick_save_file(
    app: AppHandle,
    title: Option<String>,
    default_name: Option<String>,
    filters: Vec<DialogFilter>,
) -> Result<Option<String>, String> {
    use tauri_plugin_dialog::DialogExt;
    let mut dlg = app.dialog().file();
    if let Some(t) = &title {
        dlg = dlg.set_title(t);
    }
    if let Some(n) = &default_name {
        dlg = dlg.set_file_name(n);
    }
    for f in &filters {
        let exts: Vec<&str> = f.extensions.iter().map(String::as_str).collect();
        dlg = dlg.add_filter(&f.name, &exts);
    }
    Ok(dlg
        .blocking_save_file()
        .and_then(|p| p.into_path().ok())
        .map(|p| p.to_string_lossy().into_owned()))
}

/// 在文件管理器中显示指定文件所在位置。
#[tauri::command]
fn reveal_in_folder(app: AppHandle, path: String) -> Result<(), String> {
    use tauri_plugin_opener::OpenerExt;
    app.opener()
        .reveal_item_in_dir(&path)
        .map_err(|e| e.to_string())
}

// ---------------------------------------------------------------------------
// 主窗口
// ---------------------------------------------------------------------------

/// 显示并聚焦主窗口（托盘/悬浮窗右键“显示主窗口”用）。
#[tauri::command]
fn show_main_window(app: AppHandle) -> Result<(), String> {
    if let Some(w) = app.get_webview_window(MAIN_LABEL) {
        let _ = w.unminimize();
        let _ = w.show();
        let _ = w.set_focus();
    }
    Ok(())
}

fn toggle_main(app: &AppHandle) {
    if let Some(w) = app.get_webview_window(MAIN_LABEL) {
        if w.is_visible().unwrap_or(false) {
            let _ = w.hide();
        } else {
            let _ = w.show();
            let _ = w.set_focus();
        }
    }
}

/// 主显示器工作区（逻辑像素），悬浮窗定位用。
#[tauri::command]
fn get_work_area(app: AppHandle) -> Bounds {
    let fallback = Bounds {
        x: 0.0,
        y: 0.0,
        width: 1920.0,
        height: 1080.0,
    };
    let Some(monitor) = app.primary_monitor().ok().flatten() else {
        return fallback;
    };
    let wa = monitor.work_area();
    let sf = monitor.scale_factor().max(0.1);
    Bounds {
        x: wa.position.x as f64 / sf,
        y: wa.position.y as f64 / sf,
        width: wa.size.width as f64 / sf,
        height: wa.size.height as f64 / sf,
    }
}

// ---------------------------------------------------------------------------
// 悬浮阅读窗口
// ---------------------------------------------------------------------------

fn get_reader(app: &AppHandle) -> Option<WebviewWindow> {
    app.get_webview_window(READER_LABEL)
}

/// 创建悬浮阅读窗口（透明、无边框、置顶、不进任务栏），创建后保持隐藏。
///
/// 窗口在程序启动时预创建一次（见 setup），运行期只做“显示/隐藏”。
/// 避免在 IPC 主线程中运行期创建窗口：WebView2 初始化需要泵消息循环，
/// 与等待命令返回的主线程互相等待会造成整窗卡死（D-029）。
fn ensure_reader_window(app: &AppHandle) -> Result<WebviewWindow, String> {
    if let Some(w) = get_reader(app) {
        return Ok(w);
    }
    WebviewWindowBuilder::new(
        app,
        READER_LABEL,
        WebviewUrl::App("src/reader/index.html".into()),
    )
    .title("隐阅阁阅读")
    .inner_size(READER_MIN_WIDTH, READER_MIN_HEIGHT)
    .decorations(false)
    .transparent(true)
    .always_on_top(true)
    .skip_taskbar(true)
    .resizable(false)
    .shadow(false)
    .focused(false)
    .build()
    .map_err(|e| e.to_string())
}

fn clamp(value: f64, min: f64, max: f64) -> f64 {
    value.max(min).min(max)
}

/// 把逻辑像素坐标应用到悬浮窗（含缩放换算）。
fn apply_bounds(app: &AppHandle, w: &WebviewWindow, bounds: Bounds) -> Result<(), String> {
    let sf = w.scale_factor().map_err(|e| e.to_string())?.max(0.1);
    let pos = PhysicalPosition::new(
        (bounds.x * sf).round() as i32,
        (bounds.y * sf).round() as i32,
    );
    let size = PhysicalSize::new(
        (bounds.width * sf).round() as u32,
        (bounds.height * sf).round() as u32,
    );
    w.set_position(pos).map_err(|e| e.to_string())?;
    w.set_size(size).map_err(|e| e.to_string())?;
    let _ = app;
    Ok(())
}

/// 显示悬浮阅读窗口；窗口不存在则先创建（兜底，正常情况下启动时已预创建）。
#[tauri::command]
fn reader_show(app: AppHandle, bounds: Option<Bounds>) -> Result<(), String> {
    let w = ensure_reader_window(&app)?;
    if let Some(b) = bounds {
        apply_bounds(&app, &w, b)?;
    }
    let _ = w.set_always_on_top(true);
    let _ = w.set_skip_taskbar(true);
    w.show().map_err(|e| e.to_string())?;
    let _ = w.set_focus();
    Ok(())
}

/// 关闭（隐藏）悬浮阅读窗口。
/// 窗口常驻不销毁：销毁后如需重建会回到运行期创建窗口的卡死场景（D-029）。
#[tauri::command]
fn reader_close(app: AppHandle) -> Result<(), String> {
    if let Some(w) = get_reader(&app) {
        let _ = w.hide();
    }
    Ok(())
}

/// 隐藏悬浮阅读窗口。
#[tauri::command]
fn reader_hide(app: AppHandle) -> Result<(), String> {
    if let Some(w) = get_reader(&app) {
        let _ = w.hide();
    }
    Ok(())
}

/// 调整悬浮窗大小（逻辑像素，自动夹在限制范围内）。
#[tauri::command]
fn reader_resize(app: AppHandle, width: f64, height: f64) -> Result<(), String> {
    let Some(w) = get_reader(&app) else {
        return Ok(());
    };
    let sf = w.scale_factor().map_err(|e| e.to_string())?.max(0.1);
    let size = PhysicalSize::new(
        (clamp(width, READER_MIN_WIDTH, READER_MAX_WIDTH) * sf).round() as u32,
        (clamp(height, READER_MIN_HEIGHT, READER_MAX_HEIGHT) * sf).round() as u32,
    );
    w.set_size(size).map_err(|e| e.to_string())
}

/// 移动悬浮窗到指定逻辑坐标。
#[tauri::command]
fn reader_move(app: AppHandle, x: f64, y: f64) -> Result<(), String> {
    let Some(w) = get_reader(&app) else {
        return Ok(());
    };
    let sf = w.scale_factor().map_err(|e| e.to_string())?.max(0.1);
    let pos = PhysicalPosition::new((x * sf).round() as i32, (y * sf).round() as i32);
    w.set_position(pos).map_err(|e| e.to_string())
}

/// 返回悬浮窗当前逻辑坐标与尺寸（拖动记忆用）。
#[tauri::command]
fn reader_position(app: AppHandle) -> Result<Bounds, String> {
    let Some(w) = get_reader(&app) else {
        return Ok(Bounds::default());
    };
    let sf = w.scale_factor().map_err(|e| e.to_string())?.max(0.1);
    let pos = w.outer_position().map_err(|e| e.to_string())?;
    let size = w.outer_size().map_err(|e| e.to_string())?;
    Ok(Bounds {
        x: pos.x as f64 / sf,
        y: pos.y as f64 / sf,
        width: size.width as f64 / sf,
        height: size.height as f64 / sf,
    })
}

/// 聚焦悬浮窗（鼠标进入时调用，恢复键盘响应）。
#[tauri::command]
fn reader_focus(app: AppHandle) -> Result<(), String> {
    if let Some(w) = get_reader(&app) {
        let _ = w.set_focus();
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// 开机启动
// ---------------------------------------------------------------------------

#[tauri::command]
fn set_autostart(app: AppHandle, enabled: bool) -> Result<(), String> {
    use tauri_plugin_autostart::ManagerExt;
    if enabled {
        app.autolaunch().enable().map_err(|e| e.to_string())
    } else {
        app.autolaunch().disable().map_err(|e| e.to_string())
    }
}

#[tauri::command]
fn get_autostart(app: AppHandle) -> Result<bool, String> {
    use tauri_plugin_autostart::ManagerExt;
    app.autolaunch().is_enabled().map_err(|e| e.to_string())
}

// ---------------------------------------------------------------------------
// 托盘与启动
// ---------------------------------------------------------------------------

fn setup_tray(app: &AppHandle) -> tauri::Result<()> {
    let toggle = MenuItem::with_id(app, "toggle", "显示/隐藏窗口", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "退出", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&toggle, &quit])?;
    let icon = app
        .default_window_icon()
        .cloned()
        .ok_or_else(|| tauri::Error::AssetNotFound("default icon".into()))?;
    TrayIconBuilder::with_id("hushreader-tray")
        .icon(icon)
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "toggle" => toggle_main(app),
            "quit" => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            // Windows 上一次单击会触发按下/抬起两个 Click 事件（D-028），
            // 只在“抬起”时切换一次，避免单击窗口闪烁（显示又隐藏）。
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                toggle_main(tray.app_handle());
            }
        })
        .build(app)?;
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            if let Some(w) = app.get_webview_window(MAIN_LABEL) {
                let _ = w.show();
                let _ = w.set_focus();
            }
        }))
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_autostart::init(MacosLauncher::LaunchAgent, None))
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .invoke_handler(tauri::generate_handler![
            read_text_file,
            read_file_binary,
            get_file_modified_time,
            write_text_file,
            pick_open_files,
            pick_save_file,
            reveal_in_folder,
            show_main_window,
            get_work_area,
            reader_show,
            reader_hide,
            reader_close,
            reader_resize,
            reader_move,
            reader_position,
            reader_focus,
            set_autostart,
            get_autostart,
            sonovel::backend_status,
            sonovel::backend_start,
            sonovel::backend_stop,
            sonovel::get_bookshelf_dir,
            sonovel::set_bookshelf_dir,
            sonovel::pick_directory,
            sonovel::sonovel_search,
            sonovel::sonovel_sources,
            sonovel::sonovel_local_books,
            sonovel::sonovel_fetch,
        ])
        .setup(|app| {
            if cfg!(debug_assertions) {
                app.handle().plugin(
                    tauri_plugin_log::Builder::default()
                        .level(log::LevelFilter::Info)
                        .build(),
                )?;
            }
            setup_tray(app.handle())?;
            // 预创建隐藏的悬浮阅读窗口（D-029）：运行期只做显示/隐藏，
            // 避免在 IPC 主线程中创建 WebView2 造成整窗卡死。
            let _ = ensure_reader_window(app.handle());
            // 启动 So Novel 下载后台（隐藏进程）。失败不阻断主程序，
            // 由前端在需要时提示"下载后台未就绪"并提供重试。
            if let Err(e) = sonovel::init(app.handle()) {
                eprintln!("[sonovel] 后台启动失败：{e}");
            }
            // 主窗口关闭时隐藏到托盘（D-012），真正退出走托盘菜单。
            if let Some(main) = app.get_webview_window(MAIN_LABEL) {
                let handle = app.handle().clone();
                main.on_window_event(move |event| {
                    if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                        api.prevent_close();
                        if let Some(w) = handle.get_webview_window(MAIN_LABEL) {
                            let _ = w.hide();
                        }
                    }
                });
            }
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app, event| {
            // 真正退出时关闭 So Novel 后台进程，避免残留 java 进程。
            if let RunEvent::Exit = event {
                sonovel::shutdown(app);
            }
        });
}
