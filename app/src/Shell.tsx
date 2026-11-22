import { useCallback, useEffect, useMemo, useRef, useState } from 'react'
import { LoggingAlert } from './components/LoggingAlert'
import {
  appStatus, autostart, exportBackup, importBackup, isTauri, listSessions, onAppEvent, periodRecap, pickBackup, playDays, sessionRecap, today,
  type AppStatus, type DayItem, type PeriodKind, type SessionItem,
} from './lib/data'
import { copyPng, exportPng } from './lib/exportPng'
import type { Privacy } from './lib/format'
import { useCardTone } from './templates/DailyCard'
import { RecapCard, defaultStyle, type CardStyle } from './templates/RecapCard'
import { Canvas } from './templates/parts'
import { PAGES } from './templates/story/pages'
import type { Recap } from './types'

/** 桌面端主界面，三栏：导航 / 列表 / 卡片（工具栏在卡片上方） */
type Tab = 'session' | PeriodKind | 'settings'
const NAV: { id: Tab; label: string }[] = [
  { id: 'session', label: '本次' },
  { id: 'day', label: '日报' },
  { id: 'week', label: '周报' },
  { id: 'month', label: '月报' },
  { id: 'year', label: '年度' },
]
const WEEKDAY = '日一二三四五六'
const hm = (m: number) => (m >= 60 ? `${Math.floor(m / 60)}h ${String(m % 60).padStart(2, '0')}m` : `${m}m`)
const pad = (n: number) => String(n).padStart(2, '0')
const ymd = (d: Date) => `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}`
const dot = (d: string) => `${d.slice(5, 7)}.${d.slice(8, 10)}`

/** 卡片按容器大小等比缩放，铺满可用空间 */
function useFit(ref: React.RefObject<HTMLElement | null>, w: number, h: number) {
  const [s, setS] = useState(0.4)
  useEffect(() => {
    const el = ref.current
    if (!el) return
    const fit = () => setS(Math.max(0.15, Math.min(el.clientWidth / w, el.clientHeight / h)))
    fit()
    const ro = new ResizeObserver(fit)
    ro.observe(el)
    return () => ro.disconnect()
  }, [ref, w, h])
  return s
}

function useBusy() {
  const [busy, setBusy] = useState<string | null>(null)
  const run = async (label: string, fn: () => Promise<void>) => {
    setBusy(label)
    try { await fn(); setBusy(`${label}好了`) } catch { setBusy(`${label}失败`) }
    setTimeout(() => setBusy(null), 1500)
  }
  return { busy, run }
}

function Toolbar({ title, sub, children }: { title: string; sub?: string; children?: React.ReactNode }) {
  return (
    <div className="toolbar">
      <div className="tb-title"><b>{title}</b>{sub && <span className="mono">{sub}</span>}</div>
      <div className="tb-actions">{children}</div>
    </div>
  )
}

const STYLE_LABEL: Record<CardStyle, string> = { receipt: '小票', bento: '格子', ticket: '票根' }

/** 一张报告卡：工具栏（版式 / 复制 / 保存）+ 铺满的卡片 */
function CardPane({ r, priv, title, sub, fileName }: { r: Recap | null; priv: Privacy; title: string; sub?: string; fileName: string }) {
  const boxRef = useRef<HTMLDivElement>(null)
  const cardRef = useRef<HTMLDivElement>(null)
  const scale = useFit(boxRef, 1080, 1440)
  const [style, setStyle] = useState<CardStyle | null>(null)
  const { busy, run } = useBusy()
  const has = !!r && r.total_minutes > 0
  const current = style ?? (r ? defaultStyle(r) : 'bento')
  return (
    <>
      <Toolbar title={title} sub={sub}>
        {has && (
          <>
            <div className="seg" role="group" aria-label="版式">
              {(['receipt', 'bento', 'ticket'] as CardStyle[]).map((s) => (
                <button key={s} aria-pressed={current === s} onClick={() => setStyle(s)}>{STYLE_LABEL[s]}</button>
              ))}
            </div>
            <button className="btn" disabled={!!busy} onClick={() => run('复制', () => copyPng(cardRef.current!))}>{busy?.startsWith('复制') ? busy : '复制'}</button>
            <button className="btn primary" disabled={!!busy} onClick={() => run('保存', () => exportPng(cardRef.current!, fileName))}>{busy?.startsWith('保存') ? busy : '保存图片'}</button>
          </>
        )}
      </Toolbar>
      <div ref={boxRef} className="card-box">
        {has ? (
          <Canvas ref={cardRef} w={1080} h={1440} scale={scale}><RecapCard r={r!} priv={priv} style={current} /></Canvas>
        ) : (
          <div className="empty">还没有记录</div>
        )}
      </div>
    </>
  )
}

