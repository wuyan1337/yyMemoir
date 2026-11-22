//! 日志行解析：所有 VRChat 日志格式相关的规则都集中在这个文件里。
//! 日志格式变了只改这里。规则对照 VRCTimeline (MIT) 的 LogPatterns.cs 与 vrchat-log-rs (MIT)。
//! TODO: 拿到真实日志后逐条校对（见 进度.md）。

use chrono::NaiveDateTime;
use regex::Regex;
use std::sync::OnceLock;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    /// 登录用户本人
    SelfUser { name: String, user_id: Option<String> },
    /// `Initialized PlayerAPI "名字" is local`：每次进房间都会写，本人是 local，其他人是 remote
    LocalPlayer { name: String },
    /// `Joining wrld_xxx:12345~friends(usr_...)~region(jp)`
    Joining { world_id: String, instance_id: String, access_type: String },
    /// `Entering Room: 世界名`
    EnteringRoom { world_name: String },
    PlayerJoined { name: String, user_id: Option<String> },
    PlayerLeft { name: String, user_id: Option<String> },
    /// `OnLeftRoom`：离开当前房间
    LeftRoom,
    Video { url: String },
    /// `[VRC Camera] Took screenshot to: 路径`：VRChat 相机拍的照片（多层相机模式不会写这行）
    Photo { path: String },
}

macro_rules! re {
    ($name:ident, $pat:expr) => {
        fn $name() -> &'static Regex {
            static R: OnceLock<Regex> = OnceLock::new();
            R.get_or_init(|| Regex::new($pat).unwrap())
        }
    };
}

