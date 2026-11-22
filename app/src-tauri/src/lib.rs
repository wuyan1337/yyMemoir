//! yyMemoir 桌面端：开机静默启动，只有托盘图标；后台监听 VRChat，下线推送「本次回顾」。
//! 窗口只在点通知 / 托盘 / 再次打开程序时出现，关窗口只是隐藏。

mod log;
mod notify;
mod paths;
mod watcher;

use paths::Paths;
use serde::Serialize;
use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{Emitter, LogicalSize, Manager, State, WindowEvent};
use tauri_plugin_autostart::{MacosLauncher, ManagerExt};
use yymemoir_core::{list_sessions, recap_period, recap_session, Recap, Store, TimeConv};

const TZ: TimeConv = TimeConv::Local;
/// 开机自启时带这个参数：不弹窗口，只在托盘
const BACKGROUND_ARG: &str = "--background";

struct AppState {
    paths: Paths,
}

fn open_store(st: &State<AppState>) -> Result<Store, String> {
    Store::open(&st.paths.db).map_err(|e| e.to_string())
}

/// 日 / 周 / 月 / 年报告；date 是 YYYY-MM-DD
#[tauri::command]
fn period_recap(st: State<AppState>, kind: String, date: String) -> Result<Recap, String> {
    let d = chrono::NaiveDate::parse_from_str(&date, "%Y-%m-%d").map_err(|e| e.to_string())?;
    recap_period(&open_store(&st)?, TZ, &kind, d, 5).map_err(|e| e.to_string())
}

/// 某次游玩的「本次回顾」；id 为空取最近一次
#[tauri::command]
fn session_recap(st: State<AppState>, id: Option<i64>) -> Result<Option<Recap>, String> {
    let store = open_store(&st)?;
    let id = match id {
        Some(i) => i,
        None => match list_sessions(&store, 1).map_err(|e| e.to_string())?.first() {
            Some(s) => s.0,
            None => return Ok(None),
        },
    };
    recap_session(&store, TZ, id, 5).map_err(|e| e.to_string())
}

#[derive(Serialize)]
struct DayItem {
    date: String,
    minutes: i64,
}

/// 最近 `days` 天里有游玩的日子（新的在前），日报 / 周报 / 月报的列表用
#[tauri::command]
fn play_days(st: State<AppState>, days: Option<i64>) -> Result<Vec<DayItem>, String> {
    let store = open_store(&st)?;
    let today = chrono::Local::now().date_naive();
    let from = today - chrono::Duration::days(days.unwrap_or(400));
    let ms = |d: chrono::NaiveDate| TZ.to_utc_ms(d.and_hms_opt(0, 0, 0).unwrap());
    let r = yymemoir_core::recap(&store, TZ, "range", ms(from), ms(today + chrono::Duration::days(1)), 1).map_err(|e| e.to_string())?;
    let mut v: Vec<DayItem> =
        r.daily_minutes.into_iter().filter(|d| d.minutes > 0).map(|d| DayItem { date: d.date, minutes: d.minutes }).collect();
    v.reverse();
    Ok(v)
}

#[derive(Serialize)]
struct SessionItem {
    id: i64,
    start: String,
    end: String,
    minutes: i64,
}

#[tauri::command]
fn sessions(st: State<AppState>, limit: Option<usize>) -> Result<Vec<SessionItem>, String> {
    let store = open_store(&st)?;
    let f = |ms: i64| TZ.to_local(ms).format("%Y-%m-%d %H:%M").to_string();
    Ok(list_sessions(&store, limit.unwrap_or(50))
        .map_err(|e| e.to_string())?
        .into_iter()
        .map(|(id, a, b)| SessionItem { id, start: f(a), end: f(b), minutes: (b - a) / 60_000 })
        .collect())
}

/// 读本地图片（世界封面 / 截图），缩到最长边 `max` 像素，返回 JPEG data URL。
/// 不走 asset 协议：同源的 data URL 才能被 canvas 取色、被「保存图片」完整导出。
/// 只允许读封面缓存和截图文件夹里的文件。
#[tauri::command]
fn image_data(st: State<AppState>, path: String, max: Option<u32>) -> Result<String, String> {
    use base64::Engine;
    let p = std::path::Path::new(&path).canonicalize().map_err(|e| e.to_string())?;
    let allowed = [Some(st.paths.world_cache.clone()), st.paths.screenshots.clone()]
        .into_iter()
        .flatten()
        .filter_map(|d| d.canonicalize().ok())
        .any(|d| p.starts_with(d));
    if !allowed {
        return Err("不允许读取这个位置的文件".into());
    }
    let img = image::open(&p).map_err(|e| e.to_string())?;
    let m = max.unwrap_or(1200);
    let img = if img.width() > m || img.height() > m { img.thumbnail(m, m) } else { img };
    let mut buf = std::io::Cursor::new(Vec::new());
    img.to_rgb8().write_to(&mut buf, image::ImageFormat::Jpeg).map_err(|e| e.to_string())?;
    Ok(format!("data:image/jpeg;base64,{}", base64::engine::general_purpose::STANDARD.encode(buf.into_inner())))
}

