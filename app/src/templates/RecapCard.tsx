import type { Privacy } from '../lib/format'
import type { Recap } from '../types'
import { BentoCard } from './BentoCard'
import { DailyCard } from './DailyCard'
import { ReceiptCard } from './ReceiptCard'

export type CardStyle = 'receipt' | 'bento' | 'ticket'

/**
 * 版式规则（2026-11-22 用户确认）：
 * - 下线推送的「本次回顾」（kind = session）用小票
 * - app 里的今日 / 周 / 月报用便当格
 * - 「行程票根」（DailyCard）保留为备选
 */
export function defaultStyle(r: Recap): CardStyle {
  return r.kind === 'session' ? 'receipt' : 'bento'
}

export function RecapCard({ r, priv, style }: { r: Recap; priv: Privacy; style?: CardStyle }) {
  const s = style ?? defaultStyle(r)
  if (s === 'receipt') return <ReceiptCard r={r} priv={priv} />
  if (s === 'ticket') return <DailyCard r={r} priv={priv} />
  return <BentoCard r={r} priv={priv} />
}
