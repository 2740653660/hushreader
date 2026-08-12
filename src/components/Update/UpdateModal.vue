<script setup lang="ts">
import { ref, computed, onBeforeUnmount } from 'vue'
import { platform } from '../../platform'

/**
 * 更新状态机：
 * - idle    已发现新版本，等待用户选择
 * - checking 正在检查（仅"手动检查更新"时短暂显示，由父组件传入状态）
 * - downloading 下载中（含进度）
 * - ready    下载完成，等待用户确认重启安装
 * - error    下载/检查失败，可重试
 */
export type UpdatePhase = 'checking' | 'idle' | 'downloading' | 'ready' | 'error' | 'installing'

const props = defineProps<{
  /** 当前状态机阶段。 */
  phase: UpdatePhase
  /** 可用的新版本信息（checking/error 时可为空）。 */
  version: string
  /** 当前版本号（展示用）。 */
  currentVersion: string
  /** 更新说明文本（来自 GitHub Release 正文）。 */
  body: string
  /** 下载进度 0-100。 */
  progress: number
  /** 下载的字节数（已下载/总计，便于展示）。 */
  downloadedBytes: number
  totalBytes: number
  /** 错误信息（phase === 'error' 时展示）。 */
  errorMessage: string
}>()

const emit = defineEmits<{
  install: []
  later: []
  ignore: []
  retry: []
  confirmRestart: []
  close: []
}>()

const downloadingPercent = computed(() => {
  if (props.totalBytes > 0) return Math.min(100, Math.round((props.downloadedBytes / props.totalBytes) * 100))
  return props.progress
})

function formatBytes(bytes: number): string {
  if (!bytes || bytes <= 0) return ''
  const mb = bytes / (1024 * 1024)
  if (mb >= 1024) return `${(mb / 1024).toFixed(1)} GB`
  return `${mb.toFixed(1)} MB`
}

/** 更新说明第一行可能是"版本标题"，按行展示，空行折叠。 */
const bodyLines = computed(() => {
  if (!props.body) return []
  return props.body.split('\n').filter(l => l.trim() !== '')
})

onBeforeUnmount(() => {
  // 关闭弹窗时若正在下载，不中断插件侧下载（安装前仍需确认）；
  // 下载句柄由父组件持有，这里无需清理。
})
</script>

<template>
  <div class="update-overlay" @click.self="emit('close')">
    <div class="update-box">
      <!-- Header -->
      <div class="update-header">
        <h3 class="update-title">软件更新</h3>
        <button class="update-close" @click="emit('close')" title="关闭">
          <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
            <line x1="18" y1="6" x2="6" y2="18" /><line x1="6" y1="6" x2="18" y2="18" />
          </svg>
        </button>
      </div>

      <div class="update-body">
        <!-- 检查中 -->
        <div v-if="phase === 'checking'" class="update-center">
          <div class="spinner"></div>
          <p class="update-hint">正在检查更新…</p>
        </div>

        <!-- 已是最新 -->
        <div v-else-if="phase === 'idle' && !version" class="update-center">
          <p class="update-current">当前版本 {{ currentVersion }}</p>
          <p class="update-hint">已是最新版本</p>
        </div>

        <!-- 发现新版本 -->
        <template v-else-if="version">
          <div class="update-version-row">
            <span class="update-badge">发现新版本</span>
            <span class="update-version">
              {{ currentVersion }} → <b>{{ version }}</b>
            </span>
          </div>

          <!-- 更新说明 -->
          <div v-if="bodyLines.length > 0" class="update-notes">
            <p v-for="(line, i) in bodyLines" :key="i" class="update-note-line">{{ line }}</p>
          </div>
          <p v-else class="update-hint" style="margin-top: 8px">本次更新没有提供说明</p>

          <!-- 下载中 -->
          <div v-if="phase === 'downloading'" class="update-download">
            <div class="progress-track">
              <div class="progress-fill" :style="{ width: downloadingPercent + '%' }"></div>
            </div>
            <p class="update-hint">
              正在下载更新…
              <template v-if="totalBytes > 0">
                {{ formatBytes(downloadedBytes) }} / {{ formatBytes(totalBytes) }}
              </template>
              （{{ downloadingPercent }}%）
            </p>
          </div>

          <!-- 下载完成 -->
          <div v-else-if="phase === 'ready'" class="update-ready">
            <p class="update-hint">下载完成，重启应用即可完成更新（书库与设置不受影响）。</p>
            <div class="update-actions">
              <button class="btn-secondary" @click="emit('later')">稍后再说</button>
              <button class="btn-primary" @click="emit('confirmRestart')">重启并安装</button>
            </div>
          </div>

          <!-- 出错 -->
          <div v-else-if="phase === 'error'" class="update-error">
            <p class="update-error-msg">{{ errorMessage || '更新失败，请检查网络后重试。' }}</p>
            <div class="update-actions">
              <button class="btn-secondary" @click="emit('later')">稍后再说</button>
              <button class="btn-primary" @click="emit('retry')">重试</button>
            </div>
          </div>

          <!-- 等待选择 -->
          <div v-else-if="phase === 'idle'" class="update-actions">
            <button class="btn-ghost" @click="emit('ignore')">忽略此版本</button>
            <div style="flex: 1"></div>
            <button class="btn-secondary" @click="emit('later')">稍后再说</button>
            <button class="btn-primary" @click="emit('install')">下载并安装</button>
          </div>
        </template>

        <!-- 异常状态兜底 -->
        <div v-else class="update-center">
          <p class="update-hint">{{ errorMessage || '暂时无法检查更新。' }}</p>
          <div class="update-actions">
            <button class="btn-secondary" @click="emit('close')">关闭</button>
            <button class="btn-primary" @click="emit('retry')">重试</button>
          </div>
        </div>
      </div>
    </div>
  </div>
