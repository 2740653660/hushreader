/**
 * 悬浮阅读窗口入口：把 Tauri 事件 API 桥接给页面内联脚本。
 * - 页面通过 window.__hrEmit 发送命令到主窗口
 * - 主窗口通过 reader-state / reader-notification 事件推送状态与通知
 */
import { emit, listen } from '@tauri-apps/api/event'

declare global {
  interface Window {
    /** 页面内联脚本发送命令到主窗口（event: 'reader-cmd'）。 */
    __hrEmit?: (event: string, payload: unknown) => void
    /** 页面内联脚本接收状态（由主窗口推送）。 */
    hushreaderSetState?: (payload: unknown) => void
    /** 页面内联脚本接收通知（由主窗口推送）。 */
    hushreaderShowNotification?: (message: string) => void
  }
}

window.__hrEmit = (event, payload) => {
  void emit(event, payload)
}

void listen('reader-state', (e) => {
  window.hushreaderSetState?.(e.payload)
})

void listen('reader-notification', (e) => {
  window.hushreaderShowNotification?.(String(e.payload))
})

export {}
