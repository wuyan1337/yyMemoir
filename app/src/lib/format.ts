import type { PersonStat } from '../types'

export const hours = (m: number) => Math.round(m / 60)
export function duration(m: number) {
  const h = Math.floor(m / 60), r = m % 60
  if (h === 0) return `${r} 分钟`
  return r ? `${h} 小时 ${r} 分` : `${h} 小时`
}
export function cnDate(d: string) {
  const [, mo, da] = d.slice(0, 10).split('-').map(Number)
  return `${mo} 月 ${da} 日`
}
export const clock = (s: string) => s.slice(11, 16)

/** 稳定的字符串哈希，用来给世界/人生成固定的占位配色 */
export function hash(s: string) {
  let h = 2166136261
  for (let i = 0; i < s.length; i++) h = Math.imul(h ^ s.charCodeAt(i), 16777619)
  return h >>> 0
}
/** 没有封面图时的占位封面：按世界 ID 生成固定渐变 */
export function coverStyle(id: string, url?: string | null): React.CSSProperties {
  if (url) return { backgroundImage: `url(${url})`, backgroundSize: 'cover', backgroundPosition: 'center' }
  const h = hash(id) % 360
  return { background: `radial-gradient(circle at 25% 20%, hsl(${(h + 40) % 360} 80% 70% / .9), transparent 55%), linear-gradient(135deg, hsl(${h} 70% 45%), hsl(${(h + 70) % 360} 65% 22%))` }
}

/** hideStrangers 需要好友列表（要登录 VRChat 才有），做登录功能前界面上不提供 */
export interface Privacy { hideStrangers: boolean; hideAll: boolean }
/** 隐私：默认只露出好友的名字和头像 */
export function shown(p: PersonStat, priv: Privacy) {
  const hidden = priv.hideAll || (priv.hideStrangers && !p.is_friend)
  return { name: hidden ? '一位旅伴' : p.name, hidden }
}
