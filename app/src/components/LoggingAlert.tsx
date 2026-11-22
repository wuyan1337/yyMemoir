import { useState } from 'react'
import type { AppStatus } from '../lib/data'

/** VRChat 日志是「仅错误」时，顶部一行提醒 */
export function LoggingAlert({ status }: { status: AppStatus | null }) {
  const [hidden, setHidden] = useState(false)
  if (!status || hidden || !status.latest_errors_only) return null
  return (
    <div className="alert" role="alert">
      <span>日志设置为「仅错误」，没有记录到游玩</span>
      <span className="path">游戏内 设置 → Debug → 日志 → 完整</span>
      <button className="alert-close" onClick={() => setHidden(true)} aria-label="关闭">×</button>
    </div>
  )
}
