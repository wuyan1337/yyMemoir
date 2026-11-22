import { useEffect, useRef, useState } from 'react'
import { exportPng } from '../../lib/exportPng'
import type { Privacy } from '../../lib/format'
import type { Recap } from '../../types'
import { Canvas } from '../parts'
import { PAGES } from './pages'
import { useCardTone } from '../DailyCard'

/**
 * 快拍式播放器（交互参考 minagishl/github-wrapped 的 WrappedPlayer，MIT）：
 * 顶部分段进度条，点左/右半边或方向键切页，空格自动播放。
 */
export function StoryPlayer({ r, priv, scale }: { r: Recap; priv: Privacy; scale: number }) {
  const [i, setI] = useState(0)
  const [auto, setAuto] = useState(false)
  const [busy, setBusy] = useState(false)
  const ref = useRef<HTMLDivElement>(null)
  const n = PAGES.length
  const go = (d: number) => setI((x) => Math.min(n - 1, Math.max(0, x + d)))

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === 'ArrowRight') go(1)
      else if (e.key === 'ArrowLeft') go(-1)
      else if (e.key === ' ') { e.preventDefault(); setAuto((a) => !a) }
    }
    window.addEventListener('keydown', onKey)
    return () => window.removeEventListener('keydown', onKey)
  }, [])
  useEffect(() => {
    if (!auto) return
    if (i >= n - 1) { setAuto(false); return }
    const t = setTimeout(() => go(1), 4000)
    return () => clearTimeout(t)
  }, [auto, i])

  const page = PAGES[i]
  // 整套年报的色调从全年待得最久的世界封面取
  const tone = useCardTone(r)
  const exportOne = async () => {
    if (!ref.current) return
    setBusy(true)
    await exportPng(ref.current, `yyMemoir-${r.from.slice(0, 4)}-${String(i + 1).padStart(2, '0')}-${page.key}.png`)
    setBusy(false)
  }
  const exportAll = async () => {
    setBusy(true)
    for (let k = 0; k < n; k++) {
      setI(k)
      await new Promise((res) => setTimeout(res, 350))
      if (ref.current) await exportPng(ref.current, `yyMemoir-${r.from.slice(0, 4)}-${String(k + 1).padStart(2, '0')}-${PAGES[k].key}.png`)
    }
    setBusy(false)
  }

  return (
    <div className="player">
      <div className="player-stage">
        <Canvas ref={ref} w={1080} h={1920} scale={scale} className="story" style={tone}>
          <div className="progress no-export">
            {PAGES.map((p, k) => <div key={p.key} className={k < i ? 'done' : k === i ? 'now' : ''} />)}
          </div>
          <div key={page.key} className="page-anim">{page.render(r, priv)}</div>
        </Canvas>
        <button className="tap left" aria-label="上一页" onClick={() => go(-1)} />
        <button className="tap right" aria-label="下一页" onClick={() => go(1)} />
      </div>
      <div className="player-bar">
        <span>{i + 1} / {n} · {page.title}</span>
        <button onClick={() => setAuto((a) => !a)}>{auto ? '暂停' : '自动播放'}</button>
        <button onClick={exportOne} disabled={busy}>保存这一页</button>
        <button onClick={exportAll} disabled={busy}>保存全部 {n} 页</button>
      </div>
    </div>
  )
}
