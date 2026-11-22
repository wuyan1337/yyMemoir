import { shown, type Privacy } from '../lib/format'
import type { Recap } from '../types'
import { DayRibbon, Line, WEEK, clock, hm, itinerary, periodWord, useCardTone } from './DailyCard'
import { Brand, Cover } from './parts'

/**
 * 方向 A：便当格（Bento）1080×1440。
 * 每类信息一个格子，信息多但各管一件事：大封面 / 总时长 / 时间轴 / 行程 / 陪伴的人 / 小数据。
 */
export function BentoCard({ r, priv }: { r: Recap; priv: Privacy }) {
  const style = useCardTone(r)
  const session = r.kind === 'session'
  // 周报 / 月报：时间轴换成每天的柱子，行程换成最常去的世界
  const multiDay = r.kind === 'week' || r.kind === 'month'
  const lastDay = (() => {
    const d = new Date(new Date(r.to.slice(0, 10) + 'T00:00:00').getTime() - 86400000)
    return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}`
  })()
  const top = r.top_worlds[0]
  const day = r.from.slice(0, 10)
  const date = new Date(day + 'T00:00:00')
  const photos = r.photos.slice(0, 4)
  // 放不下时保留最久的 5 站，按时间排
  const all = itinerary(r.timeline ?? [])
  const keep = new Set([...all].sort((a, b) => b.minutes - a.minutes).slice(0, photos.length ? 3 : 5))
  const stops = multiDay
    ? r.top_worlds.slice(0, photos.length ? 3 : 5).map((w) => ({ world_id: w.world_id, name: w.name, thumbnail: w.thumbnail, minutes: w.minutes, start: `来过 ${w.visits} 次`, end: '', access_type: '' }))
    : all.filter((s) => keep.has(s))
  const people = r.top_people.slice(0, photos.length ? 3 : 4)
  const maxP = Math.max(1, ...people.map((p) => p.minutes))
  const lastEnd = r.last_logoff ?? r.latest_logoff?.end
  // 占比最多的房间类型（好友 + 好友+ 合并算「好友房」）
  const kinds: Record<string, number> = {}
  for (const [k, v] of Object.entries(r.access_minutes)) {
    const label = k.startsWith('friends') ? '好友房' : k === 'public' ? '公开房' : k === 'group' ? '群组房' : '邀请房'
    kinds[label] = (kinds[label] ?? 0) + v
  }
  const allMin = Object.values(kinds).reduce((a, b) => a + b, 0) || 1
  const [mainKind, mainMin] = Object.entries(kinds).sort((a, b) => b[1] - a[1])[0] ?? ['好友房', 0]

  return (
    <div className="bt" style={style}>
      <div className="dc-grain" />
      <header className="bt-head">
        <div className="bt-date">
          <b>{multiDay ? (r.kind === 'month' ? `${date.getFullYear()}.${pad(date.getMonth() + 1)}` : md(day)) : md(day)}</b>
          <span className="mono">{r.kind === 'week' ? `– ${md(lastDay)}` : r.kind === 'month' ? '' : WEEK[date.getDay()]}</span>
        </div>
        <span className="bt-kind mono">
          {session ? `本次回顾 · ${clock(r.from)}–${clock(lastEnd ?? r.to)}` : r.kind === 'week' ? '周报' : r.kind === 'month' ? '月报' : '今日回顾 · 全天'}
        </span>
      </header>

      <div className={`bt-grid ${photos.length ? 'with-photos' : ''}`}>
        {top && (
          <Cover id={top.world_id} url={top.thumbnail} className="bt-tile bt-hero">
            <div className="bt-hero-shade" />
            <div className="bt-hero-cap">
              <span className="bt-label">待得最久</span>
              <b>{top.name}</b>
              <span className="mono">{hm(top.minutes)}{top.visits > 1 ? ` · 来回 ${top.visits} 次` : ''}</span>
            </div>
          </Cover>
        )}

        <div className="bt-tile bt-total">
          <span className="bt-label">{periodWord(r.kind)}玩了</span>
          <b>{hm(r.total_minutes)}</b>
          <span className="bt-sub">{multiDay ? `${r.play_days} 天上线` : lastEnd ? `${clock(lastEnd)} 下线` : ''}</span>
        </div>

        <div className="bt-tile bt-nums">
          <div><b>{r.world_count}</b><span>个世界</span></div>
          <div><b>{r.people_count}</b><span>个人</span></div>
          <div><b>{r.photo_count}</b><span>张照片</span></div>
          <div><b>{Math.round((mainMin / allMin) * 100)}%</b><span>在{mainKind}</span></div>
        </div>

        <div className="bt-tile bt-ribbon">
          <span className="bt-label">{multiDay ? '每天玩了多久' : '什么时候在线'}</span>
          {multiDay ? <DailyBars r={r} /> : <DayRibbon stops={r.timeline ?? []} day={day} longest={top?.world_id} />}
        </div>

        <div className="bt-tile bt-stops">
          <span className="bt-label">{multiDay ? '最常去' : '行程'}</span>
          <ol>
            {stops.map((s, i) => (
              <li key={i}>
                <Cover id={s.world_id} url={s.thumbnail} className="bt-thumb" />
                <span className="bt-sname">{s.name}</span>
                <span className="bt-stime mono">{multiDay ? s.start : clock(s.start)}</span>
                <span className="bt-sdur mono">{hm(s.minutes)}</span>
              </li>
            ))}
          </ol>
        </div>

        <div className="bt-tile bt-people">
          <span className="bt-label">陪你最久</span>
          <ul>
            {people.map((p) => (
              <li key={p.user_id}>
                <div className="bt-prow"><b>{shown(p, priv).name}</b><span className="mono">{hm(p.minutes)}</span></div>
                <div className="bt-pbar"><i style={{ width: `${(p.minutes / maxP) * 100}%` }} /></div>
              </li>
            ))}
            {people.length === 0 && <li className="bt-sub">一个人的旅行</li>}
          </ul>
        </div>
        {photos.length > 0 && (
          <div className="bt-tile bt-photos">
            <span className="bt-label">拍下的瞬间 · {r.photo_count} 张</span>
            <div className="bt-photo-row">
              {photos.map((p) => (
                <div key={p.path} className="bt-photo">
                  <Cover id={p.path} url={p.url} className="bt-photo-img" />
                  <span className="mono">{clock(p.taken_at)}</span>
                </div>
              ))}
            </div>
          </div>
        )}
        <div className="bt-tile bt-line"><Line r={r} priv={priv} /></div>
      </div>

      <footer className="bt-foot"><Brand /></footer>
    </div>
  )
}

const pad = (n: number) => String(n).padStart(2, '0')
const md = (d: string) => `${d.slice(5, 7)}.${d.slice(8, 10)}`

/** 周报 / 月报：每天玩了多久（单色柱，最多的一天用强调色） */
function DailyBars({ r }: { r: Recap }) {
  const start = new Date(r.from.slice(0, 10) + 'T00:00:00')
  const end = new Date(r.to.slice(0, 10) + 'T00:00:00')
  const byDate = new Map(r.daily_minutes.map((d) => [d.date, d.minutes]))
  const days: { date: string; minutes: number }[] = []
  for (let t = start.getTime(); t < end.getTime(); t += 86400000) {
    const d = new Date(t)
    const key = `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}`
    days.push({ date: key, minutes: byDate.get(key) ?? 0 })
  }
  const max = Math.max(1, ...days.map((d) => d.minutes))
  const best = days.reduce((a, b) => (b.minutes > a.minutes ? b : a), days[0])
  const week = days.length <= 7
  return (
    <div className="bt-bars">
      {days.map((d, i) => (
        <div key={d.date} className="bt-bar-col" title={`${d.date} ${hm(d.minutes)}`}>
          <div className="bt-bar-track"><i className={d === best && d.minutes > 0 ? 'hl' : ''} style={{ height: `${(d.minutes / max) * 100}%` }} /></div>
          <span className="mono">{week ? '一二三四五六日'[i] : (i + 1) % 5 === 0 || i === 0 ? i + 1 : ''}</span>
        </div>
      ))}
    </div>
  )
}
