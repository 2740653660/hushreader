<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, provide, ref, watch } from 'vue'
import Bookshelf from './components/Bookshelf/index.vue'
import FindBook from './components/FindBook/index.vue'
import Toast from './components/Bookshelf/Toast.vue'
import { useReaderStore } from './stores/reader'
import { useBookStore, type Bookmark } from './stores/books'
import { useConfigStore } from './stores/config'
import { useDownloadStore } from './stores/download'
import { parseTxt } from './utils/txtParser'
import { parseEpub } from './utils/epubParser'
import { parseMobi } from './utils/mobiParser'
import { loadChapters, saveChapters } from './utils/db'
import { platform, type Bounds } from './platform'

type HushreaderCommand = string | { type?: string; width?: number; height?: number; x?: number; y?: number; percent?: number }

const FISH_MIN_WIDTH = 280
const FISH_MAX_WIDTH = 1180
const FISH_MIN_HEIGHT = 22
const FISH_MAX_HEIGHT = 500
const FISH_META_ROW_HEIGHT = 18
const FISH_SIDE_CONTROLS_WIDTH = 44
const FISH_CONTENT_PADDING = 20

const route = ref('bookshelf')
const enterAction = ref<any>({})

const readerStore = useReaderStore()
const bookStore = useBookStore()
const configStore = useConfigStore()
const downloadStore = useDownloadStore()

/** 页面导航（书架 ⇄ 找书），供子组件注入使用。 */
function navigate(page: 'bookshelf' | 'findbook') {
  route.value = page
}

const isReaderHidden = ref(false)
const isAutoPaging = ref(false)
const isHushreaderKeyboardActive = ref(false)
const hushreaderActivated = ref(false)
const toastMsg = ref('')
const toastType = ref<'info' | 'error' | 'success'>('info')
let toastTimer = 0
let autoTimer = 0
let readingTimer = 0
let readingTimerStart = 0
let readingTimerWarning = 0
let isAutoPageTickRunning = false

let offReaderCommand: (() => void) | undefined
let offMainCommand: (() => void) | undefined

const cfg = computed(() => configStore.config)
const hushreaderCfg = computed(() => cfg.value.hushreader)

const currentBook = computed(() => {
  const id = bookStore.currentBookId
  return id ? bookStore.books.find(b => b.id === id) ?? null : null
})

/** 正在阅读的书籍 id：悬浮窗激活且未隐藏时才有值，供书架显示"正在读"角标与关闭悬浮窗入口。 */
const readerActiveBookId = computed(() =>
  hushreaderActivated.value && !isReaderHidden.value && bookStore.currentBookId
    ? bookStore.currentBookId
    : null
)

const currentChapter = computed(() =>
  readerStore.chapters[readerStore.currentChapterIndex] ?? null
)

const hushreaderLines = computed(() => readerStore.hushreaderLines)

const progressLabel = computed(() => {
  if (hushreaderCfg.value.progressMode === 'percent') {
    return `${readerStore.readingPercent.toFixed(2)}%`
  }
  const total = readerStore.chapters.length
  const ci = readerStore.currentChapterIndex
  return total > 0 ? `${ci + 1}/${total}` : ''
})

const activeBookLabel = computed(() => currentBook.value?.title ?? '')

function clampNumber(value: number, min: number, max: number) {
  const safeMin = Number.isFinite(min) ? min : 0
  const safeMax = Number.isFinite(max) ? Math.max(safeMin, max) : safeMin
  const safeValue = Number.isFinite(value) ? value : safeMin
  return Math.min(safeMax, Math.max(safeMin, Math.round(safeValue)))
}

/** 主显示器工作区缓存（Tauri 命令异步获取；启动时初始化）。 */
let workAreaCache: Bounds | null = null

function initWorkArea() {
  platform.getWorkArea().then(a => { workAreaCache = a }).catch(() => { })
}

function getWorkArea(): Bounds {
  return workAreaCache ?? { x: 0, y: 0, width: window.screen.availWidth, height: window.screen.availHeight }
}

function getHushreaderSizeLimits() {
  const area = getWorkArea()
  return {
    minWidth: FISH_MIN_WIDTH,
    maxWidth: Math.max(FISH_MIN_WIDTH, Math.min(area.width, FISH_MAX_WIDTH)),
    minHeight: FISH_MIN_HEIGHT,
    maxHeight: Math.max(FISH_MIN_HEIGHT, Math.min(area.height, FISH_MAX_HEIGHT))
  }
}