re!(ts_re, r"^(\d{4}\.\d{2}\.\d{2} \d{2}:\d{2}:\d{2})\s");
re!(joining_re, r"\[Behaviour\] Joining (wrld_[0-9a-fA-F\-]+):(\S+)\s*$");
re!(entering_re, r"\[Behaviour\] Entering Room: (.+?)\s*$");
re!(auth_re, r"User Authenticated: (.+?)\s*$");
re!(post_login_re, r"post login checks for (.+?)\s*$");
re!(joined_re, r"\[Behaviour\] OnPlayerJoined (.+?)\s*$");
re!(left_re, r"\[Behaviour\] OnPlayerLeft (.+?)\s*$");
re!(left_room_re, r"\[Behaviour\] OnLeftRoom\b");
re!(video_re, r"\[Video Playback\].*?Attempting to resolve URL '(.+?)'");
re!(local_re, r#"\[Behaviour\] Initialized PlayerAPI "(.+)" is local\s*$"#);
re!(photo_re, r"\[VRC Camera\] Took screenshot to: (.+?)\s*$");
re!(user_suffix_re, r"^(.*?)\s+\((usr_[0-9a-fA-F\-]+)\)$");

/// 解析一行。没有时间戳的行（堆栈、多行续行）返回 None。
/// 这一行是不是 Error 级别。VRChat 设置 → Debug → 日志 选了「仅错误（Errors Only）」时，
/// 文件里只会有 Error 行，没有任何世界/玩家记录。
pub fn is_error_line(line: &str) -> bool {
    line.get(20..).is_some_and(|rest| rest.starts_with("Error") || rest.starts_with("Exception"))
}

pub fn parse_line(line: &str) -> Option<(NaiveDateTime, Option<Event>)> {
    let line = line.trim_end_matches(['\r', '\n']);
    let caps = ts_re().captures(line)?;
    let ts = NaiveDateTime::parse_from_str(&caps[1], "%Y.%m.%d %H:%M:%S").ok()?;
    Some((ts, parse_event(line)))
}

fn split_user(raw: &str) -> (String, Option<String>) {
    match user_suffix_re().captures(raw) {
        Some(c) => (c[1].to_string(), Some(c[2].to_string())),
        None => (raw.to_string(), None),
    }
}

fn parse_event(line: &str) -> Option<Event> {
    // 先用便宜的 contains 过滤，绝大多数行一个正则都不用跑
    if line.contains("[Behaviour]") {
        if let Some(c) = joined_re().captures(line) {
            let (name, user_id) = split_user(&c[1]);
            return Some(Event::PlayerJoined { name, user_id });
        }
        if let Some(c) = left_re().captures(line) {
            let (name, user_id) = split_user(&c[1]);
            return Some(Event::PlayerLeft { name, user_id });
        }
        if let Some(c) = joining_re().captures(line) {
            let instance_id = c[2].to_string();
            return Some(Event::Joining {
                world_id: c[1].to_string(),
                access_type: access_type(&instance_id).to_string(),
                instance_id,
            });
        }
        if let Some(c) = entering_re().captures(line) {
            return Some(Event::EnteringRoom { world_name: c[1].to_string() });
        }
        if line.contains(" is local") {
            if let Some(c) = local_re().captures(line) {
                return Some(Event::LocalPlayer { name: c[1].to_string() });
            }
        }
        if left_room_re().is_match(line) {
            return Some(Event::LeftRoom);
        }
        return None;
    }
    if let Some(c) = post_login_re().captures(line) {
        let (name, user_id) = split_user(&c[1]);
        return Some(Event::SelfUser { name, user_id });
    }
    if let Some(c) = auth_re().captures(line) {
        let (name, user_id) = split_user(&c[1]);
        return Some(Event::SelfUser { name, user_id });
    }
    if line.contains("[VRC Camera]") {
        if let Some(c) = photo_re().captures(line) {
            return Some(Event::Photo { path: c[1].to_string() });
        }
    }
    if line.contains("[Video Playback]") {
        if let Some(c) = video_re().captures(line) {
            return Some(Event::Video { url: c[1].to_string() });
        }
    }
    None
}

/// 从房间号推断房间类型。
/// `12345` → public；`~hidden(..)` → friends+；`~friends(..)` → friends；
/// `~private(..)~canRequestInvite` → invite+；`~private(..)` → invite；`~group(..)` → group
pub fn access_type(instance: &str) -> &'static str {
    let tags: Vec<&str> = instance.split('~').skip(1).map(|t| t.split('(').next().unwrap_or("")).collect();
    let has = |t: &str| tags.iter().any(|x| *x == t);
    if has("group") {
        "group"
    } else if has("hidden") {
        "friends+"
    } else if has("friends") {
        "friends"
    } else if has("private") {
        if has("canRequestInvite") { "invite+" } else { "invite" }
    } else {
        "public"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_core_lines() {
        let (_, e) = parse_line("2026.04.30 21:00:00 Log        -  [Behaviour] Joining wrld_f8ff20cd-5310-4257-ade8-c3fd6ae95436:98257~friends(usr_f8229b4f-794c-4a94-bf5d-d21f3fc0daf5)~region(jp)").unwrap();
        assert_eq!(e, Some(Event::Joining {
            world_id: "wrld_f8ff20cd-5310-4257-ade8-c3fd6ae95436".into(),
            instance_id: "98257~friends(usr_f8229b4f-794c-4a94-bf5d-d21f3fc0daf5)~region(jp)".into(),
            access_type: "friends".into(),
        }));
        let (_, e) = parse_line("2026.04.30 21:00:05 Log        -  [Behaviour] OnPlayerJoined 小明 (Ming) (usr_0a1b2c3d-0000-0000-0000-000000000001)").unwrap();
        assert_eq!(e, Some(Event::PlayerJoined { name: "小明 (Ming)".into(), user_id: Some("usr_0a1b2c3d-0000-0000-0000-000000000001".into()) }));
        let (_, e) = parse_line("2026.04.30 21:00:01 Log        -  [Behaviour] Entering Room: The Black Cat").unwrap();
        assert_eq!(e, Some(Event::EnteringRoom { world_name: "The Black Cat".into() }));
        assert!(parse_line("   at UnityEngine.Debug.Log").is_none());
    }

    #[test]
    fn access_types() {
        assert_eq!(access_type("123"), "public");
        assert_eq!(access_type("123~region(jp)"), "public");
        assert_eq!(access_type("1~hidden(usr_x)~region(us)"), "friends+");
        assert_eq!(access_type("1~private(usr_x)~canRequestInvite~region(us)"), "invite+");
        assert_eq!(access_type("1~private(usr_x)"), "invite");
        assert_eq!(access_type("1~group(grp_x)~groupAccessType(public)"), "group");
    }
}

#[cfg(test)]
mod level_tests {
    #[test]
    fn error_level() {
        assert!(super::is_error_line("2026.11.22 02:06:54 Error      -  StyleEngine: Missing resource"));
        assert!(!super::is_error_line("2026.11.21 01:34:07 Debug      -  Launching with args: 2"));
        assert!(!super::is_error_line("2026.11.21 01:34:07 Warning    -  OvrLipSync Awake"));
    }
}
