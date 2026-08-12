//! So Novel 下载后台：进程管理、配置生成与本机 Web 接口转发。
//!
//! 后台随应用启动/退出；前端不直接访问后台端口，搜索/下载全部经本模块转发，
//! 保证「后台通信限制为安全的本机访问」。

use std::{
    io::{BufRead, BufReader, Write},
    net::{TcpListener, TcpStream},
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    time::Duration,
};

use tauri::{AppHandle, Emitter, Manager, State};

#[cfg(windows)]
use std::os::windows::process::CommandExt;

/// Windows 下不弹出控制台窗口。
#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// 后台 Web 端口起始值（被占用时自动 +1 探测空闲端口）。
const BASE_PORT: u16 = 7765;
const MAX_PORT_TRY: u16 = 50;

// ---------------------------------------------------------------------------
// 状态
// ---------------------------------------------------------------------------

#[derive(Clone, Default)]
pub struct SonovelState {
    inner: Arc<Mutex<Option<SonovelBackend>>>,
}

struct SonovelBackend {
    child: Option<Child>,
    port: u16,
    /// 是否有下载任务进行中（一次只下一本）。
    fetching: Arc<AtomicBool>,
}

#[derive(serde::Serialize)]
pub struct BackendStatus {
    running: bool,
    port: u16,
}

// ---------------------------------------------------------------------------
// 路径与持久化
// ---------------------------------------------------------------------------

/// 后台资源目录：优先安装目录（打包后），否则回退源码 `resources/sonovel`（开发期）。
fn sonovel_resource_dir(app: &AppHandle) -> PathBuf {
    if let Ok(p) = app.path().resource_dir() {
        let cand = p.join("sonovel");
        if cand.join("app.jar").exists() {
            return cand;
        }
    }
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("resources")
        .join("sonovel")
}

fn settings_path(data_dir: &Path) -> PathBuf {
    data_dir.join("sonovel").join("settings.json")
}

/// 书库目录：由本模块持久化（下载后台的下载路径依赖它）。
fn load_bookshelf(data_dir: &Path) -> PathBuf {
    if let Ok(s) = std::fs::read_to_string(settings_path(data_dir)) {
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(&s) {
            if let Some(p) = v["bookshelfDir"].as_str() {
                return PathBuf::from(p);
            }
        }
    }
    data_dir.join("books")
}

fn save_bookshelf(data_dir: &Path, path: &str) -> Result<(), String> {
    let v = serde_json::json!({ "bookshelfDir": path });
    std::fs::write(settings_path(data_dir), v.to_string()).map_err(|e| e.to_string())
}

fn copy_dir_recursive(src: &Path, dst: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(dst)?;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let ty = entry.file_type()?;
        let target = dst.join(entry.file_name());
        if ty.is_dir() {
            copy_dir_recursive(&entry.path(), &target)?;
        } else {
            std::fs::copy(entry.path(), target)?;
        }
    }
    Ok(())
}

/// 把书源规则复制到数据目录（仅首次或缺失时）。规则文件留在数据目录，
/// 便于后续阶段 5 实现"书源规则软件内更新"。
fn ensure_rules(res: &Path, data: &Path) -> Result<(), String> {
    let src = res.join("rules");
    let dst = data.join("rules");
    if !src.exists() {
        return Err("下载后台缺少书源规则文件".into());
    }
    if dst.join("main.json").exists() {
        return Ok(());
    }
    copy_dir_recursive(&src, &dst).map_err(|e| format!("复制书源规则失败：{e}"))
}

