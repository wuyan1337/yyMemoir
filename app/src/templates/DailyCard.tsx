import { cnDate, duration, shown, type Privacy } from '../lib/format'
import type { Recap } from '../types'
import { Avatar, Brand, Cover } from './parts'

/** 今日卡片 1080×1440（3:4，适合小红书 / 朋友圈） */
export function DailyCard({ r, priv }: { r: Recap; priv: Privacy }) {
  const top = r.top_worlds[0]
  const people = r.top_people.slice(0, 3)
  const photo = r.photos[0]
  const late = r.latest_logoff
  return (
    <div className="daily">
      <div className="daily-glow" />
      <header className="daily-head">
        <div className="eyebrow">今日回顾 · {cnDate(r.from)}</div>
        <h1>今天你在 VRChat 待了<br /><em>{duration(r.total_minutes)}</em></h1>
        <p className="lede">
          走过 {r.world_count} 个世界，遇见 {r.people_count} 个人
          {late ? `，${late.end.slice(11, 16)} 才说晚安` : ''}
        </p>
      </header>

      {top && (
        <Cover id={top.world_id} url={top.thumbnail} className="daily-cover">
          <div className="cover-shade" />
          <div className="cover-text">
            <div className="tag">待得最久的世界</div>
            <div className="cover-name">{top.name}</div>
            <div className="cover-meta">{duration(top.minutes)}{top.visits > 1 ? ` · 来回 ${top.visits} 次` : ''}</div>
          </div>
        </Cover>
      )}

      <section className="daily-people">
        <div className="tag">今天陪你最久的人</div>
        <div className="people-row">
          {people.map((p) => (
            <div key={p.user_id} className="person">
              <Avatar p={p} priv={priv} size={132} />
              <div className="person-name">{shown(p, priv).name}</div>
              <div className="person-time">{duration(p.minutes)}</div>
            </div>
          ))}
          {people.length === 0 && <div className="muted">今天是一个人的旅行</div>}
        </div>
      </section>

      <footer className="daily-foot">
        {photo ? (
          <div className="daily-photo">
            <Cover id={photo.path} className="photo-thumb" />
            <div>
              <div className="tag">今天的 {r.photo_count} 张照片之一</div>
              <div className="photo-meta">{photo.taken_at.slice(11, 16)} · {photo.world_name ?? '某个世界'}</div>
            </div>
          </div>
        ) : <div />}
        <Brand sub="下线自动生成" />
      </footer>
    </div>
  )
}
