import { useState } from 'react'
import type { AppStatus } from '../lib/data'

/** 后台检测到 VRChat 日志设成了「仅错误」时，在 app 顶部提醒用户去改。 */
export function LoggingAlert({ status }: { status: AppStatus | null }) {
  const [hidden, setHidden] = useState(false)
  if (!status || hidden || !status.latest_errors_only) return null
  return (
    <div className="alert" role="alert">
      <div className="alert-body">
        <b>最近的游玩没有被记录下来</b>
        <p>
          VRChat 的日志设置是「仅错误」，游戏不会写下你去了哪些世界、和谁在一起。
          请进游戏打开 <span className="path">设置 → Debug → 日志（Logging）</span>，改成 <span className="path">完整（Full）</span>，下次上线就能正常记录。
        </p>
        {status.errors_only_logs > 1 && <p className="alert-sub">已经有 {status.errors_only_logs} 次游玩受影响，这几次的数据补不回来。</p>}
      </div>
      <button className="alert-close" onClick={() => setHidden(true)} aria-label="知道了">知道了</button>
    </div>
  )
}