/// 生成后台配置文件（download-path 指向书库目录，端口为探测出的空闲端口）。
fn write_config(data: &Path, bookshelf: &Path, port: u16) -> Result<(), String> {
    let ini = format!(
        "[global]\n\
         auto-update = 0\n\
         gh-proxy =\n\
         cf-bypass =\n\
         [download]\n\
         download-path = {}\n\
         extname = epub\n\
         txt-encoding = UTF-8\n\
         preserve-chapter-cache = 0\n\
         enable-progressbar = 1\n\
         [source]\n\
         language =\n\
         active-rules = main.json\n\
         source-id =\n\
         search-limit = 30\n\
         search-filter = 1\n\
         [crawl]\n\
         concurrency = 20\n\
         min-interval = 200\n\
         max-interval = 400\n\
         enable-retry = 1\n\
         max-retries = 3\n\
         retry-min-interval = 2000\n\
         retry-max-interval = 4000\n\
         [web]\n\
         enabled = 1\n\
         port = {}\n\
         [cookie]\n\
         qidian =\n\
         [proxy]\n\
         enabled = 0\n\
         host = 127.0.0.1\n\
         port =\n",
        bookshelf.display(),
        port
    );
    std::fs::write(data.join("config.ini"), ini).map_err(|e| e.to_string())
}

// ---------------------------------------------------------------------------
// 进程管理
// ---------------------------------------------------------------------------

/// 清理上次异常退出残留的后台进程（通过 pid 文件）。
#[cfg(windows)]
fn kill_pid_file(data: &Path) {
    let pf = data.join("backend.pid");
    if let Ok(s) = std::fs::read_to_string(&pf) {
        if let Ok(pid) = s.trim().parse::<u32>() {
            let _ = Command::new("taskkill")
                .args(["/PID", &pid.to_string(), "/F"])
                .creation_flags(CREATE_NO_WINDOW)
                .status();
        }
    }
    let _ = std::fs::remove_file(&pf);
}

#[cfg(not(windows))]
fn kill_pid_file(_data: &Path) {}

fn write_pid(data: &Path, pid: u32) {
    let _ = std::fs::write(data.join("backend.pid"), pid.to_string());
}

/// Windows 下 Tauri 的 resource_dir() 返回带 `\\?\` 前缀的扩展路径，
/// 直接传给子进程会被命令行解析破坏（java -jar 报 ClassNotFound）。
/// 统一剥离该前缀后再使用。
#[cfg(windows)]
fn strip_verbatim(p: &Path) -> PathBuf {
    let s = p.to_string_lossy();
    let s = if let Some(rest) = s.strip_prefix(r"\\?\") {
        rest
    } else {
        &s
    };
    PathBuf::from(s)
}

#[cfg(not(windows))]
fn strip_verbatim(p: &Path) -> PathBuf {
    p.to_path_buf()
}

/// 从 7765 开始探测空闲端口。
fn find_free_port() -> Result<u16, String> {
    for port in BASE_PORT..BASE_PORT + MAX_PORT_TRY {
        if TcpListener::bind(("0.0.0.0", port)).is_ok() {
            return Ok(port);
        }
    }
    Err("找不到可用的下载后台端口".into())
}

/// 等待后台 Web 服务就绪；期间若进程退出则报错。
fn wait_ready(port: u16, child: &mut Child) -> Result<(), String> {
    let url = format!("http://127.0.0.1:{port}/sources");
    let deadline = std::time::Instant::now() + Duration::from_secs(30);
    loop {
        if let Some(status) = child.try_wait().map_err(|e| e.to_string())? {
            return Err(format!(
                "下载后台启动失败（进程已退出，退出码 {}）",
                status.code().unwrap_or(-1)
            ));
        }
        if let Ok(resp) = ureq::get(&url).timeout(Duration::from_secs(2)).call() {
            if resp.status() == 200 {
                return Ok(());
            }
        }
        if std::time::Instant::now() > deadline {
            return Err("下载后台启动超时".into());
        }
        std::thread::sleep(Duration::from_millis(300));
    }
}

/// 初始化：注册状态并启动后台。
pub fn init(app: &AppHandle) -> Result<(), String> {
    app.manage(SonovelState::default());
    ensure_backend(app)
}

