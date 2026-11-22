import { shown, type Privacy } from '../lib/format'
import type { Recap } from '../types'
import { DayRibbon, Line, WEEK, clock, hm, itinerary, useCardTone } from './DailyCard'
import { Brand, Cover } from './parts'

/**
 * 方向 A：便当格（Bento）1080×1440。
 * 每类信息一个格子，信息多但各管一件事：大封面 / 总时长 / 时间轴 / 行程 / 陪伴的人 / 小数据。
 */
export function BentoCard({ r, priv }: { r: Recap; priv: Privacy }) {
  const style = useCardTone(r)
  const session = r.kind === 'session'
  const top = r.top_worlds[0]
  const day = r.from.slice(0, 10)
  const date = new Date(day + 'T00:00:00')
  const photos = r.photos.slice(0, 4)
  // 放不下时保留最久的 5 站，按时间排
  const all = itinerary(r.timeline ?? [])
  const keep = new Set([...all].sort((a, b) => b.minutes - a.minutes).slice(0, photos.length ? 3 : 5))
  const stops = all.filter((s) => keep.has(s))
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
          <b>{String(date.getMonth() + 1).padStart(2, '0')}.{String(date.getDate()).padStart(2, '0')}</b>
          <span className="mono">{WEEK[date.getDay()]}</span>
        </div>
        <span className="bt-kind mono">{session ? `本次回顾 · ${clock(r.from)}–${clock(lastEnd ?? r.to)}` : '今日回顾 · 全天'}</span>
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
          <span className="bt-label">{session ? '这次玩了' : '今天玩了'}</span>
          <b>{hm(r.total_minutes)}</b>
          <span className="bt-sub">{lastEnd ? `${clock(lastEnd)} 下线` : ''}</span>
        </div>

        <div className="bt-tile bt-nums">
          <div><b>{r.world_count}</b><span>个世界</span></div>
          <div><b>{r.people_count}</b><span>个人</span></div>
          <div><b>{r.photo_count}</b><span>张照片</span></div>
          <div><b>{Math.round((mainMin / allMin) * 100)}%</b><span>在{mainKind}</span></div>
        </div>

        <div className="bt-tile bt-ribbon">
          <span className="bt-label">什么时候在线</span>
          <DayRibbon stops={r.timeline ?? []} day={day} longest={top?.world_id} />
        </div>

        <div className="bt-tile bt-stops">
          <span className="bt-label">行程</span>
          <ol>
            {stops.map((s, i) => (
              <li key={i}>
                <Cover id={s.world_id} url={s.thumbnail} className="bt-thumb" />
                <span className="bt-sname">{s.name}</span>
                <span className="bt-stime mono">{clock(s.start)}</span>
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
