/**
 * 找书 / 下载 store：负责多书源搜索、下载任务状态与进度、
 * 下载完成后自动把新文件加入书架（复用公共导入）。
 */
import { defineStore } from 'pinia'
import { ref } from 'vue'
import { platform } from '../platform'
import { importBookFromFile } from '../utils/importer'

export interface SearchResult {
  sourceId: number
  sourceName?: string
  url: string
  bookName?: string
  author?: string
  intro?: string
  category?: string
  latestChapter?: string
  lastUpdateTime?: string
  status?: string
  wordCount?: string
}

export interface LocalBookItem {
  name: string
  size: number
  timestamp: number
}

export type DownloadState = 'idle' | 'downloading' | 'success' | 'error' | 'aborted'

export interface DownloadTask {
  state: DownloadState
  bookName: string
  sourceName: string
  url: string
  sourceId: number
  format: 'epub' | 'txt'
  total: number
  index: number
  percent: number
  error?: string
}

const SUPPORTED_EXT = /\.(epub|txt)$/i

export const useDownloadStore = defineStore('download', () => {
  // ---- 搜索 ----
  const searching = ref(false)
  const searchError = ref('')
  const results = ref<SearchResult[]>([])
  const lastKeyword = ref('')

  // ---- 后台状态 ----
  const backendRunning = ref(false)
  const backendError = ref('')

  // ---- 下载任务 ----
  const task = ref<DownloadTask | null>(null)

  /** 下载开始前的书库文件快照，用于识别新落盘的文件。 */
  let filesBefore: string[] = []

  async function refreshBackendStatus() {
    try {
      const st = await platform.backendStatus()
      backendRunning.value = st.running
      backendError.value = st.running ? '' : '下载后台未就绪'
    } catch (e: any) {
      backendRunning.value = false
      backendError.value = `无法获取下载后台状态：${e}`
    }
  }

  async function startBackend() {
    backendError.value = ''
    try {
      const st = await platform.backendStart()
      backendRunning.value = st.running
      backendError.value = st.running ? '' : '下载后台启动失败'
    } catch (e: any) {
      backendRunning.value = false
      backendError.value = `下载后台启动失败：${e}`
    }
  }

  async function search(keyword: string) {
    const kw = keyword.trim()
    if (!kw) return
    searching.value = true
    searchError.value = ''
    results.value = []
    lastKeyword.value = kw
    try {
      const data = (await platform.sonovelSearch(kw, 30)) as SearchResult[]
      results.value = Array.isArray(data) ? data : []
      if (results.value.length === 0) {
        searchError.value = '没有找到匹配的书籍，换个关键词试试'
      }
    } catch (e: any) {
      searchError.value = `搜索失败：${e}`
    } finally {
      searching.value = false
    }
  }

  /** 下载前记录书库文件快照。 */
  async function snapshotFiles(): Promise<string[]> {
    try {
      const items = (await platform.sonovelLocalBooks()) as LocalBookItem[]
      return Array.isArray(items) ? items.map(i => i.name) : []
    } catch {
      return []
    }
  }

  /** 下载完成后找出新落盘的文件名（排除正在解析的中间状态）。 */
  async function findNewFile(): Promise<string | null> {
    try {
      const items = (await platform.sonovelLocalBooks()) as LocalBookItem[]
      if (!Array.isArray(items)) return null
      const now = new Set(items.map(i => i.name))
      for (const name of now) {
        if (!filesBefore.includes(name) && SUPPORTED_EXT.test(name)) {
          return name
        }
      }
      return null
    } catch {
      return null
    }
  }

  /** 发起下载（一次一本）。 */
  async function startDownload(result: SearchResult, format: 'epub' | 'txt'): Promise<boolean> {
    if (task.value && task.value.state === 'downloading') {
      return false
    }
    if (!backendRunning.value) {
      backendError.value = '下载后台未就绪'
      return false
    }
    filesBefore = await snapshotFiles()
    task.value = {
      state: 'downloading',
      bookName: result.bookName || '未知书名',
      sourceName: result.sourceName || '未知书源',
      url: result.url,
      sourceId: result.sourceId,
      format,
      total: 0,
      index: 0,
      percent: 0
    }
    try {
      await platform.sonovelFetch(result.url, result.sourceId, format)
      return true
    } catch (e: any) {
      task.value.state = 'error'
      task.value.error = String(e)
      return false
    }
  }

  /** 用户放弃当前下载：后台会继续下完但不再自动入架。 */
  function abortDownload() {
    if (task.value && task.value.state === 'downloading') {
      task.value.state = 'aborted'
    }
  }

  function resetTask() {
    task.value = null
  }

  async function handleProgress(p: { total: number; index: number; percent: number }) {
    const t = task.value
    if (!t || t.state !== 'downloading') return
    t.total = p.total
    t.index = p.index
    t.percent = p.percent
  }

  async function handleFetchDone(p: { ok: boolean; message: string }) {
    const t = task.value
    if (!t) return
    if (t.state === 'aborted') return // 已放弃：不自动入架
    if (p.ok) {
      const fileName = await findNewFile()
      if (fileName) {
        // 路径 = 书库目录 + 文件名；书库目录由 Rust 侧管理，
        // 通过 getBookshelfDir 获得后拼接（本地导入同样按该目录定位）。
        try {
          const dir = await platform.getBookshelfDir()
          const filePath = `${dir.replace(/[\\/]+$/, '')}\\${fileName}`
          const result = await importBookFromFile(filePath)
          t.state = 'success'
          t.error = result.ok
            ? undefined
            : result.message
        } catch (e: any) {
          t.state = 'error'
          t.error = `导入书架失败：${e}`
        }
      } else {
        t.state = 'success'
        t.error = '下载完成，但未找到新文件（可能文件名与已有文件相同）'
      }
    } else {
      t.state = 'error'
      t.error = p.message || '下载失败'
    }
  }

  // ---- 事件订阅（由 App.vue 启动时调用） ----
  let unlistenProgress: (() => void) | undefined
  let unlistenDone: (() => void) | undefined

  async function init() {
    unlistenProgress = await platform.onSonovelProgress(handleProgress).catch(() => undefined)
    unlistenDone = await platform.onFetchDone(handleFetchDone).catch(() => undefined)
    await refreshBackendStatus()
  }

  function dispose() {
    unlistenProgress?.()
    unlistenDone?.()
    unlistenProgress = undefined
    unlistenDone = undefined
  }

  return {
    searching, searchError, results, lastKeyword,
    backendRunning, backendError,
    task,
    refreshBackendStatus, startBackend, search,
    startDownload, abortDownload, resetTask,
    init, dispose
  }
})
