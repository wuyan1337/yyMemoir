import { useEffect, useRef, useState } from 'react'
import dayJson from './demo/day.json'
import yearJson from './demo/year.json'
import { exportPng } from './lib/exportPng'
import type { Privacy } from './lib/format'
import { DailyCard } from './templates/DailyCard'
import { Canvas } from './templates/parts'
import { StoryPlayer } from './templates/story/StoryPlayer'
import { PAGES } from './templates/story/pages'
import type { Recap } from './types'

// 演示数据：由 scripts/gen_fake_logs.py 生成假日志 → yymemoir-cli recap 算出来的真实 Recap JSON
const DEMO_DAY = dayJson as Recap
const DEMO_YEAR = yearJson as Recap

type Tab = 'day' | 'year'

function useFitScale(ref: React.RefObject<HTMLElement | null>, w: number, h: number) {
  const [s, setS] = useState(0.3)
  useEffect(() => {
    const fit = () => {
      const el = ref.current
      if (!el) return
      const availW = el.clientWidth
      const availH = window.innerHeight - el.getBoundingClientRect().top - 90
      setS(Math.max(0.15, Math.min(availW / w, availH / h, 1)))
    }
    fit()
    window.addEventListener('resize', fit)
    return () => window.removeEventListener('resize', fit)
  }, [ref, w, h])
  return s
}

/** 渲染模式：?render=day 或 ?render=year&page=N，只输出 1:1 画布，给 scripts/render.mjs 截图用 */
function RenderOnly({ q }: { q: URLSearchParams }) {
  const priv: Privacy = { hideStrangers: q.get('hide') !== '0', hideAll: q.get('hideAll') === '1' }
  if (q.get('render') === 'day') {
    return <div id="shot" style={{ width: 1080, height: 1440, position: 'relative', overflow: 'hidden' }}><DailyCard r={DEMO_DAY} priv={priv} /></div>
  }
  const page = PAGES[Number(q.get('page') ?? 0)]
  return (
    <div id="shot" className={`canvas story ${page.theme}`} style={{ width: 1080, height: 1920 }}>
      {page.render(DEMO_YEAR, priv)}
    </div>
  )
}

export default function App() {
  const q = new URLSearchParams(location.search)
  if (q.has('render')) return <RenderOnly q={q} />
  return <Shell />
}

function Shell() {
  const [tab, setTab] = useState<Tab>('year')
  const [priv, setPriv] = useState<Privacy>({ hideStrangers: true, hideAll: false })
  const [busy, setBusy] = useState(false)
  const stageRef = useRef<HTMLDivElement>(null)
  const cardRef = useRef<HTMLDivElement>(null)
  const scale = useFitScale(stageRef, 1080, tab === 'day' ? 1440 : 1920)

  return (
    <div className="app">
      <header className="topbar">
        <div className="logo"><span className="brand-mark">◐</span> yyMemoir</div>
        <nav className="tabs" role="tablist">
          <button role="tab" aria-selected={tab === 'day'} onClick={() => setTab('day')}>今日回顾</button>
          <button role="tab" aria-selected={tab === 'year'} onClick={() => setTab('year')}>年度报告</button>
        </nav>
        <div className="privacy">
          <label><input type="checkbox" checked={priv.hideStrangers} onChange={(e) => setPriv({ ...priv, hideStrangers: e.target.checked })} /> 隐藏非好友</label>
          <label><input type="checkbox" checked={priv.hideAll} onChange={(e) => setPriv({ ...priv, hideAll: e.target.checked })} /> 隐藏所有人</label>
        </div>
      </header>
      <main ref={stageRef} className="stage">
        {tab === 'day' ? (
          <div className="player">
            <Canvas ref={cardRef} w={1080} h={1440} scale={scale}>
              <DailyCard r={DEMO_DAY} priv={priv} />
            </Canvas>
            <div className="player-bar">
              <span>{DEMO_DAY.from.slice(0, 10)} · 今日卡片</span>
              <button disabled={busy} onClick={async () => {
                if (!cardRef.current) return
                setBusy(true)
                await exportPng(cardRef.current, `yyMemoir-${DEMO_DAY.from.slice(0, 10)}.png`)
                setBusy(false)
              }}>保存图片</button>
            </div>
          </div>
        ) : (
          <StoryPlayer r={DEMO_YEAR} priv={priv} scale={scale} />
        )}
      </main>
    </div>
  )
}
