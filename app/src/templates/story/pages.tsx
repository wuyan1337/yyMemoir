import type { ReactNode } from 'react'
import { cnDate, clock, duration, hours, shown, type Privacy } from '../../lib/format'
import type { Recap } from '../../types'
import { Avatar, Bars, Brand, Cover } from '../parts'

/**
 * 年度故事页，一页一个结论。页序参考 PhumudzoSly/github-wrapped (MIT)：
 * 开场 → 总览 → 世界 → 人 → 月份 → 夜猫子 → 最长一次 → 房间类型 → 照片 → 总结
 */
export interface StoryPage { key: string; title: string; theme: string; render: (r: Recap, p: Privacy) => ReactNode }

const year = (r: Recap) => r.from.slice(0, 4)
const ACCESS: Record<string, { label: string; color: string }> = {
  friends: { label: '好友房', color: '#3987e5' },
  'friends+': { label: '好友+', color: '#d95926' },
  public: { label: '公开房', color: '#199e70' },
  'invite+': { label: '邀请房', color: '#c98500' },
  invite: { label: '邀请房', color: '#c98500' },
  group: { label: '群组房', color: '#c98500' },
}

function Page({ children, eyebrow }: { children: ReactNode; eyebrow?: string }) {
  return (
    <div className="page">
      {eyebrow && <div className="eyebrow">{eyebrow}</div>}
      <div className="page-body">{children}</div>
      <Brand sub="VRChat 年度回顾" />
    </div>
  )
}