/* ---------------- 本次 ---------------- */
function SessionTab({ priv, openId, refreshKey }: { priv: Privacy; openId: number | null; refreshKey: number }) {
  const [list, setList] = useState<SessionItem[]>([])
  const [sel, setSel] = useState<number | null>(null)
  const [r, setR] = useState<Recap | null>(null)
  useEffect(() => { listSessions().then(setList) }, [refreshKey])
  useEffect(() => { if (openId != null) setSel(openId) }, [openId])
  const current = sel ?? list[0]?.id ?? null
  useEffect(() => { sessionRecap(current).then(setR) }, [current, refreshKey])
  const groups = useMemo(() => {
    const g: { date: string; items: SessionItem[] }[] = []
    for (const s of list) {
      const d = s.start.slice(0, 10)
      if (g[g.length - 1]?.date !== d) g.push({ date: d, items: [] })
      g[g.length - 1].items.push(s)
    }
    return g
  }, [list])
  const cur = list.find((s) => s.id === current)
  return (
    <>
      <aside className="panel">
        {groups.map((g) => (
          <section key={g.date}>
            <h3 className="mono">{dot(g.date)} 周{WEEKDAY[new Date(g.date + 'T00:00:00').getDay()]}</h3>
            {g.items.map((s) => (
              <button key={s.id} className={`item ${s.id === current ? 'on' : ''}`} onClick={() => setSel(s.id)}>
                <span className="mono">{s.start.slice(11, 16)}–{s.end.slice(11, 16)}</span>
                <i className="mono">{hm(s.minutes)}</i>
              </button>
            ))}
          </section>
        ))}
        {list.length === 0 && <p className="panel-empty">还没有记录</p>}
      </aside>
      <main className="main">
        <CardPane r={r} priv={priv} title="本次回顾" sub={cur ? `${dot(cur.start)} ${cur.start.slice(11, 16)}–${cur.end.slice(11, 16)}` : undefined}
          fileName={`yyMemoir-本次-${(cur?.start ?? '').replace(/[: ]/g, '-')}.png`} />
      </main>
    </>
  )
}

/* ---------------- 日 / 周 / 月 ---------------- */
interface PeriodItem { key: string; label: string; sub: string; minutes: number }

function groupDays(days: DayItem[], kind: 'day' | 'week' | 'month'): PeriodItem[] {
  const m = new Map<string, PeriodItem>()
  for (const d of days) {
    const dt = new Date(d.date + 'T00:00:00')
    let key = d.date, label = dot(d.date), sub = `周${WEEKDAY[dt.getDay()]}`
    if (kind === 'week') {
      const mon = new Date(dt); mon.setDate(dt.getDate() - ((dt.getDay() + 6) % 7))
      const sun = new Date(mon); sun.setDate(mon.getDate() + 6)
      key = ymd(mon); label = `${dot(ymd(mon))}–${dot(ymd(sun))}`; sub = `${mon.getFullYear()}`
    } else if (kind === 'month') {
      key = `${d.date.slice(0, 7)}-01`; label = `${d.date.slice(0, 4)}.${d.date.slice(5, 7)}`; sub = ''
    }
    const it = m.get(key) ?? { key, label, sub, minutes: 0 }
    it.minutes += d.minutes
    m.set(key, it)
  }
  return [...m.values()]
}

const KIND_TITLE = { day: '日报', week: '周报', month: '月报' } as const

function PeriodTab({ kind, priv, refreshKey }: { kind: 'day' | 'week' | 'month'; priv: Privacy; refreshKey: number }) {
  const [days, setDays] = useState<DayItem[]>([])
  const [sel, setSel] = useState<string | null>(null)
  const [r, setR] = useState<Recap | null>(null)
  useEffect(() => { playDays().then(setDays) }, [refreshKey])
  const items = useMemo(() => groupDays(days, kind), [days, kind])
  const current = sel ?? items[0]?.key ?? today()
  useEffect(() => { periodRecap(kind, current).then(setR) }, [kind, current, refreshKey])
  const max = Math.max(1, ...items.map((i) => i.minutes))
  const cur = items.find((i) => i.key === current)
  return (
    <>
      <aside className="panel">
        {items.map((it) => (
          <button key={it.key} className={`item day ${it.key === current ? 'on' : ''}`} onClick={() => setSel(it.key)}>
            <span className="mono">{it.label}</span>
            <small>{it.sub}</small>
            <i className="mono">{hm(it.minutes)}</i>
            <em style={{ width: `${(it.minutes / max) * 100}%` }} />
          </button>
        ))}
        {items.length === 0 && <p className="panel-empty">还没有记录</p>}
      </aside>
      <main className="main">
        <CardPane r={r} priv={priv} title={KIND_TITLE[kind]} sub={cur?.label ?? dot(current)} fileName={`yyMemoir-${KIND_TITLE[kind]}-${current}.png`} />
      </main>
    </>
  )
}

