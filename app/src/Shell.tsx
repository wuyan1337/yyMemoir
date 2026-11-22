import { useCallback, useEffect, useRef, useState } from 'react'
import { LoggingAlert } from './components/LoggingAlert'
import {
  appStatus, autostart, isTauri, listSessions, onAppEvent, periodRecap, sessionRecap, shiftDate, today,
  type AppStatus, type PeriodKind, type SessionItem,
} from './lib/data'
import { exportPng } from './lib/exportPng'
import type { Privacy } from './lib/format'
import { RecapCard } from './templates/RecapCard'
import { Canvas } from './templates/parts'
import { StoryPlayer } from './templates/story/StoryPlayer'
import type { Recap } from './types'

/** 桌面端主界面：本次回顾 / 日报 / 周报 / 月报 / 年度报告 / 设置 */
type Tab = 'session' | PeriodKind | 'settings'
const TABS: { id: Tab; label: string }[] = [
  { id: 'session', label: '本次回顾' },
  { id: 'day', label: '日报' },
  { id: 'week', label: '周报' },
  { id: 'month', label: '月报' },
  { id: 'year', label: '年度报告' },
  { id: 'settings', label: '设置' },
]

function useFitScale(ref: React.RefObject<HTMLElement | null>, w: number, h: number) {
  const [s, setS] = useState(0.3)
  useEffect(() => {
    const fit = () => {
      const el = ref.current
      if (!el) return
      const availH = window.innerHeight - el.getBoundingClientRect().top - 70
      setS(Math.max(0.15, Math.min(el.clientWidth / w, availH / h, 1)))
    }
    fit()
    const ro = new ResizeObserver(fit)
    if (ref.current) ro.observe(ref.current)
    window.addEventListener('resize', fit)
    return () => { ro.disconnect(); window.removeEventListener('resize', fit) }
  }, [ref, w, h])
  return s
}

/** 一张卡片 + 保存按钮 */
function CardView({ r, priv, fileName, label }: { r: Recap | null; priv: Privacy; fileName: string; label: string }) {
  const boxRef = useRef<HTMLDivElement>(null)
  const cardRef = useRef<HTMLDivElement>(null)
  const scale = useFitScale(boxRef, 1080, 1440)
  const [busy, setBusy] = useState(false)
  return (
    <div ref={boxRef} className="card-view">
      {r && r.total_minutes > 0 ? (
        <>
          <Canvas ref={cardRef} w={1080} h={1440} scale={scale}><RecapCard r={r} priv={priv} /></Canvas>
          <div className="player-bar">
            <span>{label}</span>
            <button disabled={busy} onClick={async () => {
              if (!cardRef.current) return
              setBusy(true)
              await exportPng(cardRef.current, fileName)
              setBusy(false)
            }}>保存图片</button>
          </div>
        </>
      ) : (
        <div className="empty">
          <b>{r ? '这段时间还没有游玩记录' : '还没有游玩记录'}</b>
          <p>开着 yyMemoir 玩一次 VRChat，下线时就会收到第一份回顾。</p>
        </div>
      )}
    </div>
  )
}

function SessionView({ priv, openId, refreshKey }: { priv: Privacy; openId: number | null; refreshKey: number }) {
  const [list, setList] = useState<SessionItem[]>([])
  const [sel, setSel] = useState<number | null>(null)
  const [r, setR] = useState<Recap | null>(null)
  useEffect(() => { listSessions().then(setList) }, [refreshKey])
  useEffect(() => { if (openId != null) setSel(openId) }, [openId])
  useEffect(() => { sessionRecap(sel).then(setR) }, [sel, refreshKey])
  const current = sel ?? list[0]?.id ?? null
  return (
    <div className="split">
      <aside className="sessions" aria-label="每次游玩">
        {list.length === 0 && <p className="muted-s">还没有记录</p>}
        {list.map((s) => (
          <button key={s.id} className={`session-item ${s.id === current ? 'on' : ''}`} onClick={() => setSel(s.id)}>
            <b>{s.start.slice(5, 10).replace('-', '.')}</b>
            <span className="mono">{s.start.slice(11, 16)}–{s.end.slice(11, 16)}</span>
            <i className="mono">{Math.floor(s.minutes / 60)}h {String(s.minutes % 60).padStart(2, '0')}m</i>
          </button>
        ))}
      </aside>
      <CardView r={r} priv={priv} label="本次回顾" fileName={`yyMemoir-本次-${(r?.from ?? '').slice(0, 16).replace(/[: ]/g, '-')}.png`} />
    </div>
  )
}

const PERIOD_LABEL: Record<PeriodKind, string> = { day: '日报', week: '周报', month: '月报', year: '年度报告' }

