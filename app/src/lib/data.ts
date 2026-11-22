import type { Recap } from '../types'
import dayJson from '../demo/day.json'
import yearJson from '../demo/year.json'

/**
 * 数据入口：在桌面端（Tauri）里调用 Rust 命令读本地数据库；
 * 在浏览器里（开发 / 效果图预览）用演示数据，方便在没有 Windows 的机器上看效果。
 */
export const isTauri = () => typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window

export interface SessionItem { id: number; start: string; end: string; minutes: number }
export interface AppStatus { errors_only_logs: number; latest_errors_only: boolean; log_dir?: string; log_dir_found?: boolean }
export type PeriodKind = 'day' | 'week' | 'month' | 'year'

async function invoke<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  const { invoke } = await import('@tauri-apps/api/core')
  return invoke<T>(cmd, args)
}

/** 本地文件路径 → WebView 能加载的地址（封面、截图） */
async function withUrls(r: Recap | null): Promise<Recap | null> {
  if (!r || !isTauri()) return r
  const { convertFileSrc } = await import('@tauri-apps/api/core')
  const u = (p: string | null) => (p ? convertFileSrc(p) : null)
  return {
    ...r,
    top_worlds: r.top_worlds.map((w) => ({ ...w, thumbnail: u(w.thumbnail) })),
    top_photo_world: r.top_photo_world ? { ...r.top_photo_world, thumbnail: u(r.top_photo_world.thumbnail) } : null,
    timeline: r.timeline?.map((s) => ({ ...s, thumbnail: u(s.thumbnail) })),
    photos: r.photos.map((p) => ({ ...p, url: u(p.path) })),
  }
}

const DEMO_DAY = dayJson as Recap
const DEMO_YEAR = yearJson as Recap

export async function listSessions(): Promise<SessionItem[]> {
  if (isTauri()) return invoke<SessionItem[]>('sessions', { limit: 100 })
  return [{ id: 0, start: DEMO_DAY.from, end: DEMO_DAY.last_logoff ?? DEMO_DAY.to, minutes: DEMO_DAY.total_minutes }]
}

export async function sessionRecap(id: number | null): Promise<Recap | null> {
  if (isTauri()) return withUrls(await invoke<Recap | null>('session_recap', { id }))
  return { ...DEMO_DAY, kind: 'session' }
}

export async function periodRecap(kind: PeriodKind, date: string): Promise<Recap | null> {
  if (isTauri()) return withUrls(await invoke<Recap>('period_recap', { kind, date }))
  return kind === 'year' ? DEMO_YEAR : { ...DEMO_DAY, kind }
}

export async function appStatus(): Promise<AppStatus | null> {
  if (isTauri()) return invoke<AppStatus>('status')
  return fetch('/real/status.json').then((r) => (r.ok ? r.json() : null)).catch(() => null)
}

/** 桌面端事件：下线后有新回顾 / 点通知要打开某次回顾 */
export async function onAppEvent(name: 'session-ended' | 'open-session', cb: (id: number | null) => void) {
  if (!isTauri()) return () => {}
  const { listen } = await import('@tauri-apps/api/event')
  return listen<number | null>(name, (e) => cb(e.payload))
}

export async function autostart() {
  if (!isTauri()) return null
  return import('@tauri-apps/plugin-autostart')
}

export const today = () => {
  const d = new Date()
  return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, '0')}-${String(d.getDate()).padStart(2, '0')}`
}

/** 日期加减：天 / 周 / 月 / 年 */
export function shiftDate(date: string, kind: PeriodKind, n: number) {
  const d = new Date(date + 'T00:00:00')
  if (kind === 'day') d.setDate(d.getDate() + n)
  if (kind === 'week') d.setDate(d.getDate() + 7 * n)
  if (kind === 'month') d.setMonth(d.getMonth() + n, 1)
  if (kind === 'year') d.setFullYear(d.getFullYear() + n, 0, 1)
  return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, '0')}-${String(d.getDate()).padStart(2, '0')}`
}