function clampHushreaderSize(width: number, height: number) {
  const limits = getHushreaderSizeLimits()
  return {
    width: clampNumber(width, limits.minWidth, limits.maxWidth),
    height: clampNumber(height, limits.minHeight, limits.maxHeight)
  }
}

function getInitialHushreaderWindowBoundsForSize(width: number, height: number) {
  const area = getWorkArea()
  const size = clampHushreaderSize(width, height)
  return {
    ...size,
    x: Math.round(area.x + (area.width - size.width) / 2),
    y: Math.round(area.y + area.height - size.height)
  }
}

function getStoredHushreaderAnchor() {
  const fc = hushreaderCfg.value
  if (Number.isFinite(fc.hushreaderX) && Number.isFinite(fc.hushreaderY) && (fc.hushreaderX !== 0 || fc.hushreaderY !== 0)) {
    return { x: fc.hushreaderX, y: fc.hushreaderY }
  }
  return null
}

function getAnchoredHushreaderWindowBoundsForSize(width: number, height: number) {
  const area = getWorkArea()
  const size = clampHushreaderSize(width, height)
  const fallback = getStoredHushreaderAnchor() ?? getInitialHushreaderWindowBoundsForSize(size.width, size.height)
  const maxX = area.x + Math.max(0, area.width - size.width)
  const maxY = area.y + Math.max(0, area.height - size.height)
  return {
    ...size,
    x: clampNumber(fallback.x, area.x, maxX),
    y: clampNumber(fallback.y, area.y, maxY)
  }
}

function getMovedHushreaderWindowBounds(x: number, y: number) {
  const area = getWorkArea()
  const size = clampHushreaderSize(hushreaderCfg.value.hushreaderWidth, hushreaderCfg.value.hushreaderHeight)
  const maxX = area.x + Math.max(0, area.width - size.width)
  const maxY = area.y + Math.max(0, area.height - size.height)
  return {
    ...size,
    x: clampNumber(x, area.x, maxX),
    y: clampNumber(y, area.y, maxY)
  }
}

function getHushreaderWindowBounds() {
  return getAnchoredHushreaderWindowBoundsForSize(hushreaderCfg.value.hushreaderWidth, hushreaderCfg.value.hushreaderHeight)
}

function getHushreaderLineLength(): number {
  const readableWidth = Math.max(24, hushreaderCfg.value.hushreaderWidth - FISH_SIDE_CONTROLS_WIDTH - FISH_CONTENT_PADDING)
  const cjkCharWidth = Math.max(1, hushreaderCfg.value.fontSize + hushreaderCfg.value.letterSpacing)
  return Math.max(1, Math.floor(readableWidth / cjkCharWidth))
}

function getHushreaderLineCount(): number {
  const linePx = hushreaderCfg.value.fontSize * hushreaderCfg.value.lineHeight
  const reservedHeight = hushreaderCfg.value.showHushreaderMeta ? FISH_META_ROW_HEIGHT : 0
  const readableHeight = Math.max(18, hushreaderCfg.value.hushreaderHeight - 6 - reservedHeight)
  return Math.max(1, Math.floor(readableHeight / Math.max(12, linePx)))
}

function updateHushreaderLayout() {
  readerStore.setHushreaderLayout(getHushreaderLineLength(), getHushreaderLineCount())
}

