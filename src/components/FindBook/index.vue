<script setup lang="ts">
import { computed, inject, onMounted, ref } from 'vue'
import { useDownloadStore, type SearchResult } from '../../stores/download'
import { useConfigStore } from '../../stores/config'
import Toast from '../Bookshelf/Toast.vue'

const downloadStore = useDownloadStore()
const configStore = useConfigStore()
const navigate = inject<(page: 'bookshelf' | 'findbook') => void>('navigate')

const searchInput = ref('')
const toastMsg = ref('')
const toastType = ref<'info' | 'success' | 'error'>('info')
let toastTimer = 0
function toast(msg: string, type: 'info' | 'success' | 'error' = 'info') {
  toastMsg.value = msg
  toastType.value = type
  clearTimeout(toastTimer)
  toastTimer = window.setTimeout(() => { toastMsg.value = '' }, 3000)
}

async function doSearch() {
  const kw = searchInput.value.trim()
  if (!kw || downloadStore.searching) return
  await downloadStore.search(kw)
}

function onKeydown(e: KeyboardEvent) {
  if (e.key === 'Enter') {
    doSearch()
  }
}

/** 下载按钮（EPUB/TXT），一次一本。 */
async function startDownload(result: SearchResult, format: 'epub' | 'txt') {
  if (!downloadStore.backendRunning) {
    toast('下载后台未就绪，请点击页面上的"启动后台"', 'error')
    return
  }
  if (downloadStore.task?.state === 'downloading') {
    toast('已有下载任务进行中，请等待完成或放弃后重试', 'info')
    return
  }
  const ok = await downloadStore.startDownload(result, format)
  if (ok) {
    toast(`开始下载《${result.bookName || ''}》（${format.toUpperCase()}）`, 'info')
  }
}

async function retryBackend() {
  await downloadStore.startBackend()
  if (downloadStore.backendRunning) {
    toast('下载后台已就绪', 'success')
  } else {
    toast('下载后台启动失败，请稍后重试', 'error')
  }
}

const progressText = computed(() => {
  const t = downloadStore.task
  if (!t) return ''
  if (t.total > 0) {
    return `${t.index}/${t.total} 章`
  }
  return '准备中...'
})

const percent = computed(() => downloadStore.task?.percent ?? 0)

function formatSize(size?: number) {
  if (!size) return ''
  if (size < 1024) return `${size}B`
  if (size < 1024 * 1024) return `${(size / 1024).toFixed(1)}KB`
  return `${(size / 1024 / 1024).toFixed(1)}MB`
}

/** 简介截断。 */
function shorten(text: string | undefined, max = 120) {
  if (!text) return ''
  const t = text.replace(/\s+/g, ' ').trim()
  return t.length > max ? t.slice(0, max) + '…' : t
}

onMounted(() => {
  // 进入页面时若后台未就绪，自动尝试启动一次
  if (!downloadStore.backendRunning && !downloadStore.backendError) {
    void downloadStore.startBackend()
  }
})
</script>