/* ---------------- 年度 ---------------- */
function YearTab({ priv }: { priv: Privacy }) {
  const [r, setR] = useState<Recap | null>(null)
  const [i, setI] = useState(0)
  const boxRef = useRef<HTMLDivElement>(null)
  const pageRef = useRef<HTMLDivElement>(null)
  const scale = useFit(boxRef, 1080, 1920)
  const { busy, run } = useBusy()
  useEffect(() => { periodRecap('year', today()).then(setR) }, [])
  const tone = useCardTone(r ?? ({ top_worlds: [] } as unknown as Recap))
  const n = PAGES.length
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === 'ArrowRight' || e.key === 'ArrowDown') setI((x) => Math.min(n - 1, x + 1))
      if (e.key === 'ArrowLeft' || e.key === 'ArrowUp') setI((x) => Math.max(0, x - 1))
    }
    window.addEventListener('keydown', onKey)
    return () => window.removeEventListener('keydown', onKey)
  }, [n])
  const year = r?.from.slice(0, 4) ?? ''
  const saveAll = async () => {
    for (let k = 0; k < n; k++) {
      setI(k)
      await new Promise((res) => setTimeout(res, 350))
      if (pageRef.current) await exportPng(pageRef.current, `yyMemoir-${year}-${pad(k + 1)}-${PAGES[k].key}.png`)
    }
  }
  return (
    <>
      <aside className="panel">
        <section>
          {PAGES.map((p, k) => (
            <button key={p.key} className={`item ${k === i ? 'on' : ''}`} onClick={() => setI(k)}>
              <span className="mono">{pad(k + 1)}</span><span>{p.title}</span>
            </button>
          ))}
        </section>
      </aside>
      <main className="main">
        <Toolbar title={`${year} 年度`} sub={`${i + 1} / ${n}`}>
          {r && r.total_minutes > 0 && (
            <>
              <div className="seg">
                <button onClick={() => setI(Math.max(0, i - 1))} aria-label="上一页">‹</button>
                <button onClick={() => setI(Math.min(n - 1, i + 1))} aria-label="下一页">›</button>
              </div>
              <button className="btn" disabled={!!busy} onClick={() => run('复制', () => copyPng(pageRef.current!))}>{busy?.startsWith('复制') ? busy : '复制'}</button>
              <button className="btn" disabled={!!busy} onClick={() => run('保存全部', saveAll)}>{busy?.startsWith('保存全部') ? busy : '保存全部'}</button>
              <button className="btn primary" disabled={!!busy} onClick={() => run('保存这页', () => exportPng(pageRef.current!, `yyMemoir-${year}-${pad(i + 1)}-${PAGES[i].key}.png`))}>{busy?.startsWith('保存这页') ? busy : '保存这一页'}</button>
            </>
          )}
        </Toolbar>
        <div ref={boxRef} className="card-box">
          {r && r.total_minutes > 0 ? (
            <Canvas ref={pageRef} w={1080} h={1920} scale={scale} className="story" style={tone}>
              <div key={PAGES[i].key} className="page-anim">{PAGES[i].render(r, priv)}</div>
            </Canvas>
          ) : <div className="empty">还没有记录</div>}
        </div>
      </main>
    </>
  )
}

