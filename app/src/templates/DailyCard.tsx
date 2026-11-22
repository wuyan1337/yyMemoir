import { useTone } from '../lib/color'
import { shown, type Privacy } from '../lib/format'
import type { Recap, Stop } from '../types'
import { Brand, Cover } from './parts'

/**
 * 今日卡片 1080×1440（3:4）。版式是「当天的行程票根」：
 * 顶部是待得最久的世界封面；24 小时时间轴看出什么时候在线；按时间列出去过的世界；
 * 当天拍了照才出现照片条；底部是一起最久的人。整张卡的色调从封面里取。
 */

const WEEK = ['SUN', 'MON', 'TUE', 'WED', 'THU', 'FRI', 'SAT']
const hm = (m: number) => {
  const h = Math.floor(m / 60), r = m % 60
  return h ? (r ? `${h}h ${String(r).padStart(2, '0')}m` : `${h}h`) : `${r}m`
}
const clock = (s: string) => s.slice(11, 16)

/** 去掉开游戏时的短暂停留（< 5 分钟），连续同一个世界合并（时长累加，起止取首尾） */
function itinerary(stops: Stop[]): Stop[] {
  const out: Stop[] = []
  for (const s of stops) {
    if (s.minutes < 5) continue
    const last = out[out.length - 1]
    if (last && last.world_id === s.world_id) {
      out[out.length - 1] = { ...last, end: s.end, minutes: last.minutes + s.minutes }
    } else out.push({ ...s })
  }
  return out
}

const cn = (m: number) => {
  const h = Math.floor(m / 60), r = m % 60
  return h ? (r ? `${h} 小时 ${r} 分` : `${h} 小时`) : `${r} 分钟`
}

/** 一句有情绪的总结：去哪最久、和谁最久、熬到几点 */
function Line({ r, priv }: { r: Recap; priv: Privacy }) {
  const top = r.top_worlds[0]
  const p = r.top_people[0]
  const last = r.last_logoff
  const lateNight = last && last.slice(0, 10) > r.from.slice(0, 10)
  const name = p ? shown(p, priv).name : null
  return (
    <p className="dc-line">
      {top && <>{r.kind === 'session' ? '这次' : '今天'}在<b>「{top.name}」</b>待得最久，</>}
      {name ? <>和 <b>{name}</b> 一起度过了 <em>{cn(p!.minutes)}</em>。</> : <>一个人也逛得很开心。</>}
      {lateNight && <>第二天 <em>{last!.slice(11, 16)}</em> 才说晚安。</>}
    </p>
  )
}

/** 某个时间点在当天的分钟数（跨到第二天的算 1440） */
function minuteOfDay(t: string, day: string) {
  if (t.slice(0, 10) > day) return 1440
  return Number(t.slice(11, 13)) * 60 + Number(t.slice(14, 16))
}

function DayRibbon({ stops, day, longest }: { stops: Stop[]; day: string; longest?: string }) {
  return (
    <div className="dc-ribbon">
      <div className="dc-ribbon-track">
        {stops.map((s, i) => {
          const a = minuteOfDay(s.start, day), b = Math.max(a + 3, minuteOfDay(s.end, day))
          return (
            <div key={i} className={`dc-seg ${s.world_id === longest ? 'hl' : ''}`}
              title={`${clock(s.start)}–${clock(s.end)} ${s.name}`}
              style={{ left: `${(a / 1440) * 100}%`, width: `${((b - a) / 1440) * 100}%` }} />
          )
        })}
      </div>
      <div className="dc-ribbon-ticks mono">
        {['00', '06', '12', '18', '24'].map((t) => <span key={t}>{t}</span>)}
      </div>
    </div>
  )
}