/// 启动（或重启）后台进程。
pub fn ensure_backend(app: &AppHandle) -> Result<(), String> {
    let data_dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
    let res_dir = sonovel_resource_dir(app);
    let sonovel_data = data_dir.join("sonovel");
    std::fs::create_dir_all(&sonovel_data).map_err(|e| e.to_string())?;

    // 清理上次异常残留的进程
    kill_pid_file(&sonovel_data);

    let bookshelf = load_bookshelf(&data_dir);
    std::fs::create_dir_all(&bookshelf).map_err(|e| e.to_string())?;

    ensure_rules(&res_dir, &sonovel_data)?;

    let port = find_free_port()?;
    write_config(&sonovel_data, &bookshelf, port)?;

    let java = strip_verbatim(
        &res_dir
            .join("runtime")
            .join("bin")
            .join(if cfg!(windows) { "java.exe" } else { "java" }),
    );
    let jar = strip_verbatim(&res_dir.join("app.jar"));
    let config_path = sonovel_data.join("config.ini");
    // 诊断：记录实际使用的资源路径（排查后台启动失败）
    {
        let diag = format!(
            "res_dir={}\njava={} java_exists={}\njar={} jar_exists={}\nconfig={} config_exists={}\n",
            res_dir.display(),
            java.display(),
            java.exists(),
            jar.display(),
            jar.exists(),
            config_path.display(),
            config_path.exists()
        );
        let _ = std::fs::write(sonovel_data.join("diag.txt"), diag);
    }
    if !java.exists() || !jar.exists() {
        return Err("下载后台组件缺失（Java 运行时或 app.jar）".into());
    }

    let config_arg = format!("-Dconfig.file={}", config_path.display());
    let mut cmd = Command::new(&java);
    cmd.args(["-Dmode=web", config_arg.as_str(), "-jar"])
        .arg(&jar)
        .current_dir(&sonovel_data)
        .stdin(Stdio::null());
    // 后台日志重定向到数据目录 backend.log（便于排查启动/运行问题）
    if let Ok(log_file) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(sonovel_data.join("backend.log"))
    {
        if let Ok(clone) = log_file.try_clone() {
            cmd.stdout(log_file).stderr(clone);
        }
    }
    #[cfg(windows)]
    cmd.creation_flags(CREATE_NO_WINDOW);

    let mut child = cmd
        .spawn()
        .map_err(|e| format!("启动下载后台失败：{e}"))?;
    write_pid(&sonovel_data, child.id());

    wait_ready(port, &mut child)?;

    let state = app.state::<SonovelState>();
    let mut st = state.inner.lock().unwrap();
    *st = Some(SonovelBackend {
        child: Some(child),
        port,
        fetching: Arc::new(AtomicBool::new(false)),
    });
    Ok(())
}

/// 停止后台进程（托盘"退出"与应用退出时调用）。
pub fn shutdown(app: &AppHandle) {
    let mut had_child = false;
    if let Some(state) = app.try_state::<SonovelState>() {
        let mut st = state.inner.lock().unwrap();
        if let Some(b) = st.take() {
            if let Some(mut child) = b.child {
                had_child = true;
                let _ = child.kill();
                let _ = child.wait();
            }
        }
    }
    if let Ok(data) = app.path().app_data_dir() {
        let sonovel_data = data.join("sonovel");
        // 更新或异常启动时状态可能尚未写入内存，仍按 pid 文件清理残留后台。
        if !had_child {
            kill_pid_file(&sonovel_data);
        }
        let _ = std::fs::remove_file(sonovel_data.join("backend.pid"));
    }
}

// ---------------------------------------------------------------------------
// HTTP 转发（本机访问）
// ---------------------------------------------------------------------------

fn urlencode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.as_bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(*b as char)
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

fn backend_port(state: &State<SonovelState>) -> Result<u16, String> {
    let mut st = state.inner.lock().unwrap();
    match st.as_mut() {
        Some(b) => {
            let running = b
                .child
                .as_mut()
                .map(|c| c.try_wait().ok().flatten().is_none())
                .unwrap_or(false);
            if running {
                Ok(b.port)
            } else {
                Err("下载后台已停止".into())
            }
        }
        _ => Err("下载后台未就绪".into()),
    }
}

