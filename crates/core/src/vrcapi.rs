//! VRChat 接口（可选联网）。目前只用不需要登录的「按世界 ID 查世界」：拿名字、作者和封面图。
//! 规则：只在生成报告时按需查；查过的存进 worlds_cache，图片下载到本地，同一个只查一次；请求之间间隔 1 秒。
//! 头像 / 个人背景需要登录（/users 接口未登录返回 401），放到登录功能里做。

use crate::store::Store;
use rusqlite::params;
use std::io::Read;
use std::path::Path;
use std::time::Duration;

const API: &str = "https://api.vrchat.cloud/api/1";
// TODO: 正式发布前按 VRChat 要求加上项目联系方式
pub const USER_AGENT: &str = "yyMemoir/0.1";

#[derive(Debug, Default)]
pub struct FetchReport {
    pub fetched: usize,
    pub failed: Vec<String>,
}

/// 给数据库里出现过、但还没有缓存封面的世界拉取信息和封面图，图片存到 `cache_dir`。
pub fn fetch_missing_worlds(store: &Store, cache_dir: &Path, limit: usize) -> rusqlite::Result<FetchReport> {
    std::fs::create_dir_all(cache_dir).ok();
    let ids: Vec<String> = {
        let mut st = store.conn.prepare(
            "SELECT v.world_id FROM visits v LEFT JOIN worlds_cache w ON w.world_id = v.world_id
             WHERE w.thumbnail_path IS NULL AND (w.fetched_at IS NULL OR w.fetched_at < ?2)
             GROUP BY v.world_id ORDER BY MAX(v.join_at) DESC LIMIT ?1",
        )?;
        // 失败过的（删掉 / 私有的世界）一天内不再重试
        let retry_before = chrono::Utc::now().timestamp_millis() - 24 * 3600 * 1000;
        let rows = st.query_map(rusqlite::params![limit as i64, retry_before], |r| r.get(0))?;
        rows.collect::<rusqlite::Result<_>>()?
    };
    let agent = agent()?;
    let mut rep = FetchReport::default();
    for (i, id) in ids.iter().enumerate() {
        if i > 0 {
            std::thread::sleep(Duration::from_secs(1));
        }
        match fetch_one(&agent, id, cache_dir) {
            Ok((name, author, path)) => {
                store.conn.execute(
                    "INSERT INTO worlds_cache(world_id, name, author, thumbnail_path, fetched_at) VALUES (?1,?2,?3,?4,?5)
                     ON CONFLICT(world_id) DO UPDATE SET name=excluded.name, author=excluded.author,
                       thumbnail_path=excluded.thumbnail_path, fetched_at=excluded.fetched_at",
                    params![id, name, author, path, chrono::Utc::now().timestamp_millis()],
                )?;
                rep.fetched += 1;
            }
            Err(e) => {
                store.conn.execute(
                    "INSERT INTO worlds_cache(world_id, fetched_at) VALUES (?1, ?2)
                     ON CONFLICT(world_id) DO UPDATE SET fetched_at = excluded.fetched_at",
                    params![id, chrono::Utc::now().timestamp_millis()],
                )?;
                rep.failed.push(format!("{id}: {e}"));
            }
        }
    }
    Ok(rep)
}

fn fetch_one(agent: &ureq::Agent, id: &str, cache_dir: &Path) -> Result<(String, Option<String>, String), String> {
    let v: serde_json::Value = agent
        .get(&format!("{API}/worlds/{id}"))
        .call()
        .map_err(|e| e.to_string())?
        .into_json()
        .map_err(|e| e.to_string())?;
    let name = v["name"].as_str().unwrap_or(id).to_string();
    let author = v["authorName"].as_str().map(str::to_string);
    let url = v["imageUrl"].as_str().or(v["thumbnailImageUrl"].as_str()).ok_or("没有封面图")?;
    let resp = agent.get(url).call().map_err(|e| e.to_string())?;
    let ext = match resp.content_type() {
        "image/jpeg" => "jpg",
        "image/webp" => "webp",
        _ => "png",
    };
    let mut bytes = Vec::new();
    resp.into_reader().take(20 << 20).read_to_end(&mut bytes).map_err(|e| e.to_string())?;
    let path = cache_dir.join(format!("{id}.{ext}"));
    std::fs::write(&path, bytes).map_err(|e| e.to_string())?;
    Ok((name, author, path.to_string_lossy().to_string()))
}

fn builder() -> ureq::AgentBuilder {
    ureq::AgentBuilder::new().timeout(Duration::from_secs(20)).user_agent(USER_AGENT)
}

/// Windows：系统 TLS（schannel）
#[cfg(windows)]
fn agent() -> rusqlite::Result<ureq::Agent> {
    let tls = native_tls::TlsConnector::new().map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
    Ok(builder().tls_connector(std::sync::Arc::new(tls)).build())
}

#[cfg(not(windows))]
fn agent() -> rusqlite::Result<ureq::Agent> {
    Ok(builder().build())
}