<template>
  <div class="findbook">
    <!-- Header -->
    <header class="fb-header">
      <button class="fb-back" title="返回书架" @click="navigate?.('bookshelf')">
        <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
          <path d="m15 18-6-6 6-6" />
        </svg>
        <span>书架</span>
      </button>
      <h1 class="fb-title">找书</h1>
      <div class="fb-backend" v-if="!downloadStore.backendRunning">
        <span class="fb-backend-dot"></span>
        <span class="fb-backend-text">{{ downloadStore.backendError || '下载后台未就绪' }}</span>
        <button class="fb-retry" @click="retryBackend">重试</button>
      </div>
    </header>

    <!-- Search bar -->
    <div class="fb-search-wrap">
      <div class="fb-search">
        <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
          <circle cx="11" cy="11" r="8" />
          <path d="m21 21-4.35-4.35" />
        </svg>
        <input
          v-model="searchInput"
          class="fb-search-input"
          placeholder="输入书名或作者，回车搜索"
          @keydown="onKeydown"
        />
        <button class="fb-search-btn" :disabled="!searchInput.trim() || downloadStore.searching"
          @click="doSearch">
          <span v-if="downloadStore.searching" class="spinner" style="width:14px;height:14px;border-width:1.5px;margin-right:4px"></span>
          {{ downloadStore.searching ? '搜索中...' : '搜索' }}
        </button>
      </div>
      <p class="fb-hint" v-if="!downloadStore.searching && downloadStore.results.length === 0 && !downloadStore.searchError">
        一次搜索会同时查询多个书源，约需数秒到十几秒。选择结果后可下载 EPUB 或 TXT。
      </p>
    </div>

    <!-- Search error -->
    <div v-if="downloadStore.searchError && !downloadStore.searching" class="fb-error">
      <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
        <circle cx="12" cy="12" r="10" />
        <line x1="12" y1="8" x2="12" y2="12" />
        <line x1="12" y1="16" x2="12.01" y2="16" />
      </svg>
      <span>{{ downloadStore.searchError }}</span>
    </div>

    <!-- Download task panel -->
    <div v-if="downloadStore.task" class="fb-task" :class="`state-${downloadStore.task.state}`">
      <div class="fb-task-head">
        <div class="fb-task-info">
          <strong class="fb-task-name">{{ downloadStore.task.bookName }}</strong>
          <span class="fb-task-source">{{ downloadStore.task.sourceName }} · {{ downloadStore.task.format.toUpperCase() }}</span>
        </div>
        <div class="fb-task-actions">
          <template v-if="downloadStore.task.state === 'downloading'">
            <span class="fb-task-percent">{{ percent }}%</span>
            <span class="fb-task-progress-text">{{ progressText }}</span>
            <button class="fb-abort" @click="downloadStore.abortDownload()">放弃</button>
          </template>
          <template v-else-if="downloadStore.task.state === 'success'">
            <span class="fb-state-success">下载完成，已加入书架</span>
            <button class="fb-goto-shelf" @click="navigate?.('bookshelf')">去书架</button>
          </template>
          <template v-else-if="downloadStore.task.state === 'error'">
            <span class="fb-state-error">{{ downloadStore.task.error || '下载失败' }}</span>
          </template>
          <template v-else-if="downloadStore.task.state === 'aborted'">
            <span class="fb-state-muted">已放弃，不会加入书架</span>
          </template>
        </div>
      </div>
      <div class="fb-progress-track">
        <div class="fb-progress-fill" :style="{ width: percent + '%' }"></div>
      </div>
    </div>

    <!-- Results -->
    <main v-if="downloadStore.results.length > 0" class="fb-results">
      <p class="fb-result-count">共找到 {{ downloadStore.results.length }} 条结果（含多个书源）</p>
      <div v-for="(r, i) in downloadStore.results" :key="i" class="fb-result-card">
        <div class="fb-result-main">
          <div class="fb-result-title-row">
            <strong class="fb-result-title">{{ r.bookName || '未知书名' }}</strong>
            <span class="fb-source-tag">{{ r.sourceName || '书源' }}</span>
          </div>
          <div class="fb-result-meta">
            <template v-if="r.author"><span>{{ r.author }}</span></template>
            <template v-if="r.status"><span class="fb-meta-item">{{ r.status }}</span></template>
            <template v-if="r.category"><span class="fb-meta-item">{{ r.category }}</span></template>
            <template v-if="r.wordCount"><span class="fb-meta-item">{{ r.wordCount }}</span></template>
            <template v-if="r.latestChapter"><span class="fb-meta-item">最新：{{ r.latestChapter }}</span></template>
            <template v-if="r.lastUpdateTime"><span class="fb-meta-item">更新：{{ r.lastUpdateTime }}</span></template>
          </div>
          <p v-if="shorten(r.intro)" class="fb-result-intro">{{ shorten(r.intro) }}</p>
        </div>
        <div class="fb-result-actions">
          <button class="fb-dl-btn epub" :disabled="downloadStore.task?.state === 'downloading'" @click="startDownload(r, 'epub')">EPUB</button>
          <button class="fb-dl-btn txt" :disabled="downloadStore.task?.state === 'downloading'" @click="startDownload(r, 'txt')">TXT</button>
        </div>
      </div>
    </main>

    <!-- Empty -->
    <div v-if="!downloadStore.searching && !downloadStore.searchError && downloadStore.results.length === 0 && searchInput.trim()" class="fb-empty">
      <svg width="48" height="48" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1">
        <circle cx="11" cy="11" r="8" />
        <path d="m21 21-4.35-4.35" />
      </svg>
      <p>没有找到匹配的书籍，试试换个关键词</p>
    </div>

    <Toast :message="toastMsg" :type="toastType" />
  </div>