export function DailyCard({ r, priv }: { r: Recap; priv: Privacy }) {
  const top = r.top_worlds[0]
  const tone = useTone(top?.thumbnail)
  const h = Math.round(tone?.h ?? 230)
  const sat = Math.round(Math.max(18, Math.min(45, (tone?.s ?? 0.3) * 60)))
  const style = {
    '--tint-bg': `hsl(${h} ${sat}% 9%)`,
    '--tint-ink': `hsl(${h} 30% 94%)`,
    '--tint-ink-2': `hsl(${h} 14% 72%)`,
    '--tint-ink-3': `hsl(${h} 10% 52%)`,
    '--tint-line': `hsl(${h} 20% 100% / .12)`,
    '--tint-track': `hsl(${h} 20% 100% / .07)`,
    '--tint-seg': `hsl(${h} 30% 58%)`,
    '--tint-accent': `hsl(${h} 70% 78%)`,
  } as React.CSSProperties

  const day = r.from.slice(0, 10)
  const date = new Date(day + 'T00:00:00')
  const all = r.timeline ?? []
  const stops = itinerary(all)
  const session = r.kind === 'session'
  const photos = r.photos.slice(0, 4)
  // 保证不挤：今日卡片有时间轴，少放一站；有照片再少放一站
  const maxStops = (session ? 5 : 4) - (photos.length ? 1 : 0)
  // 放不下时保留待得最久的几站，再按时间顺序排
  const keep = new Set([...stops].sort((a, b) => b.minutes - a.minutes).slice(0, maxStops))
  const shownStops = stops.filter((s) => keep.has(s))
  const more = stops.length - shownStops.length
  const lastEnd = r.last_logoff ?? r.latest_logoff?.end

  return (
    <div className="dc" style={style}>
      <div className="dc-grain" />
      {top && (
        <div className="dc-hero">
          <Cover id={top.world_id} url={top.thumbnail} className="dc-hero-img" />
          <div className="dc-hero-fade" />
        </div>
      )}

      <div className="dc-body">
        <div className="dc-head">
          <div>
            <div className="dc-md">{String(date.getMonth() + 1).padStart(2, '0')}.{String(date.getDate()).padStart(2, '0')}</div>
            <div className="dc-wk mono">
              {WEEK[date.getDay()]} · {session ? `${clock(r.from)}–${clock(lastEnd ?? r.to)}` : date.getFullYear()}
            </div>
          </div>
          <div className="dc-total">
            <div className="dc-total-n">{hm(r.total_minutes)}</div>
            <div className="dc-total-l">{r.world_count} 个世界 · {r.people_count} 人{!session && lastEnd ? ` · ${clock(lastEnd)} 下线` : ''}</div>
          </div>
        </div>

        {!session && <DayRibbon stops={all} day={day} longest={top?.world_id} />}

        <ol className="dc-stops">
          {shownStops.map((s, i) => (
            <li key={i} className={s.world_id === top?.world_id ? 'hl' : ''}>
              <span className="dc-time mono">{clock(s.start)}<i>–{clock(s.end)}</i></span>
              <Cover id={s.world_id} url={s.thumbnail} className="dc-thumb" />
              <span className="dc-name">{s.name}</span>
              <span className="dc-dur mono">{hm(s.minutes)}</span>
            </li>
          ))}
          {more > 0 && <li className="dc-more mono">还去了 {more} 个地方</li>}
        </ol>

        {photos.length > 0 && (
          <div className="dc-photos">
            {photos.map((p) => (
              <div key={p.path} className="dc-photo">
                <Cover id={p.path} url={p.url} className="dc-photo-img" />
                <span className="mono">{clock(p.taken_at)}</span>
              </div>
            ))}
            {r.photo_count > photos.length && <div className="dc-photo-more mono">共 {r.photo_count} 张</div>}
          </div>
        )}

        <Line r={r} priv={priv} />

        <div className="dc-foot">
          <span className="dc-kind mono">{session ? '本次回顾' : '今日回顾'}</span>
          <Brand />
        </div>
      </div>
    </div>
  )
}