function PeriodView({ kind, priv, refreshKey }: { kind: Exclude<PeriodKind, 'year'>; priv: Privacy; refreshKey: number }) {
  const [date, setDate] = useState(today())
  const [r, setR] = useState<Recap | null>(null)
  useEffect(() => { periodRecap(kind, date).then(setR) }, [kind, date, refreshKey])
  return (
    <div className="period">
      <div className="date-nav">
        <button onClick={() => setDate(shiftDate(date, kind, -1))} aria-label="上一个">‹</button>
        <span className="mono">{r ? (kind === 'day' ? r.from.slice(0, 10) : `${r.from.slice(0, 10)} – ${shiftDate(r.to.slice(0, 10), 'day', -1)}`) : date}</span>
        <button onClick={() => setDate(shiftDate(date, kind, 1))} aria-label="下一个">›</button>
        <button className="ghost" onClick={() => setDate(today())}>回到现在</button>
      </div>
      <CardView r={r} priv={priv} label={PERIOD_LABEL[kind]} fileName={`yyMemoir-${PERIOD_LABEL[kind]}-${date}.png`} />
    </div>
  )
}

function YearView({ priv }: { priv: Privacy }) {
  const boxRef = useRef<HTMLDivElement>(null)
  const scale = useFitScale(boxRef, 1080, 1920)
  const [r, setR] = useState<Recap | null>(null)
  useEffect(() => { periodRecap('year', today()).then(setR) }, [])
  return <div ref={boxRef} className="card-view">{r && <StoryPlayer r={r} priv={priv} scale={scale} />}</div>
}

function SettingsView({ priv, setPriv, status }: { priv: Privacy; setPriv: (p: Privacy) => void; status: AppStatus | null }) {
  const [auto, setAuto] = useState<boolean | null>(null)
  useEffect(() => { autostart().then((m) => m?.isEnabled().then(setAuto)) }, [])
  const toggleAuto = async (on: boolean) => {
    const m = await autostart()
    if (!m) return
    await (on ? m.enable() : m.disable())
    setAuto(await m.isEnabled())
  }
  return (
    <div className="settings">
      <section>
        <h2>后台记录</h2>
        <label className="row">
          <input id="autostart" type="checkbox" checked={!!auto} disabled={auto === null} onChange={(e) => toggleAuto(e.target.checked)} />
          <span><b>开机自动启动</b><small>只在托盘里安静记录，下线时推送本次回顾。关掉后需要手动打开 yyMemoir 才会记录。</small></span>
        </label>
        {!isTauri() && <p className="muted-s">（浏览器预览里不可用）</p>}
      </section>
      <section>
        <h2>分享隐私</h2>
        <label className="row">
          <input id="hide-strangers" type="checkbox" checked={priv.hideStrangers} onChange={(e) => setPriv({ ...priv, hideStrangers: e.target.checked })} />
          <span><b>隐藏非好友的名字</b><small>登录 VRChat 后才能分辨好友，没登录时会隐藏所有人。</small></span>
        </label>
        <label className="row">
          <input id="hide-all" type="checkbox" checked={priv.hideAll} onChange={(e) => setPriv({ ...priv, hideAll: e.target.checked })} />
          <span><b>隐藏所有人的名字</b><small>导出图片里显示为「一位旅伴」。</small></span>
        </label>
      </section>
      <section>
        <h2>VRChat 日志</h2>
        {status?.log_dir && (
          <p className="kv"><span>日志文件夹</span><code>{status.log_dir}</code><em className={status.log_dir_found ? 'ok' : 'bad'}>{status.log_dir_found ? '已找到' : '没找到'}</em></p>
        )}
        <p className="muted-s">请确认游戏里 设置 → Debug → 日志（Logging）是「完整（Full）」，否则记录不到去过的世界和遇到的人。</p>
      </section>
    </div>
  )
}

export function Shell() {
  const [tab, setTab] = useState<Tab>('session')
  const [priv, setPriv] = useState<Privacy>({ hideStrangers: false, hideAll: false })
  const [status, setStatus] = useState<AppStatus | null>(null)
  const [openId, setOpenId] = useState<number | null>(null)
  const [refreshKey, setRefreshKey] = useState(0)
  const refresh = useCallback(() => { setRefreshKey((k) => k + 1); appStatus().then(setStatus) }, [])

  useEffect(() => {
    appStatus().then(setStatus)
    const offs: Promise<() => void>[] = [
      onAppEvent('session-ended', () => refresh()),
      onAppEvent('open-session', (id) => { refresh(); setTab('session'); if (id != null) setOpenId(id) }),
    ]
    return () => { offs.forEach((p) => p.then((off) => off())) }
  }, [refresh])

  return (
    <div className="app">
      <header className="topbar">
        <div className="logo">yyMemoir</div>
        <nav className="tabs" role="tablist">
          {TABS.map((t) => (
            <button key={t.id} role="tab" aria-selected={tab === t.id} onClick={() => setTab(t.id)}>{t.label}</button>
          ))}
        </nav>
      </header>
      <LoggingAlert status={status} />
      <main className="stage">
        {tab === 'session' && <SessionView priv={priv} openId={openId} refreshKey={refreshKey} />}
        {(tab === 'day' || tab === 'week' || tab === 'month') && <PeriodView key={tab} kind={tab} priv={priv} refreshKey={refreshKey} />}
        {tab === 'year' && <YearView priv={priv} />}
        {tab === 'settings' && <SettingsView priv={priv} setPriv={setPriv} status={status} />}
      </main>
    </div>
  )
}