</template>

<style scoped>
.findbook {
  height: 100%;
  display: flex;
  flex-direction: column;
  background: var(--c-surface);
  overflow: hidden;
}

.fb-header {
  display: flex;
  align-items: center;
  gap: 12px;
  padding: 14px 20px;
  border-bottom: 1px solid var(--c-border);
  flex-shrink: 0;
}

.fb-back {
  display: flex;
  align-items: center;
  gap: 4px;
  padding: 6px 10px;
  border-radius: var(--radius-md);
  font-size: 13px;
  color: var(--c-ink-secondary);
  transition: background 0.15s var(--ease-out);
}

.fb-back:hover {
  background: var(--c-surface-sunken);
  color: var(--c-ink);
}

.fb-title {
  font-size: 17px;
  font-weight: 700;
  letter-spacing: -0.02em;
}

.fb-backend {
  margin-left: auto;
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 5px 12px;
  border-radius: var(--radius-md);
  background: var(--c-danger-soft);
  color: var(--c-danger);
  font-size: 12px;
}

.fb-backend-dot {
  width: 7px;
  height: 7px;
  border-radius: 50%;
  background: currentColor;
}

.fb-retry {
  padding: 2px 10px;
  border-radius: var(--radius-xs);
  background: var(--c-danger);
  color: var(--c-ink-inverse);
  font-size: 12px;
}

.fb-retry:hover {
  opacity: 0.85;
}

.fb-search-wrap {
  padding: 16px 20px 8px;
  flex-shrink: 0;
}

.fb-search {
  display: flex;
  align-items: center;
  gap: 10px;
  background: var(--c-surface-raised);
  border: 1px solid var(--c-border);
  border-radius: var(--radius-lg);
  padding: 8px 8px 8px 14px;
  transition: border-color 0.15s var(--ease-out), box-shadow 0.15s var(--ease-out);
}

.fb-search:focus-within {
  border-color: var(--c-accent);
  box-shadow: 0 0 0 3px var(--c-accent-soft);
}

.fb-search-input {
  flex: 1;
  border: none;
  background: none;
  font-size: 14px;
  color: var(--c-ink);
}

.fb-search-input::placeholder {
  color: var(--c-ink-tertiary);
}

.fb-search-btn {
  padding: 7px 18px;
  border-radius: var(--radius-md);
  background: var(--c-accent);
  color: var(--c-ink-inverse);
  font-size: 13px;
  font-weight: 600;
  white-space: nowrap;
}

.fb-search-btn:hover:not(:disabled) {
  background: var(--c-accent-hover);
}

.fb-hint {
  padding: 8px 2px 0;
  font-size: 12px;
  color: var(--c-ink-tertiary);
}

.fb-error {
  margin: 8px 20px;
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 10px 14px;
  border-radius: var(--radius-md);
  background: var(--c-danger-soft);
  color: var(--c-danger);
  font-size: 13px;
  flex-shrink: 0;
}

.fb-task {
  margin: 8px 20px;
  padding: 12px 16px;
  border-radius: var(--radius-lg);
  border: 1px solid var(--c-border);
  background: var(--c-surface-raised);
  flex-shrink: 0;
}

.fb-task.state-error {
  border-color: var(--c-danger);
  background: var(--c-danger-soft);
}

.fb-task.state-success {
  border-color: var(--c-success);
  background: var(--c-success-soft);
}

.fb-task-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  margin-bottom: 8px;
}

.fb-task-info {
  display: flex;
  flex-direction: column;
  min-width: 0;
}