export const PAGES: StoryPage[] = [
  {
    key: 'intro', title: '开场', theme: 't-intro',
    render: (r) => (
      <Page>
        <div className="intro-year">{year(r)}</div>
        <h1 className="big">{r.self_name ?? '旅行者'}，<br />这是你的<br />VRChat 一年</h1>
        <p className="lede">从 {cnDate(r.from)} 到 {cnDate(r.daily_minutes[r.daily_minutes.length - 1]?.date ?? r.to)}，<br />每一次上线，都被悄悄记了下来。</p>
      </Page>
    ),
  },
  {
    key: 'overview', title: '总览', theme: 't-overview',
    render: (r) => (
      <Page eyebrow="这一年">
        <h1 className="big">你在另一个世界<br />度过了 <em>{hours(r.total_minutes)}</em> 小时</h1>
        <p className="lede">相当于不眠不休地待了 {Math.round(r.total_minutes / 60 / 24)} 天。</p>
        <div className="stat-grid">
          <div><b>{r.play_days}</b><span>天上线</span></div>
          <div><b>{r.world_count}</b><span>个世界</span></div>
          <div><b>{r.people_count}</b><span>个人擦肩</span></div>
          <div><b>{r.photo_count}</b><span>张照片</span></div>
        </div>
      </Page>
    ),
  },
  {
    key: 'worlds', title: '世界', theme: 't-worlds',
    render: (r) => {
      const [first, ...rest] = r.top_worlds
      if (!first) return <Page><h1 className="big">还没有去过任何世界</h1></Page>
      return (
        <Page eyebrow="你最常回去的地方">
          <Cover id={first.world_id} url={first.thumbnail} className="world-hero">
            <div className="cover-shade" />
            <div className="cover-text">
              <div className="rank">No.1</div>
              <div className="cover-name">{first.name}</div>
              <div className="cover-meta">{hours(first.minutes)} 小时 · 回来了 {first.visits} 次</div>
            </div>
          </Cover>
          <ol className="rank-list">
            {rest.slice(0, 4).map((w, i) => (
              <li key={w.world_id}>
                <span className="rank-n">{i + 2}</span>
                <Cover id={w.world_id} url={w.thumbnail} className="rank-thumb" />
                <span className="rank-name">{w.name}</span>
                <span className="rank-val">{hours(w.minutes)}h</span>
              </li>
            ))}
          </ol>
          {r.new_world_count > 0 && <p className="lede small">今年你第一次踏进了 {r.new_world_count} 个新世界。</p>}
        </Page>
      )
    },
  },
  {
    key: 'people', title: '人', theme: 't-people',
    render: (r, priv) => {
      const [first, ...rest] = r.top_people
      if (!first) return <Page><h1 className="big">这一年，你是独行的旅人</h1></Page>
      const f = shown(first, priv)
      return (
        <Page eyebrow="这一年陪你最久的人">
          <div className="people-hero">
            <Avatar p={first} priv={priv} size={300} />
            <h1 className="big center">{f.name}</h1>
            <p className="lede center">你们一起度过了 <em>{hours(first.minutes)}</em> 个小时</p>
            {first.best_day && (
              <p className="lede small center">{cnDate(first.best_day.date)}那天，你们待了 {duration(first.best_day.minutes)}。</p>
            )}
          </div>
          <ol className="rank-list">
            {rest.slice(0, 4).map((p, i) => (
              <li key={p.user_id}>
                <span className="rank-n">{i + 2}</span>
                <Avatar p={p} priv={priv} size={72} />
                <span className="rank-name">{shown(p, priv).name}</span>
                <span className="rank-val">{hours(p.minutes)}h</span>
              </li>
            ))}
          </ol>
        </Page>
      )
    },
  },
  {
    key: 'months', title: '月份', theme: 't-months',
    render: (r) => {
      const peak = r.monthly_minutes.indexOf(Math.max(...r.monthly_minutes))
      return (
        <Page eyebrow="一年的节奏">
          <h1 className="big">最热闹的是 <em>{peak + 1} 月</em></h1>
          <p className="lede">那个月你上线了 {hours(r.monthly_minutes[peak])} 小时。</p>
          <Bars values={r.monthly_minutes} labels={r.monthly_minutes.map((_, i) => `${i + 1}月`)} highlight={peak} height={620} />
          {r.busiest_day && <p className="lede small">最忙的一天是 {cnDate(r.busiest_day.date)}，整整 {duration(r.busiest_day.minutes)}。</p>}
        </Page>
      )
    },
  },
  {
    key: 'night', title: '夜猫子', theme: 't-night',
    render: (r) => {
      const late = r.latest_logoff
      return (
        <Page eyebrow="夜猫子时间">
          {late && <h1 className="big">最晚的一次，<br />你 <em>{clock(late.end)}</em> 才下线</h1>}
          {late && <p className="lede">那是 {cnDate(late.end)}的{Number(clock(late.end).slice(0, 2)) < 6 ? '凌晨' : '深夜'}。</p>}
          <Bars values={r.hour_minutes} labels={r.hour_minutes.map((_, i) => `${i}`)} highlight={r.peak_hour ?? undefined} height={460} labelEvery={3} />
          {r.peak_hour != null && <p className="lede small">你最常在 {r.peak_hour} 点到 {r.peak_hour + 1} 点之间出现。</p>}
        </Page>
      )
    },
  },
  {
    key: 'longest', title: '最长一次', theme: 't-longest',
    render: (r) => {
      const l = r.longest_session
      return (
        <Page eyebrow="最长的一次">
          {l && (
            <>
              <h1 className="big">你连续待了<br /><em>{duration(l.minutes)}</em></h1>
              <p className="lede">{cnDate(l.start)} {clock(l.start)} 上线，<br />一直到 {cnDate(l.end) === cnDate(l.start) ? '' : cnDate(l.end) + ' '}{clock(l.end)}。</p>
              <div className="timeline-strip"><div style={{ width: `${Math.min(100, (l.minutes / 600) * 100)}%` }} /></div>
            </>
          )}
          <p className="lede small">一共上线了 {r.session_count} 次，平均每次 {duration(Math.round(r.total_minutes / Math.max(1, r.session_count)))}。</p>
        </Page>
      )
    },
  },
  {
    key: 'rooms', title: '房间类型', theme: 't-rooms',
    render: (r) => {
      const merged: Record<string, number> = {}
      for (const [k, v] of Object.entries(r.access_minutes)) {
        const label = (ACCESS[k] ?? ACCESS.group).label
        merged[label] = (merged[label] ?? 0) + v
      }
      const entries = Object.entries(merged).sort((a, b) => b[1] - a[1])
      const total = entries.reduce((s, [, v]) => s + v, 0) || 1
      const color = (label: string) => Object.values(ACCESS).find((a) => a.label === label)!.color
      const friendly = ((merged['好友房'] ?? 0) + (merged['好友+'] ?? 0)) / total
      return (
        <Page eyebrow="你和谁在一起">
          <h1 className="big"><em>{Math.round(friendly * 100)}%</em> 的时间，<br />你待在好友的房间里</h1>
          <div className="stack">
            {entries.map(([k, v]) => <div key={k} title={`${k} ${hours(v)} 小时`} style={{ flex: v, background: color(k) }} />)}
          </div>
          <ul className="legend">
            {entries.map(([k, v]) => (
              <li key={k}><i style={{ background: color(k) }} />{k}<span>{Math.round((v / total) * 100)}% · {hours(v)}h</span></li>
            ))}
          </ul>
        </Page>
      )
    },
  },
  {
    key: 'photos', title: '照片', theme: 't-photos',
    render: (r) => (
      <Page eyebrow="被记住的瞬间">
        <h1 className="big">你按下了 <em>{r.photo_count}</em> 次快门</h1>
        {r.top_photo_world && <p className="lede">拍得最多的是「{r.top_photo_world.name}」，{r.top_photo_world.photos} 张。</p>}
        <div className="photo-wall">
          {r.photos.slice(0, 9).map((p) => <Cover key={p.path} id={p.path} className="wall-item" />)}
        </div>
      </Page>
    ),
  },
  {
    key: 'summary', title: '总结', theme: 't-summary',
    render: (r, priv) => (
      <Page eyebrow={`${year(r)} · 我的 VRChat`}>
        <div className="summary">
          <div className="summary-big"><b>{hours(r.total_minutes)}</b><span>小时</span></div>
          <div className="summary-grid">
            <div><span>上线天数</span><b>{r.play_days}</b></div>
            <div><span>去过的世界</span><b>{r.world_count}</b></div>
            <div><span>遇见的人</span><b>{r.people_count}</b></div>
            <div><span>照片</span><b>{r.photo_count}</b></div>
          </div>
          <div className="summary-cols">
            <div>
              <div className="tag">最常去</div>
              {r.top_worlds.slice(0, 3).map((w, i) => <div key={w.world_id} className="summary-item">{i + 1}. {w.name}</div>)}
            </div>
            <div>
              <div className="tag">陪伴最久</div>
              {r.top_people.slice(0, 3).map((p, i) => <div key={p.user_id} className="summary-item">{i + 1}. {shown(p, priv).name}</div>)}
            </div>
          </div>
          {r.latest_logoff && <div className="summary-foot">最晚下线 {clock(r.latest_logoff.end)} · 最长一次 {duration(r.longest_session?.minutes ?? 0)}</div>}
        </div>
      </Page>
    ),
  },
]