fn get_json(url: &str, timeout: Duration) -> Result<serde_json::Value, String> {
    let agent = ureq::AgentBuilder::new()
        .timeout(timeout)
        .build();
    let resp = agent.get(url).call().map_err(|e| e.to_string())?;
    let text = resp.into_string().map_err(|e| e.to_string())?;
    let body: serde_json::Value = serde_json::from_str(&text).map_err(|e| e.to_string())?;
    if body["code"].as_i64() == Some(200) {
        Ok(body["data"].clone())
    } else {
        Err(body["message"].as_str().unwrap_or("请求失败").to_string())
    }
}

// ---------------------------------------------------------------------------
// 命令：后台生命周期
// ---------------------------------------------------------------------------

#[tauri::command]
pub fn backend_status(state: State<SonovelState>) -> BackendStatus {
    let mut st = state.inner.lock().unwrap();
    match st.as_mut() {
        Some(b) => BackendStatus {
            running: b
                .child
                .as_mut()
                .map(|c| c.try_wait().ok().flatten().is_none())
                .unwrap_or(false),
            port: b.port,
        },
        None => BackendStatus {
            running: false,
            port: 0,
        },
    }
}

#[tauri::command]
pub fn backend_start(app: AppHandle) -> Result<BackendStatus, String> {
    ensure_backend(&app)?;
    Ok(backend_status(app.state::<SonovelState>()))
}

#[tauri::command]
pub fn backend_stop(app: AppHandle) -> Result<(), String> {
    shutdown(&app);
    Ok(())
}

#[tauri::command]
pub fn get_bookshelf_dir(app: AppHandle) -> Result<String, String> {
    let data = app.path().app_data_dir().map_err(|e| e.to_string())?;
    Ok(load_bookshelf(&data).to_string_lossy().into_owned())
}

#[tauri::command]
pub fn set_bookshelf_dir(app: AppHandle, path: String) -> Result<(), String> {
    let data = app.path().app_data_dir().map_err(|e| e.to_string())?;
    save_bookshelf(&data, &path)?;
    // 重启后台以应用新的下载目录
    shutdown(&app);
    ensure_backend(&app)?;
    Ok(())
}

/// 目录选择对话框（书库目录设置用）。
#[tauri::command]
pub fn pick_directory(app: AppHandle, title: Option<String>) -> Result<Option<String>, String> {
    use tauri_plugin_dialog::DialogExt;
    let mut dlg = app.dialog().file();
    if let Some(t) = &title {
        dlg = dlg.set_title(t);
    }
    Ok(dlg
        .blocking_pick_folder()
        .and_then(|p| p.into_path().ok())
        .map(|p| p.to_string_lossy().into_owned()))
}

// ---------------------------------------------------------------------------
// 命令：搜索 / 下载
// ---------------------------------------------------------------------------

#[tauri::command]
pub fn sonovel_search(
    state: State<SonovelState>,
    kw: String,
    search_limit: Option<u32>,
) -> Result<serde_json::Value, String> {
    let port = backend_port(&state)?;
    let limit = search_limit.unwrap_or(30).min(30);
    let url = format!(
        "http://127.0.0.1:{port}/search/aggregated?kw={}&searchLimit={limit}",
        urlencode(&kw)
    );
    get_json(&url, Duration::from_secs(120))
}

#[tauri::command]
pub fn sonovel_sources(state: State<SonovelState>) -> Result<serde_json::Value, String> {
    let port = backend_port(&state)?;
    get_json(
        &format!("http://127.0.0.1:{port}/sources"),
        Duration::from_secs(15),
    )
}

#[tauri::command]
pub fn sonovel_local_books(state: State<SonovelState>) -> Result<serde_json::Value, String> {
    let port = backend_port(&state)?;
    get_json(
        &format!("http://127.0.0.1:{port}/local-books"),
        Duration::from_secs(15),
    )
}

