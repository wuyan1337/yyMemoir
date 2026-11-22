import { useEffect, useState } from 'react'
import dayJson from './demo/day.json'
import yearJson from './demo/year.json'
import type { Privacy } from './lib/format'
import { RecapCard, type CardStyle } from './templates/RecapCard'
import { Shell } from './Shell'
import { PAGES } from './templates/story/pages'
import type { Recap } from './types'

// 演示数据：由 scripts/gen_fake_logs.py 生成假日志 → yymemoir-cli recap 算出来的真实 Recap JSON
const DEMO_DAY = dayJson as Recap
const DEMO_YEAR = yearJson as Recap

/** 有真实数据（public/real/day.json，不入库）就用真实的今日数据，否则用演示数据 */
function useDay() {
  const [day, setDay] = useState<Recap | null>(null)
  useEffect(() => {
    if (new URLSearchParams(location.search).has('demo')) { setDay(DEMO_DAY); return }
    const src = new URLSearchParams(location.search).get('data') ?? '/real/day.json'
    fetch(src).then((r) => (r.ok ? r.json() : null)).catch(() => null).then((d) => setDay(d ?? DEMO_DAY))
  }, [])
  return day
}

/** 渲染模式：?render=day 或 ?render=year&page=N，只输出 1:1 画布，给 scripts/render.mjs 截图用 */
function RenderOnly({ q }: { q: URLSearchParams }) {
  const priv: Privacy = { hideStrangers: q.get('hide') === '1', hideAll: q.get('hideAll') === '1' }
  const day = useDay()
  if (q.get('render') === 'day') {
    if (!day) return null
    const style = (q.get('style') ?? undefined) as CardStyle | undefined
    return <div id="shot" style={{ width: 1080, height: 1440, position: 'relative', overflow: 'hidden' }}><RecapCard r={day} priv={priv} style={style} /></div>
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
