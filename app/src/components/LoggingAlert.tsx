import { useEffect, useState } from 'react'

/** 后台检测到 VRChat 日志设成了「仅错误」时，在 app 顶部提醒用户去改。
 *  数据来自 `yymemoir-cli status`（以后换成 Tauri 命令）。 */
export interface AppStatus { errors_only_logs: number; latest_errors_only: boolean }

export function useStatus() {
  const [st, setSt] = useState<AppStatus | null>(null)
  useEffect(() => {
    fetch('/real/status.json').then((r) => (r.ok ? r.json() : null)).catch(() => null).then(setSt)
  }, [])
  return st
}

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
