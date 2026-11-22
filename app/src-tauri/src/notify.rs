//! Windows 原生通知。点击通知 → 打开窗口并跳到这次的回顾。

use crate::watcher::Outcome;
use tauri::{AppHandle, Emitter, Manager};

pub fn show_window(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        w.show().ok();
        w.unminimize().ok();
        w.set_focus().ok();
    }
}

pub fn push(app: &AppHandle, o: Outcome) {
    let (title, body, session) = match o {
        Outcome::Session { id, title, body } => (title, body, Some(id)),
        Outcome::ErrorsOnly => (
            "这次游玩没有被记录".to_string(),
            "VRChat 日志设置是「仅错误」。请在 设置 → Debug → 日志 改成「完整」".to_string(),
            None,
        ),
    };
    // 让已打开的窗口也刷新
    app.emit("session-ended", session).ok();

    #[cfg(windows)]
    {
        use tauri_winrt_notification::{Duration, Toast};
        let id = app.config().identifier.clone();
        let handle = app.clone();
        let res = Toast::new(&id)
            .title(&title)
            .text1(&body)
            .duration(Duration::Long)
            .on_activated(move |_| {
                show_window(&handle);
                handle.emit("open-session", session).ok();
                Ok(())
            })
            .show();
        if let Err(e) = res {
            crate::log::log(format!("通知失败: {e}"));
        }
    }
    #[cfg(not(windows))]
    {
        let _ = (title, body);
    }
}