/* ---------------- 设置 ---------------- */
/** 备份：导出 / 导入（合并或覆盖） */
function BackupRows({ onImported }: { onImported: () => void }) {
  const [msg, setMsg] = useState<string | null>(null)
  const [file, setFile] = useState<string | null>(null)
  const [confirmReplace, setConfirmReplace] = useState(false)
  const [busy, setBusy] = useState(false)
  const [original, setOriginal] = useState(false)
  const kb = (b: number) => (b < 1024 * 1024 ? `${Math.max(1, Math.round(b / 1024))} KB` : `${(b / 1024 / 1024).toFixed(1)} MB`)
  const doExport = async () => {
    setBusy(true)
    try {
      setMsg('导出中…')
      const s = await exportBackup(original)
      setMsg(s ? `已导出 ${s.sessions} 次游玩 · ${s.photo_files} 张照片 · ${s.covers} 个封面 · ${kb(s.bytes)}` : null)
    } catch (e) { setMsg(`导出失败：${e}`) }
    setBusy(false)
  }
  const doImport = async (mode: 'merge' | 'replace') => {
    if (!file) return
    setBusy(true)
    try {
      setMsg('导入中…')
      const r = await importBackup(file, mode)
      const img = `还原 ${r.photos_restored} 张照片、${r.covers_restored} 个封面`
      setMsg(mode === 'merge' ? `已合并：新增 ${r.added} 次，跳过 ${r.skipped} 次（本机已有）· ${img}` : `已覆盖：共 ${r.added} 次游玩 · ${img}`)
      setFile(null); setConfirmReplace(false); onImported()
    } catch (e) { setMsg(`导入失败：${e}`) }
    setBusy(false)
  }
  return (
    <>
      <div className="row">
        <span>备份</span>
        <label className="inline" htmlFor="bk-original">
          <input id="bk-original" type="checkbox" checked={original} onChange={(e) => setOriginal(e.target.checked)} />照片用原图
        </label>
        <button className="btn" disabled={busy || !isTauri()} onClick={doExport}>导出</button>
        <button className="btn" disabled={busy || !isTauri()} onClick={async () => { setMsg(null); setConfirmReplace(false); setFile(await pickBackup()) }}>导入</button>
      </div>
      {file && (
        <div className="row sub">
          <code title={file}>{file.split(/[\\/]/).pop()}</code>
          <button className="btn" disabled={busy} onClick={() => doImport('merge')}>合并</button>
          {confirmReplace
            ? <button className="btn danger" disabled={busy} onClick={() => doImport('replace')}>确认覆盖本机记录</button>
            : <button className="btn" disabled={busy} onClick={() => setConfirmReplace(true)}>覆盖</button>}
          <button className="btn ghost" disabled={busy} onClick={() => { setFile(null); setConfirmReplace(false) }}>取消</button>
        </div>
      )}
      {file && <p className="row-note">合并：保留本机记录，加入备份里没有的。覆盖：清空本机记录，换成备份。</p>}
      {msg && <p className="row-note">{msg}</p>}
    </>
  )
}


function SettingsTab({ priv, setPriv, status, onImported }: { priv: Privacy; setPriv: (p: Privacy) => void; status: AppStatus | null; onImported: () => void }) {
  const [auto, setAuto] = useState<boolean | null>(null)
  useEffect(() => { autostart().then((m) => m?.isEnabled().then(setAuto)) }, [])
  const toggleAuto = async (on: boolean) => {
    const m = await autostart()
    if (!m) return
    await (on ? m.enable() : m.disable())
    setAuto(await m.isEnabled())
  }
  return (
    <main className="main settings">
      <Toolbar title="设置" />
      <div className="settings-body">
        <label className="row" htmlFor="autostart">
          <span>开机启动</span>
          <input id="autostart" type="checkbox" checked={!!auto} disabled={!isTauri() || auto === null} onChange={(e) => toggleAuto(e.target.checked)} />
        </label>
        <label className="row" htmlFor="hide-strangers">
          <span>隐藏非好友名字</span>
          <input id="hide-strangers" type="checkbox" checked={priv.hideStrangers} onChange={(e) => setPriv({ ...priv, hideStrangers: e.target.checked })} />
        </label>
        <label className="row" htmlFor="hide-all">
          <span>隐藏所有名字</span>
          <input id="hide-all" type="checkbox" checked={priv.hideAll} onChange={(e) => setPriv({ ...priv, hideAll: e.target.checked })} />
        </label>
        <BackupRows onImported={onImported} />
        {status?.log_dir && (
          <div className="row">
            <span>日志文件夹</span>
            <code title={status.log_dir}>{status.log_dir}</code>
            <em className={status.log_dir_found ? 'ok' : 'bad'}>{status.log_dir_found ? '已找到' : '未找到'}</em>
          </div>
        )}
      </div>
    </main>
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
      onAppEvent('data-updated', () => refresh()),
      onAppEvent('open-session', (id) => { refresh(); setTab('session'); if (id != null) setOpenId(id) }),
    ]
    return () => { offs.forEach((p) => p.then((off) => off())) }
  }, [refresh])

  return (
    <div className={`app ${tab === 'settings' ? 'no-panel' : ''}`}>
      <nav className="rail" aria-label="导航">
        <div className="logo">yyMemoir</div>
        {NAV.map((t) => (
          <button key={t.id} aria-current={tab === t.id ? 'page' : undefined} onClick={() => setTab(t.id)}>{t.label}</button>
        ))}
        <button className="rail-bottom" aria-current={tab === 'settings' ? 'page' : undefined} onClick={() => setTab('settings')}>设置</button>
      </nav>
      <LoggingAlert status={status} />
      {tab === 'session' && <SessionTab priv={priv} openId={openId} refreshKey={refreshKey} />}
      {(tab === 'day' || tab === 'week' || tab === 'month') && <PeriodTab key={tab} kind={tab} priv={priv} refreshKey={refreshKey} />}
      {tab === 'year' && <YearTab priv={priv} />}
      {tab === 'settings' && <SettingsTab priv={priv} setPriv={setPriv} status={status} onImported={refresh} />}
    </div>
  )
}