.fb-task-name {
  font-size: 13px;
  color: var(--c-ink);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.fb-task-source {
  font-size: 11px;
  color: var(--c-ink-tertiary);
}

.fb-task-actions {
  display: flex;
  align-items: center;
  gap: 10px;
  font-size: 12px;
  flex-shrink: 0;
}

.fb-task-percent {
  font-weight: 700;
  color: var(--c-accent);
  font-family: var(--font-mono);
}

.fb-task-progress-text {
  color: var(--c-ink-secondary);
  font-family: var(--font-mono);
}

.fb-abort {
  padding: 3px 10px;
  border-radius: var(--radius-xs);
  background: var(--c-surface-sunken);
  border: 1px solid var(--c-border);
  color: var(--c-ink-secondary);
  font-size: 12px;
}

.fb-abort:hover {
  color: var(--c-danger);
  border-color: var(--c-danger);
}

.fb-state-success {
  color: var(--c-success);
}

.fb-state-error {
  color: var(--c-danger);
}

.fb-state-muted {
  color: var(--c-ink-tertiary);
}

.fb-goto-shelf {
  padding: 4px 12px;
  border-radius: var(--radius-xs);
  background: var(--c-success);
  color: var(--c-ink-inverse);
  font-size: 12px;
  font-weight: 600;
}

.fb-goto-shelf:hover {
  opacity: 0.85;
}

.fb-progress-track {
  height: 6px;
  border-radius: var(--radius-full);
  background: var(--c-border);
  overflow: hidden;
}

.fb-progress-fill {
  height: 100%;
  border-radius: var(--radius-full);
  background: var(--c-accent);
  transition: width 0.2s var(--ease-out);
}

.fb-results {
  flex: 1;
  overflow-y: auto;
  padding: 8px 20px 20px;
}

.fb-result-count {
  font-size: 12px;
  color: var(--c-ink-tertiary);
  margin-bottom: 10px;
}

.fb-result-card {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: 16px;
  padding: 14px 16px;
  margin-bottom: 10px;
  border-radius: var(--radius-lg);
  border: 1px solid var(--c-border);
  background: var(--c-surface-raised);
  transition: border-color 0.15s var(--ease-out), box-shadow 0.15s var(--ease-out);
}

.fb-result-card:hover {
  border-color: var(--c-border-strong);
  box-shadow: var(--shadow-sm);
}

.fb-result-main {
  min-width: 0;
}

.fb-result-title-row {
  display: flex;
  align-items: center;
  gap: 8px;
  flex-wrap: wrap;
}

.fb-result-title {
  font-size: 15px;
  color: var(--c-ink);
}

.fb-source-tag {
  padding: 1px 8px;
  border-radius: var(--radius-full);
  background: var(--c-accent-soft);
  color: var(--c-accent);
  font-size: 11px;
  white-space: nowrap;
}

.fb-result-meta {
  display: flex;
  flex-wrap: wrap;
  gap: 4px 12px;
  margin-top: 6px;
  font-size: 12px;
  color: var(--c-ink-secondary);
}

.fb-result-intro {
  margin-top: 8px;
  font-size: 12px;
  line-height: 1.7;
  color: var(--c-ink-tertiary);
  display: -webkit-box;
  -webkit-line-clamp: 2;
  -webkit-box-orient: vertical;
  overflow: hidden;
}

.fb-result-actions {
  display: flex;
  gap: 8px;
  flex-shrink: 0;
}

.fb-dl-btn {
  padding: 6px 14px;
  border-radius: var(--radius-md);
  font-size: 12px;
  font-weight: 700;
  letter-spacing: 0.03em;
}

.fb-dl-btn.epub {
  background: var(--c-accent);
  color: var(--c-ink-inverse);
}

.fb-dl-btn.epub:hover:not(:disabled) {
  background: var(--c-accent-hover);
}

.fb-dl-btn.txt {
  background: var(--c-surface-sunken);
  color: var(--c-ink);
  border: 1px solid var(--c-border);
}

.fb-dl-btn.txt:hover:not(:disabled) {
  background: var(--c-border);
}

.fb-empty {
  flex: 1;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 12px;
  color: var(--c-ink-tertiary);
  font-size: 13px;
}

.spinner {
  display: inline-block;
  width: 16px;
  height: 16px;
  border: 2px solid var(--c-accent-muted);
  border-top-color: var(--c-accent);
  border-radius: 50%;
  animation: spin 0.8s linear infinite;
  vertical-align: middle;
}
</style>
