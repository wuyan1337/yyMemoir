// 与 crates/core/src/stats.rs 的 Recap 一一对应
export interface SessionStat { start: string; end: string; minutes: number }
export interface DayStat { date: string; minutes: number }
export interface WorldStat { world_id: string; name: string; minutes: number; visits: number; photos: number; thumbnail: string | null }
export interface PersonStat { user_id: string; name: string; minutes: number; is_friend: boolean; avatar: string | null; best_day: DayStat | null }
export interface Stop { world_id: string; name: string; access_type: string; start: string; end: string; minutes: number; thumbnail: string | null }
export interface PhotoStat { path: string; taken_at: string; world_name: string | null; /** 前端可加载的地址（Tauri 里由 convertFileSrc 生成） */ url?: string | null }
export interface AvatarStat { name: string; avatar_id: string | null; minutes: number; wears: number }
export interface Recap {
  product: string; kind: string; from: string; to: string; self_name: string | null;
  total_minutes: number; play_days: number; session_count: number;
  longest_session: SessionStat | null; latest_logoff: SessionStat | null; last_logoff?: string | null; peak_hour: number | null;
  hour_minutes: number[]; daily_minutes: DayStat[]; monthly_minutes: number[]; busiest_day: DayStat | null;
  world_count: number; new_world_count: number; top_worlds: WorldStat[];
  people_count: number; top_people: PersonStat[];
  access_minutes: Record<string, number>;
  photo_count: number; top_photo_world: WorldStat | null; photos: PhotoStat[];
  timeline?: Stop[];
  /** 本人穿过的模型（旧数据 / 演示数据可能没有） */
  avatar_count?: number; top_avatars?: AvatarStat[];
}