function getHushreaderPayload(bounds = getHushreaderWindowBounds()) {
  return {
    visible: hushreaderActivated.value && Boolean(currentBook.value) && readerStore.chapters.length > 0 && !isReaderHidden.value,
    title: activeBookLabel.value,
    chapter: currentChapter.value?.title ?? '',
    progress: progressLabel.value,
    readingPercent: readerStore.readingPercent,
    currentPage: readerStore.currentPage + 1,
    pageCount: readerStore.totalPages,
    lines: hushreaderLines.value,
    bounds,
    resizeLimits: getHushreaderSizeLimits(),
    settings: {
      fontSize: hushreaderCfg.value.fontSize,
      lineHeight: hushreaderCfg.value.lineHeight,
      hushreaderWidth: hushreaderCfg.value.hushreaderWidth,
      hushreaderHeight: hushreaderCfg.value.hushreaderHeight,
      hushreaderX: hushreaderCfg.value.hushreaderX,
      hushreaderY: hushreaderCfg.value.hushreaderY,
      letterSpacing: hushreaderCfg.value.letterSpacing,
      opacity: hushreaderCfg.value.opacity,
      bgOpacity: hushreaderCfg.value.bgOpacity,
      prevPageKey: hushreaderCfg.value.prevPageKey,
      nextPageKey: hushreaderCfg.value.nextPageKey,
      addBookmarkKey: hushreaderCfg.value.addBookmarkKey,
      destroyKey: hushreaderCfg.value.destroyKey,
      showHushreaderMeta: hushreaderCfg.value.showHushreaderMeta,
      progressMode: hushreaderCfg.value.progressMode,
      hideOnMouseLeave: hushreaderCfg.value.hideOnMouseLeave,
      mouseEnterDelay: hushreaderCfg.value.mouseEnterDelay,
      wheelTurnPage: hushreaderCfg.value.wheelTurnPage,
      bgColor: hushreaderCfg.value.bgColor,
      textColor: hushreaderCfg.value.textColor,
      autoFlipEnabled: hushreaderCfg.value.autoFlipEnabled,
      fontFamily: hushreaderCfg.value.fontFamily,
      windowMovable: cfg.value.function.windowMovable,
      windowSizeLocked: cfg.value.function.windowSizeLocked,
      timerEnabled: cfg.value.other.timerEnabled,
      timerRemaining: getReadingTimerRemaining()
    }
  }
}

function pushHushreaderState(options?: { skipShow?: boolean }) {
  const bounds = getHushreaderWindowBounds()
  const payload = getHushreaderPayload(bounds)
  if (payload.visible) {
    if (!options?.skipShow) {
      void platform.readerShow(bounds)
    }
  } else {
    void platform.readerHide()
  }
  void platform.pushReaderState(payload)
}

let pendingNotificationCallback: (() => void) | null = null

function pushHushreaderNotification(message: string, onClose?: () => void) {
  pendingNotificationCallback = onClose ?? null
  void platform.pushReaderNotification(message)
}

function showHushreaderWindow() {
  if (!hushreaderActivated.value) return
  isReaderHidden.value = false
  pushHushreaderState()
}

function hideHushreaderWindow() {
  isReaderHidden.value = true
  blurHushreaderKeyboard()
  stopReadingTimer()
}

function focusHushreaderKeyboard() {
  isHushreaderKeyboardActive.value = true
  void platform.readerFocus()
}

function blurHushreaderKeyboard() {
  isHushreaderKeyboardActive.value = false
}

function toggleHidden() {
  isReaderHidden.value = !isReaderHidden.value
  if (isReaderHidden.value) blurHushreaderKeyboard()
}

function toggleAutoPaging() {
  if (!currentBook.value) return
  isAutoPaging.value = !isAutoPaging.value
  hushreaderCfg.value.autoFlipEnabled = isAutoPaging.value
}

/** 关闭阅读器（命令 'close'），并隐藏悬浮窗。 */
function closePlugin() {
  isReaderHidden.value = true
  hushreaderActivated.value = false
  stopReadingTimer()
  void platform.readerClose()
}

function toast(msg: string, type: 'info' | 'error' | 'success' = 'info') {
  toastMsg.value = msg
  toastType.value = type
  clearTimeout(toastTimer)
  toastTimer = window.setTimeout(() => { toastMsg.value = '' }, 3000)
}

