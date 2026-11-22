import type { ReactNode } from 'react'
import { cnDate, clock, shown, type Privacy } from '../../lib/format'
import type { Recap } from '../../types'
import { hm } from '../DailyCard'
import { Bars, Brand, Cover } from '../parts'

/**
 * 年度故事页 1080×1920，一页一个结论。页序参考 PhumudzoSly/github-wrapped (MIT)。
 * 视觉和日报统一：整套色调从全年待得最久的世界封面取色（年度页外层设置），
 * 胶片颗粒、Bricolage 数字、等宽标签、格子（tile）组织信息；最后一页是「年度小票」。
 */
export interface StoryPage { key: string; title: string; render: (r: Recap, p: Privacy) => ReactNode }

const year = (r: Recap) => r.from.slice(0, 4)
const hours = (m: number) => Math.round(m / 60)
const ACCESS: Record<string, { label: string; color: string }> = {
  friends: { label: '好友房', color: '#3987e5' },
  'friends+': { label: '好友+', color: '#d95926' },
  public: { label: '公开房', color: '#199e70' },
  'invite+': { label: '邀请房', color: '#c98500' },
  invite: { label: '邀请房', color: '#c98500' },
  group: { label: '群组房', color: '#c98500' },
}

function Page({ r, label, children, cover }: { r: Recap; label: string; children: ReactNode; cover?: { id: string; url: string | null } }) {
  return (
    <div className="yp">
      {cover && (
        <div className="yp-cover">
          <Cover id={cover.id} url={cover.url} className="yp-cover-img" />
          <div className="yp-cover-fade" />
        </div>
      )}
      <div className="dc-grain" />
      <header className="yp-head mono"><span>{year(r)} · VRChat</span><span>{label}</span></header>
      <div className="yp-body">{children}</div>
      <footer className="yp-foot"><Brand /></footer>
    </div>
  )
}

