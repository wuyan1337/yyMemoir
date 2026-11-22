import { useTone } from '../lib/color'
import { shown, type Privacy } from '../lib/format'
import type { Recap, Stop } from '../types'
import { Brand, Cover } from './parts'

/**
 * 今日卡片 1080×1440（3:4）。版式是「当天的行程票根」：
 * 顶部是待得最久的世界封面；中间按时间列出去过的世界；底部是一起最久的人。
 * 整张卡的色调从封面里取，每天都不一样。
 */

const WEEK = ['SUN', 'MON', 'TUE', 'WED', 'THU', 'FRI', 'SAT']
const hm = (m: number) => {
  const h = Math.floor(m / 60), r = m % 60
  return h ? (r ? `${h}h ${String(r).padStart(2, '0')}m` : `${h}h`) : `${r}m`
}

/** 去掉开游戏时的短暂停留（< 5 分钟），连续同一个世界合并 */
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

export function DailyCard({ r, priv }: { r: Recap; priv: Privacy }) {
  const top = r.top_worlds[0]
  const tone = useTone(top?.thumbnail)
  const h = Math.round(tone?.h ?? 230)
  const sat = Math.round(Math.max(18, Math.min(45, (tone?.s ?? 0.3) * 60)))
  const style = {
    '--tint-bg': `hsl(${h} ${sat}% 9%)`,
    '--tint-bg-2': `hsl(${h} ${sat}% 14%)`,
    '--tint-ink': `hsl(${h} 30% 94%)`,
    '--tint-ink-2': `hsl(${h} 14% 72%)`,
    '--tint-ink-3': `hsl(${h} 10% 52%)`,
    '--tint-line': `hsl(${h} 20% 100% / .12)`,
    '--tint-accent': `hsl(${h} 70% 78%)`,
  } as React.CSSProperties

  const date = new Date(r.from.slice(0, 10) + 'T00:00:00')
  const stops = itinerary(r.timeline ?? [])
  const shownStops = stops.slice(0, 6)
  const more = stops.length - shownStops.length
  const people = r.top_people.slice(0, 3)
  // 今日卡片显示当天最后一次下线的真实时间（跨零点不截断）
  const lastEnd = r.last_logoff ?? r.latest_logoff?.end

  return (
    <div className="dc" style={style}>
      <div className="dc-grain" />
      {top && (
        <div className="dc-hero">
          <Cover id={top.world_id} url={top.thumbnail} className="dc-hero-img" />
          <div className="dc-hero-fade" />
          <div className="dc-hero-cap">
            <span>今天待得最久</span>
            <b>{top.name}</b>
            <span className="mono">{hm(top.minutes)}</span>
          </div>
        </div>
      )}

      <div className="dc-body">
        <div className="dc-head">
          <div className="dc-date">
            <div className="dc-md">{String(date.getMonth() + 1).padStart(2, '0')}.{String(date.getDate()).padStart(2, '0')}</div>
            <div className="dc-wk mono">{WEEK[date.getDay()]} · {date.getFullYear()}</div>
          </div>
          <div className="dc-total">
            <div className="dc-total-n">{hm(r.total_minutes)}</div>
            <div className="dc-total-l">
              {r.world_count} 个世界 · 遇见 {r.people_count} 人{lastEnd ? ` · ${lastEnd.slice(11, 16)} 下线` : ''}
            </div>
          </div>
        </div>

        <div className="dc-sec mono">今日行程</div>
        <ol className="dc-stops">
          {shownStops.map((s, i) => (
            <li key={i}>
              <span className="dc-time mono">{s.start.slice(11, 16)}</span>
              <Cover id={s.world_id} url={s.thumbnail} className="dc-thumb" />
              <span className="dc-name">{s.name}</span>
              <span className="dc-dur mono">{hm(s.minutes)}</span>
            </li>
          ))}
          {more > 0 && <li className="dc-more mono">+ 另外 {more} 站</li>}
        </ol>

        <div className="dc-foot">
          <div className="dc-people">
            {people.length > 0 ? (
              <>
                <div className="dc-sec mono">一起最久</div>
                <div className="dc-names">
                  {people.map((p) => (
                    <span key={p.user_id}><b>{shown(p, priv).name}</b><i className="mono">{hm(p.minutes)}</i></span>
                  ))}
                </div>
              </>
            ) : <div className="dc-sec mono">今天是一个人的旅行</div>}
          </div>
          <Brand />
        </div>
      </div>
    </div>
  )
}