function startReadingTimer() {
  stopReadingTimer()
  if (!cfg.value.other.timerEnabled || !hushreaderActivated.value) return
  const minutes = Number(cfg.value.other.timerMinutes)
  if (isNaN(minutes) || minutes <= 0) return
  readingTimerStart = Date.now()
  const ms = minutes * 60 * 1000
  const warningMs = Math.round(ms * 0.9)
  readingTimerWarning = window.setTimeout(() => {
    const elapsed = Math.round((Date.now() - readingTimerStart) / 1000)
    const remaining = Math.round((ms - (Date.now() - readingTimerStart)) / 1000)
    const elapsedMin = Math.floor(elapsed / 60)
    const elapsedSec = elapsed % 60
    const remainingMin = Math.floor(remaining / 60)
    const remainingSec = remaining % 60
    const elapsedStr = elapsedMin > 0 ? `${elapsedMin}分${elapsedSec}秒` : `${elapsedSec}秒`
    const remainingStr = remainingMin > 0 ? `${remainingMin}分${remainingSec}秒` : `${remainingSec}秒`
    pushHushreaderNotification(`已阅读${elapsedStr}，${remainingStr}后将自动关闭`)
  }, warningMs)
  readingTimer = window.setTimeout(() => {
    const elapsedMin = minutes
    pushHushreaderNotification(`阅读定时器已到 ${elapsedMin} 分钟，即将关闭`, () => {
      hideHushreaderWindow()
    })
    readingTimerStart = 0
  }, ms)
}

function stopReadingTimer() {
  clearTimeout(readingTimer)
  clearTimeout(readingTimerWarning)
  readingTimer = 0
  readingTimerWarning = 0
  readingTimerStart = 0
}

function getReadingTimerRemaining(): number | null {
  if (!readingTimerStart || !cfg.value.other.timerEnabled) return null
  const elapsed = Date.now() - readingTimerStart
  const total = cfg.value.other.timerMinutes * 60 * 1000
  return Math.max(0, total - elapsed)
}

function saveReadingProgress() {
  const book = bookStore.currentBook
  if (!book) return
  const now = Date.now()
  const updates: Partial<typeof book> = {
    lastChapter: readerStore.currentChapterIndex,
    progressIndex: readerStore.progressIndex,
    lastReadAt: now,
    totalChapters: readerStore.chapters.length,
    readingPercent: readerStore.readingPercent
  }
  if (!book.firstReadAt) {
    updates.firstReadAt = now
  }

  // 初始化或重置会话计时器，避免将跨会话的闲置时间计入阅读时长
  if ((window as any).__hushreaderSessionBookId !== book.id) {
    (window as any).__hushreaderSessionBookId = book.id;
    (window as any).__hushreaderSessionLastActive = now
  }

  const lastActive = (window as any).__hushreaderSessionLastActive || now
  const elapsed = now - lastActive

  if (elapsed > 0 && elapsed < 30 * 60 * 1000) {
    const totalMs = (book.readingTimeMs || 0) + elapsed
    updates.readingTimeMs = totalMs
    const totalChars = readerStore.chapters.reduce((sum, ch) => sum + ch.content.length, 0)
    const currentReadChars = Math.round(totalChars * (readerStore.readingPercent / 100))
    const prevReadChars = book.lastSaveReadChars || 0
    const deltaChars = Math.max(0, currentReadChars - prevReadChars)
    updates.lastSaveReadChars = currentReadChars
    const elapsedMinutes = elapsed / 60000
    if (elapsedMinutes > 0 && deltaChars > 0) {
      const sessionSpeed = deltaChars / elapsedMinutes
      const prevSpeed = book.readingSpeed || 0
      if (prevSpeed > 0) {
        updates.readingSpeed = Math.round(prevSpeed * 0.6 + sessionSpeed * 0.4)
      } else {
        updates.readingSpeed = Math.round(sessionSpeed)
      }
    }
  } else {
    const totalChars = readerStore.chapters.reduce((sum, ch) => sum + ch.content.length, 0)
    updates.lastSaveReadChars = Math.round(totalChars * (readerStore.readingPercent / 100))
  }

  if (readerStore.readingPercent >= 100 && !book.finishedAt) {
    updates.finishedAt = now
  }

  (window as any).__hushreaderSessionLastActive = now
  bookStore.updateBook(book.id, updates)
}

async function getFileModifiedTime(filePath: string): Promise<number | null> {
  try {
    return await platform.getFileModifiedTime(filePath)
  } catch {
    return null
  }
}

function makeFile(path: string, name: string, content: Uint8Array<ArrayBuffer>, mime: string): File {
  const blob = new Blob([content], { type: mime })
  return new File([blob], name)
}

