use std::path::PathBuf;

/// 所有用到的路径
#[derive(Clone, Debug)]
pub struct Paths {
    /// %USERPROFILE%\AppData\LocalLow\VRChat\VRChat
    pub vrchat_logs: PathBuf,
    /// 本程序数据：数据库、世界封面缓存
    pub db: PathBuf,
    pub world_cache: PathBuf,
    /// 从备份还原出来的照片（原位置不存在时）
    pub restored_photos: PathBuf,
}

impl Paths {
    pub fn detect(app_data: PathBuf) -> Self {
        let home = dirs::home_dir().unwrap_or_default();
        let vrchat_logs = home.join("AppData").join("LocalLow").join("VRChat").join("VRChat");
        std::fs::create_dir_all(&app_data).ok();
        Paths {
            vrchat_logs,
            db: app_data.join("yymemoir.db"),
            world_cache: app_data.join("world-cache"),
            restored_photos: app_data.join("photos"),
        }
    }
}