#[derive(Serialize)]
struct Status {
    errors_only_logs: usize,
    latest_errors_only: bool,
    log_dir: String,
    log_dir_found: bool,
}

#[tauri::command]
fn status(st: State<AppState>) -> Result<Status, String> {
    let store = open_store(&st)?;
    let quiet = yymemoir_core::store::errors_only_logs(&store, 10).map_err(|e| e.to_string())?;
    let latest: Option<String> =
        store.conn.query_row("SELECT path FROM log_files ORDER BY path DESC LIMIT 1", [], |r| r.get(0)).ok();
    Ok(Status {
        errors_only_logs: quiet.len(),
        latest_errors_only: latest.is_some_and(|l| quiet.contains(&l)),
        log_dir: st.paths.vrchat_logs.to_string_lossy().to_string(),
        log_dir_found: st.paths.vrchat_logs.is_dir(),
    })
}

/// 按屏幕大小定窗口：高度占屏幕 86%，宽度刚好放下侧栏 + 一张竖版卡片（3:4）
fn fit_window(app: &tauri::AppHandle) {
    let Some(w) = app.get_webview_window("main") else { return };
    let Ok(Some(m)) = w.current_monitor() else { return };
    let screen = m.size().to_logical::<f64>(m.scale_factor());
    let h = (screen.height * 0.86).clamp(600.0, 1100.0);
    // 三栏：导航（约 150）+ 列表（约 260）+ 卡片（高 ≈ 窗口高 - 工具栏和边距约 110，宽 = 高 × 3/4）+ 边距
    let wd = ((h - 110.0) * 0.75 + 470.0).clamp(820.0, screen.width * 0.95);
    w.set_size(LogicalSize::new(wd, h)).ok();
    w.center().ok();
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| notify::show_window(app)))
        .plugin(tauri_plugin_autostart::init(MacosLauncher::LaunchAgent, Some(vec![BACKGROUND_ARG])))
        .invoke_handler(tauri::generate_handler![period_recap, session_recap, sessions, status, image_data, play_days])
        .setup(|app| {
            let data_dir = app.path().app_data_dir()?;
            let paths = Paths::detect(data_dir.clone());
            log::init(data_dir.join("yymemoir.log"));
            fit_window(app.handle());

            // 第一次运行默认开启开机自启（设置里可关）
            let marker = data_dir.join(".autostart-initialized");
            if !marker.exists() {
                app.autolaunch().enable().ok();
                std::fs::write(&marker, b"1").ok();
            }

            // 托盘
            let open = MenuItem::with_id(app, "open", "打开 yyMemoir", true, None::<&str>)?;
            let quit = MenuItem::with_id(app, "quit", "退出", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&open, &quit])?;
            TrayIconBuilder::with_id("main")
                .icon(app.default_window_icon().cloned().expect("缺少图标"))
                .tooltip("yyMemoir · 正在记录 VRChat")
                .menu(&menu)
                .show_menu_on_left_click(false)
                .on_menu_event(|app, e| match e.id.as_ref() {
                    "open" => notify::show_window(app),
                    "quit" => app.exit(0),
                    _ => {}
                })
                .on_tray_icon_event(|tray, e| {
                    if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } = e {
                        notify::show_window(tray.app_handle());
                    }
                })
                .build(app)?;

            // 后台监听线程
            let handle = app.handle().clone();
            let handle2 = app.handle().clone();
            let watcher_paths = paths.clone();
            std::thread::spawn(move || {
                watcher::run(
                    watcher_paths,
                    move |o| notify::push(&handle, o),
                    move || {
                        handle2.emit("data-updated", ()).ok();
                    },
                )
            });

            app.manage(AppState { paths });

            if !std::env::args().any(|a| a == BACKGROUND_ARG) {
                notify::show_window(app.handle());
            }
            Ok(())
        })
        .on_window_event(|window, event| {
            // 关窗口只是隐藏，程序继续在托盘记录
            if let WindowEvent::CloseRequested { api, .. } = event {
                window.hide().ok();
                api.prevent_close();
            }
        })
        .run(tauri::generate_context!())
        .expect("启动 yyMemoir 失败");
}