export const PAGES: StoryPage[] = [
  {
    key: 'intro', title: '开场',
    render: (r) => {
      const w = r.top_worlds[0]
      return (
        <Page r={r} label="年度回顾" cover={w ? { id: w.world_id, url: w.thumbnail } : undefined}>
          <div className="yp-intro">
            <div className="yp-year">{year(r)}</div>
            <h1 className="yp-h1">{r.self_name ?? '旅行者'}的<br />VRChat 一年</h1>
            <p className="yp-lede">从 {cnDate(r.daily_minutes[0]?.date ?? r.from)} 到 {cnDate(r.daily_minutes[r.daily_minutes.length - 1]?.date ?? r.to)}</p>
          </div>
        </Page>
      )
    },
  },
  {
    key: 'overview', title: '总览',
    render: (r) => (
      <Page r={r} label="这一年">
        <h1 className="yp-h1">你在另一个世界<br />度过了 <em>{hours(r.total_minutes)}</em> 小时</h1>
        <p className="yp-lede">相当于不眠不休地待了 {Math.round(r.total_minutes / 60 / 24)} 天。</p>
        <div className="yp-grid yp-grid-2">
          <div className="yp-tile"><span className="yp-label mono">上线天数</span><b className="yp-num">{r.play_days}</b></div>
          <div className="yp-tile"><span className="yp-label mono">上线次数</span><b className="yp-num">{r.session_count}</b></div>
          <div className="yp-tile"><span className="yp-label mono">去过的世界</span><b className="yp-num">{r.world_count}</b><small>其中 {r.new_world_count} 个第一次去</small></div>
          <div className="yp-tile"><span className="yp-label mono">遇见的人</span><b className="yp-num">{r.people_count}</b></div>
          <div className="yp-tile yp-span-2"><span className="yp-label mono">按下快门</span><b className="yp-num">{r.photo_count}</b><small>次</small></div>
        </div>
      </Page>
    ),
  },
  {
    key: 'worlds', title: '世界',
    render: (r) => {
      const [first, ...rest] = r.top_worlds
      if (!first) return <Page r={r} label="世界"><h1 className="yp-h1">还没有去过任何世界</h1></Page>
      return (
        <Page r={r} label="你最常回去的地方">
          <Cover id={first.world_id} url={first.thumbnail} className="yp-tile yp-hero">
            <div className="yp-hero-shade" />
            <div className="yp-hero-cap">
              <span className="yp-label mono">No.1</span>
              <b>{first.name}</b>
              <span className="mono">{hours(first.minutes)} 小时 · 回来了 {first.visits} 次</span>
            </div>
          </Cover>
          <div className="yp-grid yp-grid-2">
            {rest.slice(0, 4).map((w, i) => (
              <Cover key={w.world_id} id={w.world_id} url={w.thumbnail} className="yp-tile yp-mini">
                <div className="yp-hero-shade" />
                <div className="yp-hero-cap small">
                  <span className="yp-label mono">No.{i + 2} · {hours(w.minutes)}h</span>
                  <b>{w.name}</b>
                </div>
              </Cover>
            ))}
          </div>
        </Page>
      )
    },
  },
  {
    key: 'people', title: '人',
    render: (r, priv) => {
      const [first, ...rest] = r.top_people
      if (!first) return <Page r={r} label="人"><h1 className="yp-h1">这一年，你是独行的旅人</h1></Page>
      const max = first.minutes || 1
      return (
        <Page r={r} label="这一年陪你最久的人">
          <div className="yp-person">
            <h1 className="yp-name">{shown(first, priv).name}</h1>
            <p className="yp-lede">你们一起度过了 <em>{hours(first.minutes)}</em> 个小时</p>
            {first.best_day && <p className="yp-lede small">{cnDate(first.best_day.date)}那天，你们待了 {hm(first.best_day.minutes)}。</p>}
          </div>
          <div className="yp-tile yp-plist">
            {rest.slice(0, 4).map((p, i) => (
              <div key={p.user_id} className="yp-prow">
                <div><span className="mono">{i + 2}</span><b>{shown(p, priv).name}</b><i className="mono">{hours(p.minutes)}h</i></div>
                <div className="bt-pbar"><i style={{ width: `${(p.minutes / max) * 100}%` }} /></div>
              </div>
            ))}
          </div>
        </Page>
      )
    },
  },
  {
    key: 'months', title: '月份',
    render: (r) => {
      const peak = r.monthly_minutes.indexOf(Math.max(...r.monthly_minutes))
      return (
        <Page r={r} label="一年的节奏">
          <h1 className="yp-h1">最热闹的是 <em>{peak + 1} 月</em></h1>
          <p className="yp-lede">那个月你上线了 {hours(r.monthly_minutes[peak])} 小时。</p>
          <div className="yp-tile yp-chart">
            <Bars values={r.monthly_minutes} labels={r.monthly_minutes.map((_, i) => `${i + 1}月`)} highlight={peak} height={560} />
          </div>
          {r.busiest_day && (
            <div className="yp-tile yp-row">
              <span className="yp-label mono">最忙的一天</span>
              <b>{cnDate(r.busiest_day.date)}</b><span className="mono">{hm(r.busiest_day.minutes)}</span>
            </div>
          )}
        </Page>
      )
    },
  },
  {
    key: 'night', title: '夜猫子',
    render: (r) => {
      const late = r.latest_logoff
      const small = late && Number(clock(late.end).slice(0, 2)) < 6
      return (
        <Page r={r} label="夜猫子时间">
          {late && (
            <div className="yp-clock">
              <span className="yp-label mono">最晚的一次下线</span>
              <b>{clock(late.end)}</b>
              <p className="yp-lede">那是 {cnDate(late.end)}的{small ? '凌晨' : '深夜'}。</p>
            </div>
          )}
          <div className="yp-tile yp-chart">
            <span className="yp-label mono">一天里什么时候在线</span>
            <Bars values={r.hour_minutes} labels={r.hour_minutes.map((_, i) => `${i}`)} highlight={r.peak_hour ?? undefined} height={420} labelEvery={3} />
          </div>
          {r.peak_hour != null && <p className="yp-lede small">你最常在 {r.peak_hour} 点到 {r.peak_hour + 1} 点之间出现。</p>}
        </Page>
      )
    },
  },
  {
    key: 'longest', title: '最长一次',
    render: (r) => {
      const l = r.longest_session
      return (
        <Page r={r} label="最长的一次">
          {l && (
            <>
              <div className="yp-clock">
                <span className="yp-label mono">连续待了</span>
                <b>{hm(l.minutes)}</b>
              </div>
              <div className="yp-tile yp-row">
                <span className="yp-label mono">上线</span><b>{cnDate(l.start)} {clock(l.start)}</b>
              </div>
              <div className="yp-tile yp-row">
                <span className="yp-label mono">下线</span><b>{cnDate(l.end)} {clock(l.end)}</b>
              </div>
            </>
          )}
          <p className="yp-lede small">一共上线了 {r.session_count} 次，平均每次 {hm(Math.round(r.total_minutes / Math.max(1, r.session_count)))}。</p>
        </Page>
      )
    },
  },
  {
    key: 'rooms', title: '房间类型',
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
        <Page r={r} label="你和谁在一起">
          <h1 className="yp-h1"><em>{Math.round(friendly * 100)}%</em> 的时间，<br />你待在好友的房间里</h1>
          <div className="yp-tile">
            <div className="stack">
              {entries.map(([k, v]) => <div key={k} title={`${k} ${hours(v)} 小时`} style={{ flex: v, background: color(k) }} />)}
            </div>
            <ul className="legend">
              {entries.map(([k, v]) => (
                <li key={k}><i style={{ background: color(k) }} />{k}<span>{Math.round((v / total) * 100)}% · {hours(v)}h</span></li>
              ))}
            </ul>
          </div>
        </Page>
      )
    },
  },
  {
    key: 'photos', title: '照片',
    render: (r) => (
      <Page r={r} label="被记住的瞬间">
        <h1 className="yp-h1">你按下了 <em>{r.photo_count}</em> 次快门</h1>
        {r.top_photo_world && <p className="yp-lede">拍得最多的是「{r.top_photo_world.name}」，{r.top_photo_world.photos} 张。</p>}
        <div className="photo-wall">
          {r.photos.slice(0, 9).map((p) => <Cover key={p.path} id={p.path} url={p.url} className="wall-item" />)}
        </div>
      </Page>
    ),
  },
  {
    key: 'summary', title: '年度小票',
    render: (r, priv) => (
      <Page r={r} label="年度小票">
        <div className="yp-receipt">
          <div className="rc-top">
            <div className="rc-brand">yyMemoir</div>
            <div className="rc-meta mono"><span>{year(r)} 年度回顾</span><span>{r.self_name ?? ''}</span></div>
          </div>
          <div className="rc-cols mono"><span>最常去</span><span>小时</span></div>
          <div className="rc-rows mono">
            {r.top_worlds.slice(0, 5).map((w, i) => <div key={w.world_id}><span className="rc-person">{String(i + 1).padStart(2, '0')}  {w.name}</span><span>{hours(w.minutes)}</span></div>)}
          </div>
          <div className="rc-rule" />
          <div className="rc-sub mono">同行</div>
          <div className="rc-rows mono">
            {r.top_people.slice(0, 5).map((p) => <div key={p.user_id}><span className="rc-person">{shown(p, priv).name}</span><span>{hours(p.minutes)}</span></div>)}
          </div>
          <div className="rc-rule" />
          <div className="rc-rows mono">
            <div><span>上线天数</span><span>{r.play_days}</span></div>
            <div><span>世界</span><span>{r.world_count}</span></div>
            <div><span>遇见</span><span>{r.people_count} 人</span></div>
            <div><span>照片</span><span>{r.photo_count} 张</span></div>
            {r.latest_logoff && <div><span>最晚下线</span><span>{clock(r.latest_logoff.end)}</span></div>}
          </div>
          <div className="rc-rule double" />
          <div className="rc-total"><span>合计</span><b>{hours(r.total_minutes)} 小时</b></div>
          <div className="rc-thanks mono yp-thanks">SEE YOU NEXT YEAR</div>
        </div>
      </Page>
    ),
  },
]
