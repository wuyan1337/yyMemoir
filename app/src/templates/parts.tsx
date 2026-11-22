import { forwardRef, type ReactNode } from 'react'
import { coverStyle, hash, shown, type Privacy } from '../lib/format'
import type { PersonStat } from '../types'

/** 固定像素尺寸的画布，预览时按容器宽度缩放；导出时用原尺寸 */
export const Canvas = forwardRef<HTMLDivElement, { w: number; h: number; scale: number; children: ReactNode; className?: string }>(
  function Canvas({ w, h, scale, children, className }, ref) {
    return (
      <div className="canvas-frame" style={{ width: w * scale, height: h * scale }}>
        <div ref={ref} className={`canvas ${className ?? ''}`} style={{ width: w, height: h, transform: `scale(${scale})` }}>
          {children}
        </div>
      </div>
    )
  },
)

export function Cover({ id, url, className, children }: { id: string; url?: string | null; className?: string; children?: ReactNode }) {
  return <div className={`cover ${className ?? ''}`} style={coverStyle(id, url)}>{children}</div>
}

export function Avatar({ p, priv, size }: { p: PersonStat; priv: Privacy; size: number }) {
  const { name, hidden } = shown(p, priv)
  const hue = hash(p.user_id) % 360
  const style: React.CSSProperties = p.avatar && !hidden
    ? { backgroundImage: `url(${p.avatar})`, backgroundSize: 'cover' }
    : { background: hidden ? 'linear-gradient(135deg,#4a4470,#2a2547)' : `linear-gradient(135deg, hsl(${hue} 75% 65%), hsl(${(hue + 50) % 360} 70% 40%))` }
  return (
    <div className="avatar" style={{ ...style, width: size, height: size, fontSize: size * 0.4 }}>
      {!p.avatar || hidden ? (hidden ? '?' : [...name][0]) : null}
    </div>
  )
}

export function Brand({ sub }: { sub?: string }) {
  return (
    <div className="brand">
      <span className="brand-mark">◐</span> yyMemoir{sub ? <span className="brand-sub"> · {sub}</span> : null}
    </div>
  )
}

/** 单系列竖条图：一个色相，4px 圆角顶端，条间 2px 以上间隔，直接标注峰值 */
export function Bars({ values, labels, highlight, height, labelEvery = 1 }: { values: number[]; labels: string[]; highlight?: number; height: number; labelEvery?: number }) {
  const max = Math.max(1, ...values)
  return (
    <div className="bars" style={{ height }}>
      {values.map((v, i) => (
        <div key={i} className="bar-col" title={`${labels[i]}：${Math.round(v / 60)} 小时`}>
          <div className="bar-track">
            <div className={`bar ${i === highlight ? 'hl' : ''}`} style={{ height: `${(v / max) * 100}%` }}>
              {i === highlight ? <span className="bar-val">{Math.round(v / 60)}h</span> : null}
            </div>
          </div>
          <div className="bar-label">{i % labelEvery === 0 ? labels[i] : ''}</div>
        </div>
      ))}
    </div>
  )
}