async function parseBookAndGetChapters(book: typeof bookStore.currentBook): Promise<any[] | null> {
  if (!book) { toast('书籍不存在', 'error'); return null }

  const name = book.filePath.split(/[\\/]/).pop() ?? 'book'
  if (book.format === 'txt') {
    const text = await platform.readFile(book.filePath).catch(() => '')
    return parseTxt(text, configStore.config.other.chapterRegex || undefined)
  } else if (book.format === 'mobi') {
    const content = await platform.readFileBinary(book.filePath).catch(() => null)
    if (!content) { toast('无法读取MOBI文件', 'error'); return null }
    const file = makeFile(book.filePath, name, content, 'application/x-mobipocket-ebook')
    const result = await parseMobi(file)
    if (result.error) { toast(`MOBI解析失败：${result.error}`, 'error'); return null }
    return result.chapters
  } else {
    const content = await platform.readFileBinary(book.filePath).catch(() => null)
    if (!content) { toast('无法读取EPUB文件', 'error'); return null }
    const file = makeFile(book.filePath, name, content, 'application/epub+zip')
    try {
      const { chapters } = await parseEpub(file)
      return chapters
    } catch (e: any) {
      toast(`EPUB解析失败：${e.message}`, 'error')
      return null
    }
  }
}

async function openBookAndHushreader(bookId: string) {
  bookStore.setCurrentBook(bookId)
  const book = bookStore.currentBook
  if (!book) return

  const startChapter = book.lastChapter ?? 0
  const startIndex = book.progressIndex ?? 0

  readerStore.isLoading = true

  try {
    let chapters = await loadChapters(bookId)
    const currentModified = await getFileModifiedTime(book.filePath)
    const fileChanged = currentModified !== book.fileModifiedAt

    if (!chapters || fileChanged) {
      chapters = await parseBookAndGetChapters(book)
      if (!chapters) return
      saveChapters(bookId, chapters).catch(() => { })
    }

    readerStore.setChapters(chapters)

    if (currentModified !== book.fileModifiedAt) {
      bookStore.updateBook(bookId, { fileModifiedAt: currentModified })
    }

    updateHushreaderLayout()

    readerStore.goToProgress(startChapter, startIndex)

    hushreaderActivated.value = true
    isReaderHidden.value = false
    pushHushreaderState()
    startReadingTimer()
    nextTick(() => {
      pushHushreaderState()
    })
  } catch (e: any) {
    toast(`打开失败：${e.message}`, 'error')
  } finally {
    readerStore.isLoading = false
  }
}

provide('openBookAndHushreader', openBookAndHushreader)
provide('hideHushreaderWindow', hideHushreaderWindow)
provide('readerActiveBookId', readerActiveBookId)
provide('navigate', navigate)

function resizeHushreaderWindow(width: number, height: number) {
  if (!Number.isFinite(width) || !Number.isFinite(height)) return
  const size = clampHushreaderSize(width, height)
  hushreaderCfg.value.hushreaderWidth = size.width
  hushreaderCfg.value.hushreaderHeight = size.height
  void platform.readerResize(size.width, size.height)
  updateHushreaderLayout()
  configStore.save()
}

function moveHushreaderWindow(x: number, y: number) {
  if (!Number.isFinite(x) || !Number.isFinite(y)) return
  const bounds = getMovedHushreaderWindowBounds(x, y)
  hushreaderCfg.value.hushreaderX = bounds.x
  hushreaderCfg.value.hushreaderY = bounds.y
  void platform.readerMove(bounds.x, bounds.y)
  configStore.save()
}

function previewHushreaderWindowSize(width: number, height: number) {
  const size = clampHushreaderSize(width, height)
  void platform.readerResize(size.width, size.height)
}

function previewHushreaderWindowPosition(x: number, y: number) {
  const bounds = getMovedHushreaderWindowBounds(x, y)
  void platform.readerMove(bounds.x, bounds.y)
}