</template>

<style scoped>
.update-overlay {
  position: fixed;
  inset: 0;
  background: var(--c-overlay-bg);
  backdrop-filter: blur(4px);
  display: flex;
  align-items: center;
  justify-content: center;
  z-index: 9000;
  animation: fade-in 0.15s var(--ease-out);
}

.update-box {
  background: var(--c-surface-overlay);
  border: 1px solid var(--c-border);
  border-radius: var(--radius-xl);
  width: 440px;
  max-width: 94vw;
  max-height: 80vh;
  display: flex;
  flex-direction: column;
  box-shadow: var(--shadow-xl);
  animation: slide-up 0.2s var(--ease-out);
}

.update-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 16px 20px 14px;
  border-bottom: 1px solid var(--c-border);
  flex-shrink: 0;
}

.update-title {
  font-size: 15px;
  font-weight: 700;
  letter-spacing: -0.01em;
}

.update-close {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 28px;
  height: 28px;
  border-radius: var(--radius-sm);
  color: var(--c-ink-tertiary);
  transition: background 0.12s var(--ease-out), color 0.12s var(--ease-out);
}

.update-close:hover {
  background: var(--c-surface-sunken);
  color: var(--c-ink);
}

.update-body {
  padding: 18px 20px 20px;
  overflow-y: auto;
}

.update-center {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 12px;
  padding: 20px 0;
}

.spinner {
  width: 26px;
  height: 26px;
  border: 3px solid var(--c-border);
  border-top-color: var(--c-accent);
  border-radius: 50%;
  animation: spin 0.8s linear infinite;
}

@keyframes spin {
  to { transform: rotate(360deg); }
}

.update-current {
  font-size: 13px;
  color: var(--c-ink);
}

.update-hint {
  font-size: 12px;
  color: var(--c-ink-tertiary);
  line-height: 1.6;
  text-align: center;
}

.update-version-row {
  display: flex;
  align-items: center;
  gap: 10px;
  margin-bottom: 12px;
}

.update-badge {
  background: var(--c-accent-soft);
  color: var(--c-accent);
  font-size: 11px;
  font-weight: 700;
  padding: 3px 8px;
  border-radius: var(--radius-full);
  flex-shrink: 0;
}

.update-version {
  font-size: 14px;
  color: var(--c-ink);
  font-family: var(--font-mono);
}

.update-notes {
  background: var(--c-surface-sunken);
  border: 1px solid var(--c-border);
  border-radius: var(--radius-sm);
  padding: 10px 12px;
  max-height: 180px;
  overflow-y: auto;
  margin-bottom: 14px;
}

.update-note-line {
  font-size: 12px;
  color: var(--c-ink-secondary);
  line-height: 1.7;
}

.update-download {
  margin-top: 4px;
}

.progress-track {
  height: 6px;
  background: var(--c-border-strong);
  border-radius: var(--radius-full);
  overflow: hidden;
  margin-bottom: 8px;
}

.progress-fill {
  height: 100%;
  background: var(--c-accent);
  border-radius: var(--radius-full);
  transition: width 0.15s linear;
}

.update-ready {
  margin-top: 4px;
}

.update-error {
  margin-top: 4px;
}

.update-error-msg {
  font-size: 12px;
  color: var(--c-danger);
  line-height: 1.6;
  margin-bottom: 12px;
}

.update-actions {
  display: flex;
  align-items: center;
  gap: 8px;
  margin-top: 14px;
  justify-content: flex-end;
}

.btn-primary,
.btn-secondary,
.btn-ghost {
  padding: 8px 18px;
  border-radius: var(--radius-sm);
  font-size: 13px;
  font-weight: 500;
  transition: all 0.15s var(--ease-out);
}

.btn-primary {
  background: var(--c-accent);
  color: var(--c-ink-inverse);
}

.btn-primary:hover {
  background: var(--c-accent-hover);
}

.btn-secondary {
  background: var(--c-surface-sunken);
  color: var(--c-ink);
  border: 1px solid var(--c-border);
}

.btn-secondary:hover {
  background: var(--c-border);
}

.btn-ghost {
  color: var(--c-ink-tertiary);
  padding-left: 0;
}

.btn-ghost:hover {
  color: var(--c-ink);
}
</style>