fn extract_error(body: &str) -> String {
    if let Ok(v) = serde_json::from_str::<serde_json::Value>(body) {
        if let Some(m) = v["message"].as_str() {
            return m.to_string();
        }
    }
    "下载失败".to_string()
}

/// 同步下载一整本书（独立线程中执行，避免阻塞命令）。
fn fetch_blocking(port: u16, url: &str, source_id: i64, format: &str) -> Result<(), String> {
    let u = format!(
        "http://127.0.0.1:{port}/book-fetch?url={}&id={}&format={}",
        urlencode(url),
        source_id,
        urlencode(format)
    );
    let agent = ureq::AgentBuilder::new().build();
    match agent.get(&u).call() {
        Ok(resp) => {
            if resp.status() == 200 {
                Ok(())
            } else {
                let msg = resp.into_string().unwrap_or_default();
                Err(extract_error(&msg))
            }
        }
        Err(e) => Err(format!("下载失败：{e}")),
    }
}

/// 读取后台 SSE 下载进度，转发为前端事件 `sonovel-progress`。
fn watch_download_progress(app: AppHandle, port: u16) {
    std::thread::spawn(move || {
        let addr = format!("127.0.0.1:{port}");
        let mut stream = match TcpStream::connect(&addr) {
            Ok(s) => s,
            Err(_) => return,
        };
        let req = format!(
            "GET /download-progress HTTP/1.1\r\nHost: {addr}\r\nAccept: text/event-stream\r\nConnection: keep-alive\r\n\r\n"
        );
        if stream.write_all(req.as_bytes()).is_err() {
            return;
        }
        let reader = BufReader::new(stream);
        let mut headers_done = false;
        for line in reader.lines() {
            let line = match line {
                Ok(l) => l,
                Err(_) => break, // 连接断开（后台停止等），线程结束
            };
            if !headers_done {
                if line.is_empty() {
                    headers_done = true;
                }
                continue;
            }
            if let Some(data) = line.strip_prefix("data:") {
                let json: serde_json::Value =
                    serde_json::from_str(data.trim()).unwrap_or_default();
                if json["type"].as_str() == Some("download-progress") {
                    let total = json["total"].as_u64().unwrap_or(0);
                    let index = json["index"].as_u64().unwrap_or(0);
                    let percent = if total > 0 {
                        (index as f64 / total as f64 * 100.0).round() as u64
                    } else {
                        0
                    };
                    let _ = app.emit(
                        "sonovel-progress",
                        serde_json::json!({ "total": total, "index": index, "percent": percent }),
                    );
                }
            }
        }
    });
}

/// 发起下载任务（异步）：立即返回，完成/失败后发事件 `sonovel-fetch-done`。
#[tauri::command]
pub fn sonovel_fetch(
    app: AppHandle,
    state: State<SonovelState>,
    url: String,
    source_id: i64,
    format: String,
) -> Result<(), String> {
    let port = backend_port(&state)?;
    let fetching = {
        let mut st = state.inner.lock().unwrap();
        match st.as_mut() {
            Some(b) => {
                if b.fetching.swap(true, Ordering::SeqCst) {
                    return Err("已有下载任务正在进行".into());
                }
                b.fetching.clone()
            }
            None => return Err("下载后台未就绪".into()),
        }
    };
    watch_download_progress(app.clone(), port);
    let state_inner = state.inner.clone();
    std::thread::spawn(move || {
        let result = fetch_blocking(port, &url, source_id, &format);
        if let Ok(mut st) = state_inner.lock() {
            if let Some(b) = st.as_mut() {
                b.fetching.store(false, Ordering::SeqCst);
            }
        }
        let payload = match result {
            Ok(()) => serde_json::json!({ "ok": true, "message": "" }),
            Err(e) => serde_json::json!({ "ok": false, "message": e }),
        };
        let _ = app.emit("sonovel-fetch-done", payload);
        let _ = fetching; // 与后端状态共享
    });
    Ok(())
}