function handleHushreaderCommand(command: HushreaderCommand) {
  if (typeof command !== 'string') {
    if (command?.type === 'resize-preview' && typeof command.width === 'number' && typeof command.height === 'number') {
      previewHushreaderWindowSize(command.width, command.height)
    }
    if (command?.type === 'resize' && typeof command.width === 'number' && typeof command.height === 'number') {
      resizeHushreaderWindow(command.width, command.height)
    }
    if (command?.type === 'move-preview' && typeof command.x === 'number' && typeof command.y === 'number') {
      previewHushreaderWindowPosition(command.x, command.y)
    }
    if (command?.type === 'move' && typeof command.x === 'number' && typeof command.y === 'number') {
      moveHushreaderWindow(command.x, command.y)
    }
    if (command?.type === 'jump-percent' && typeof command.percent === 'number') {
      const percent = clampNumber(command.percent, 0, 100)
      const totalChars = readerStore.chapters.reduce((sum, ch) => sum + ch.content.length, 0)
      if (totalChars > 0) {
        const targetChar = Math.round((percent / 100) * totalChars)
        let accumulated = 0
        let targetChapter = 0
        for (let i = 0; i < readerStore.chapters.length; i++) {
          const chapterLen = readerStore.chapters[i].content.length
          if (accumulated + chapterLen >= targetChar) {
            targetChapter = i
            break
          }
          accumulated += chapterLen
          targetChapter = i
        }
        const charInChapter = Math.max(0, Math.min(targetChar - accumulated, readerStore.chapters[targetChapter]?.content.length ?? 0))
        readerStore.goToProgress(targetChapter, charInChapter)
        saveReadingProgress()
      }
    }
    return
  }
  if (command === 'prev') { readerStore.prevPage(); saveReadingProgress() }
  else if (command === 'next') { readerStore.nextPage(); saveReadingProgress() }
  else if (command === 'focus') focusHushreaderKeyboard()
  else if (command === 'blur') blurHushreaderKeyboard()
  else if (command === 'hide') hideHushreaderWindow()
  else if (command === 'close') closePlugin()
  else if (command === 'auto') toggleAutoPaging()
  else if (command === 'close-reader') { isReaderHidden.value = true; blurHushreaderKeyboard() }
  else if (command === 'destroy') { saveReadingProgress(); hushreaderActivated.value = false; stopReadingTimer(); void platform.readerClose() }
  else if (command === 'show-main') { void platform.showMainWindow() }
  else if (command === 'stop-auto') { isAutoPaging.value = false; hushreaderCfg.value.autoFlipEnabled = false }
  else if (command === 'start-auto') { if (currentBook.value) { isAutoPaging.value = true; hushreaderCfg.value.autoFlipEnabled = true } }
  else if (command === 'add-bookmark') {
    const book = currentBook.value
    if (book) {
      const pageText = hushreaderLines.value.join('')
      const boundaryRe = /[。！？…!?\.\」\』\）\】\」]/
      let start = 0
      const boundary = boundaryRe.exec(pageText)
      if (boundary) {
        start = boundary.index + boundary[0].length
        while (start < pageText.length && /[\s\u3000]/.test(pageText[start])) start++
      }
      if (start >= pageText.length) start = 0
      const rest = pageText.slice(start)
      const sentenceEndRe = /[。！？…!?\.]/
      let text = ''
      const endMatch = sentenceEndRe.exec(rest)
      if (endMatch) {
        text = rest.slice(0, endMatch.index + endMatch[0].length).trim()
      } else {
        text = rest.trim().slice(0, 50)
      }
      if (text.length > 50) text = text.slice(0, 50) + '...'
      const bookmark: Bookmark = {
        id: `bm_${Date.now()}_${Math.random().toString(36).slice(2)}`,
        chapterIndex: readerStore.currentChapterIndex,
        charIndex: readerStore.currentPageSlice.startIndex,
        readingPercent: readerStore.readingPercent,
        text,
        createdAt: Date.now()
      }
      const existing = book.bookmarks ?? []
      bookStore.updateBook(book.id, { bookmarks: [...existing, bookmark] })
      toast('书签已添加', 'success')
    }
  }
  else if (command === 'notification-close') { pendingNotificationCallback?.(); pendingNotificationCallback = null }
}

/** 全局快捷键动作分发（已移除全局快捷键，D-027 作废；保留结构备用）。 */
function handleMainCommand(command: unknown) {
  if (command === 'prev') {
    if (currentBook.value && !isReaderHidden.value) { readerStore.prevPage(); saveReadingProgress() }
  } else if (command === 'next') {
    if (currentBook.value && !isReaderHidden.value) { readerStore.nextPage(); saveReadingProgress() }
  }
}

watch(
  () => hushreaderCfg.value.autoFlipEnabled,
  (enabled) => {
    if (enabled && currentBook.value && !isReaderHidden.value) {
      isAutoPaging.value = true
    } else {
      isAutoPaging.value = false
    }
  }
)

