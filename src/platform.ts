/**
 * 平台能力层：把原先由 ZTools 提供的能力统一替换为 Tauri 命令/事件。
 * 前端其余代码只应通过本模块访问底层能力，不再直接触碰 window.ztools / window.services。
 */
import { invoke } from '@tauri-apps/api/core'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'

export interface DialogFilter {
  name: string
  extensions: string[]
}

export interface Bounds {
  x: number
  y: number
  width: number
  height: number
}

function toFilter(filters?: DialogFilter[]) {
  return (filters ?? []).map(f => ({ name: f.name, extensions: f.extensions }))
}

export const platform = {
  /** 读取文本文件（Rust 侧自动识别 UTF-8/GBK）。 */
  readFile: (path: string): Promise<string> => invoke<string>('read_text_file', { path }),

  /** 读取文件原始字节（EPUB/MOBI 解析用）。 */
  readFileBinary: async (path: string): Promise<Uint8Array<ArrayBuffer>> => {
    const arr = await invoke<number[]>('read_file_binary', { path })
    return new Uint8Array(arr)
  },

  /** 文件修改时间（毫秒），失败返回 null。 */
  getFileModifiedTime: (path: string): Promise<number | null> =>
    invoke<number | null>('get_file_modified_time', { path }),

  /** 写入文本文件。 */
  writeFile: (path: string, content: string): Promise<void> =>
    invoke('write_text_file', { path, content }),

  /** 打开文件选择对话框，返回选中路径列表（取消为空数组）。 */
  pickOpenFiles: (opts: {
    title?: string
    filters?: DialogFilter[]
    multiple?: boolean
  }): Promise<string[]> =>
    invoke<string[]>('pick_open_files', {
      title: opts.title ?? null,
      filters: toFilter(opts.filters),
      multiple: opts.multiple ?? false,
    }),

  /** 保存文件对话框，取消返回 null。 */
  pickSaveFile: (opts: {
    title?: string
    defaultName?: string
    filters?: DialogFilter[]
  }): Promise<string | null> =>
    invoke<string | null>('pick_save_file', {
      title: opts.title ?? null,
      default_name: opts.defaultName ?? null,
      filters: toFilter(opts.filters),
    }),

  /** 在资源管理器中显示文件位置。 */
  revealInFolder: (path: string): Promise<void> => invoke('reveal_in_folder', { path }),

  /** 显示主窗口。 */
  showMainWindow: (): Promise<void> => invoke('show_main_window'),

  /** 主显示器工作区（逻辑像素）。 */
  getWorkArea: (): Promise<Bounds> => invoke<Bounds>('get_work_area'),

  // ---------- 悬浮阅读窗口 ----------

  readerShow: (bounds?: Bounds): Promise<void> => invoke('reader_show', { bounds: bounds ?? null }),
  readerHide: (): Promise<void> => invoke('reader_hide'),
  readerClose: (): Promise<void> => invoke('reader_close'),
  readerResize: (width: number, height: number): Promise<void> =>
    invoke('reader_resize', { width, height }),
  readerMove: (x: number, y: number): Promise<void> => invoke('reader_move', { x, y }),
  readerPosition: (): Promise<Bounds> => invoke<Bounds>('reader_position'),
  readerFocus: (): Promise<void> => invoke('reader_focus'),

  // ---------- 开机启动 ----------

  setAutostart: (enabled: boolean): Promise<void> => invoke('set_autostart', { enabled }),
  getAutostart: (): Promise<boolean> => invoke('get_autostart'),

  // ---------- 事件 ----------

  /** 订阅主窗口命令（来自悬浮窗与全局快捷键）。 */
  onMainCommand: (cb: (command: unknown) => void): Promise<UnlistenFn> =>
    listen('main-cmd', e => cb(e.payload)),

  /** 订阅悬浮窗命令。 */
  onReaderCommand: (cb: (command: unknown) => void): Promise<UnlistenFn> =>
    listen('reader-cmd', e => cb(e.payload)),

  /** 向悬浮窗推送状态。 */
  pushReaderState: (payload: unknown): Promise<void> =>
    import('@tauri-apps/api/event').then(({ emit }) => emit('reader-state', payload)),

  /** 向悬浮窗推送通知。 */
  pushReaderNotification: (message: string): Promise<void> =>
    import('@tauri-apps/api/event').then(({ emit }) => emit('reader-notification', message)),
}

/** 悬浮窗页面专用：发送命令到主窗口。 */
export const readerCommand = (command: unknown): void => {
  void import('@tauri-apps/api/event').then(({ emit }) => emit('reader-cmd', command))
}
