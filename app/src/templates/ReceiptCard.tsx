import { hash, shown, type Privacy } from '../lib/format'
import type { Recap } from '../types'
import { WEEK, clock, hm, itinerary, useCardTone } from './DailyCard'
import { Cover } from './parts'

/**
 * 方向 B：小票（参考 Receiptify / @albumreceipts）1080×1440。
 * 去过的世界当「商品」一行行列出，同行的人、合计、条形码。小票本身就是密集信息的形式。
 */
const ACCESS: Record<string, string> = { public: '公开', friends: '好友', 'friends+': '好友+', invite: '邀请', 'invite+': '邀请+', group: '群组' }

function Barcode({ seed }: { seed: string }) {
  let x = hash(seed)
  const bars: number[] = []
  for (let i = 0; i < 48; i++) {
    x = Math.imul(x ^ (x >>> 13), 2654435761) >>> 0
    bars.push(1 + (x % 4))
  }
  return <div className="rc-barcode">{bars.map((w, i) => <i key={i} style={{ width: w * 3, marginRight: ((x >> i) & 1) ? 4 : 6 }} />)}</div>
}

export function ReceiptCard({ r, priv }: { r: Recap; priv: Privacy }) {
  const style = useCardTone(r)
  const session = r.kind === 'session'
  const top = r.top_worlds[0]
  const day = r.from.slice(0, 10)
  const date = new Date(day + 'T00:00:00')
  const people = r.top_people.slice(0, 4)
  // 小票长度有限：放不下时保留最久的几站，按时间排
  const all = itinerary(r.timeline ?? [])
  const keep = new Set([...all].sort((a, b) => b.minutes - a.minutes).slice(0, people.length ? 5 : 7))
  const stops = all.filter((s) => keep.has(s))
  const more = all.length - stops.length
  const lastEnd = r.last_logoff ?? r.latest_logoff?.end
  const no = String(hash(r.from + (lastEnd ?? ''))).slice(0, 6)

  return (
    <div className="rc" style={style}>
      {top && <Cover id={top.world_id} url={top.thumbnail} className="rc-bg" />}
      <div className="rc-bg-shade" />
      <div className="rc-paper">
        <div className="rc-top">
          <div className="rc-brand">yyMemoir</div>
          <div className="rc-meta mono">
            <span>{session ? '本次回顾' : '今日回顾'} · No.{no}</span>
            <span>{day.replace(/-/g, '.')} {WEEK[date.getDay()]} · {session ? `${clock(r.from)}–${clock(lastEnd ?? r.to)}` : `全天 · ${lastEnd ? clock(lastEnd) + ' 下线' : ''}`}</span>
          </div>
        </div>

        <div className="rc-cols mono"><span>世界</span><span>时长</span></div>
        <ol className="rc-items">
          {stops.map((s, i) => (
            <li key={i}>
              <span className="rc-qty mono">{String(i + 1).padStart(2, '0')}</span>
              <span className="rc-name">
                {s.name}
                <small className="mono">{clock(s.start)}–{clock(s.end)} · {ACCESS[s.access_type] ?? s.access_type}</small>
              </span>
              <span className="rc-dur mono">{hm(s.minutes)}</span>
            </li>
          ))}
        </ol>
        {more > 0 && <div className="rc-more mono">… 另有 {more} 站</div>}

        <div className="rc-rule" />
        <div className="rc-rows mono">
          <div><span>世界</span><span>{r.world_count}</span></div>
          <div><span>遇见</span><span>{r.people_count} 人</span></div>
          <div><span>照片</span><span>{r.photo_count} 张</span></div>
        </div>

        {people.length > 0 && (
          <>
            <div className="rc-rule" />
            <div className="rc-sub mono">同行</div>
            <div className="rc-rows mono">
              {people.map((p) => <div key={p.user_id}><span className="rc-person">{shown(p, priv).name}</span><span>{hm(p.minutes)}</span></div>)}
            </div>
          </>
        )}

        <div className="rc-rule double" />
        <div className="rc-total"><span>合计</span><b>{hm(r.total_minutes)}</b></div>

        <div className="rc-end">
          <Barcode seed={r.from + no} />
          <div className="rc-thanks mono">THANK YOU FOR TRAVELLING</div>
        </div>
      </div>
    </div>
  )
}