watch(
  () => [isAutoPaging.value, hushreaderCfg.value.autoFlipInterval, bookStore.currentBookId, isReaderHidden.value],
  () => {
    window.clearInterval(autoTimer)
    if (!isAutoPaging.value || !currentBook.value || isReaderHidden.value) return
    autoTimer = window.setInterval(() => {
      if (isAutoPageTickRunning) return
      isAutoPageTickRunning = true
      try {
        const advanced = readerStore.nextPage()
        if (advanced) saveReadingProgress()
        else {
          if (!hushreaderCfg.value.continuousFlip) {
            isAutoPaging.value = false
            hushreaderCfg.value.autoFlipEnabled = false
          }
        }
      } finally {
        isAutoPageTickRunning = false
      }
    }, Math.max(1000, hushreaderCfg.value.autoFlipInterval))
  },
  { immediate: true }
)

watch(
  () => readerStore.currentChapterIndex,
  () => {
    if (currentBook.value) {
      updateHushreaderLayout()
      saveReadingProgress()
    }
  }
)

watch(
  () => [hushreaderCfg.value.hushreaderHeight, hushreaderCfg.value.hushreaderWidth, hushreaderCfg.value.fontSize, hushreaderCfg.value.lineHeight, hushreaderCfg.value.letterSpacing],
  () => {
    if (currentBook.value && readerStore.chapters.length > 0) {
      updateHushreaderLayout()
    }
  }
)

watch(
  () => [
    bookStore.currentBookId,
    readerStore.currentChapterIndex,
    readerStore.progressIndex,
    readerStore.readingPercent,
    isReaderHidden.value,
    hushreaderCfg.value.fontSize,
    hushreaderCfg.value.lineHeight,
    hushreaderCfg.value.hushreaderWidth,
    hushreaderCfg.value.hushreaderHeight,
    hushreaderCfg.value.hushreaderX,
    hushreaderCfg.value.hushreaderY,
    hushreaderCfg.value.letterSpacing,
    hushreaderCfg.value.opacity,
    hushreaderCfg.value.bgOpacity,
    hushreaderCfg.value.prevPageKey,
    hushreaderCfg.value.nextPageKey,
    hushreaderCfg.value.addBookmarkKey,
    hushreaderCfg.value.destroyKey,
    hushreaderCfg.value.showHushreaderMeta,
    hushreaderCfg.value.progressMode,
    hushreaderCfg.value.hideOnMouseLeave,
    hushreaderCfg.value.mouseEnterDelay,
    hushreaderCfg.value.wheelTurnPage,
    hushreaderCfg.value.bgColor,
    hushreaderCfg.value.textColor,
    hushreaderCfg.value.autoFlipEnabled,
    hushreaderCfg.value.fontFamily,
    cfg.value.function.windowMovable,
    cfg.value.function.windowSizeLocked
  ],
  () => {
    nextTick(() => {
      pushHushreaderState()
    })
  }
)

// 翻页快捷键变化时无需重新注册系统快捷键（全局快捷键已移除，D-027 作废）。
// 悬浮窗内的按键由悬浮窗页面自身处理。

onMounted(async () => {
  initWorkArea()
  await configStore.load()
  await bookStore.load()
  await downloadStore.init()

  // 回填书库目录（首次启动取 Rust 侧默认值；用户改过则保持原值）
  if (!configStore.config.other.bookshelfDir) {
    try {
      configStore.config.other.bookshelfDir = await platform.getBookshelfDir()
      configStore.save()
    } catch { }
  }

  offReaderCommand = await platform.onReaderCommand((c: unknown) => handleHushreaderCommand(c as HushreaderCommand)).catch(() => undefined)
  offMainCommand = await platform.onMainCommand(handleMainCommand).catch(() => undefined)

  if (!route.value) route.value = 'bookshelf'
})

onBeforeUnmount(() => {
  saveReadingProgress()
  downloadStore.dispose()
  window.clearInterval(autoTimer)
  stopReadingTimer()
  offReaderCommand?.()
  offMainCommand?.()
})
</script>

<template>
  <Bookshelf v-if="route === 'bookshelf'" :enter-action="enterAction" />
  <FindBook v-else-if="route === 'findbook'" />
  <Toast :message="toastMsg" :type="toastType" />
</template>
