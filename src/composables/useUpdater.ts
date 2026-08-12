import { ref } from 'vue'
import { platform } from '../platform'
import { useConfigStore } from '../stores/config'
import type { Update } from '@tauri-apps/plugin-updater'
import type { UpdatePhase } from '../components/Update/UpdateModal.vue'

/**
 * GitHub 应用内更新状态与操作（模块级单例）。
 * App 启动自动检查与设置页手动检查共用同一份状态，更新弹窗统一在 App 根节点渲染。
 */
const DAY_MS = 24 * 60 * 60 * 1000

const updateVisible = ref(false)
const updatePhase = ref<UpdatePhase>('checking')
const updateVersion = ref('')
const updateBody = ref('')
const updateError = ref('')
const updateProgress = ref(0)
const updateDownloadedBytes = ref(0)
const updateTotalBytes = ref(0)
const isChecking = ref(false)
const currentVersion = ref('')

let pendingUpdate: Update | null = null

/** 获取当前应用版本（Tauri 提供）。 */
async function loadCurrentVersion() {
  try {
    const { getVersion } = await import('@tauri-apps/api/app')
    currentVersion.value = await getVersion()
  } catch {
    currentVersion.value = ''
  }
}

export function useUpdater() {
  const configStore = useConfigStore()

  function openModal(phase: UpdatePhase, version = '', body = '', error = '') {
    updatePhase.value = phase
    updateVersion.value = version
    updateBody.value = body
    updateError.value = error
    updateProgress.value = 0
    updateDownloadedBytes.value = 0
    updateTotalBytes.value = 0
    updateVisible.value = true
  }

  function closeModal() {
    updateVisible.value = false
  }

  /**
   * 检查更新。
   * @param opts.silent 为 true 时"已是最新/出错"不弹窗（启动自动检查用）。
   * @param opts.force  为 true 时忽略"已忽略版本"过滤（手动检查用）。
   * @returns 是否成功完成一次检查。
   */
  async function checkForUpdate(opts?: { silent?: boolean; force?: boolean }): Promise<boolean> {
    if (isChecking.value) return false
    isChecking.value = true
    try {
      const update = await platform.checkForUpdate(25000)
      if (update) {
        pendingUpdate = update
        const ignored = configStore.config.other.ignoredUpdateVersion
        if (opts?.force || update.version !== ignored) {
          // 记录待处理更新：即使关闭弹窗，设置页仍会提示"发现新版本"
          configStore.config.other.pendingUpdateVersion = update.version
          configStore.config.other.pendingUpdateBody = update.body ?? ''
          configStore.save()
          // 手动检查与启动自动检查发现新版本时都直接弹窗告知
          openModal('idle', update.version, update.body ?? '')
        }
      } else if (!opts?.silent) {
        openModal('idle', '', '', '')
      }
      configStore.config.other.lastUpdateCheckAt = Date.now()
      configStore.save()
      return true
    } catch (e: any) {
      if (!opts?.silent) {
        openModal('error', '', '', `检查更新失败：${e}`)
      }
      return false
    } finally {
      isChecking.value = false
    }
  }

  /** 启动时自动检查：每天最多一次，失败静默，发现新版本且未忽略时才弹窗。 */
  async function autoCheck(): Promise<void> {
    const last = configStore.config.other.lastUpdateCheckAt || 0
    if (Date.now() - last < DAY_MS) return
    await checkForUpdate({ silent: true, force: false })
  }

  /** 下载更新（仅下载，不安装）；进度通过事件回调更新。 */
  async function startDownload() {
    const update = pendingUpdate
    if (!update) return
    updatePhase.value = 'downloading'
    try {
      await platform.downloadUpdate(update, (event) => {
        if (event.event === 'Started') {
          updateTotalBytes.value = event.contentLength ?? 0
        } else if (event.event === 'Progress') {
          updateDownloadedBytes.value += event.chunkLength ?? 0
          // GitHub 下载经重定向可能拿不到总大小（contentLength 缺失），
          // 此时进度条显示下载中动画与已下载字节，不做百分比
          if (updateTotalBytes.value > 0) {
            updateProgress.value = Math.round((updateDownloadedBytes.value / updateTotalBytes.value) * 100)
          }
        } else if (event.event === 'Finished') {
          updateProgress.value = 100
        }
      })
      updatePhase.value = 'ready'
    } catch (e: any) {
      updateError.value = `下载失败：${e}`
      updatePhase.value = 'error'
    }
  }

  /** 安装并重启应用（用户点击"重启并安装"后执行）。 */
  async function restartToInstall() {
    const update = pendingUpdate
    if (!update) return
    updatePhase.value = 'installing'
    try {
      // 安装器会覆盖捆绑的 Java 文件；先停止后台释放 extnet.dll 文件锁。
      // process 插件直接重启不会触发 Rust 的 RunEvent::Exit。
      await platform.backendStop()
      // 先安装（运行安装器），再重启；完成后本机即为新版本
      await platform.installUpdate(update)
      // 安装成功，清除待处理标记
      configStore.config.other.pendingUpdateVersion = ''
      configStore.config.other.pendingUpdateBody = ''
      configStore.save()
      await platform.relaunchApp()
    } catch (e: any) {
      updateError.value = `安装失败：${e}`
      updatePhase.value = 'error'
    }
  }

  /** 忽略当前版本：记入配置并清除待处理标记，之后不再自动提示。 */
  function ignoreThisVersion() {
    if (updateVersion.value) {
      configStore.config.other.ignoredUpdateVersion = updateVersion.value
      configStore.config.other.pendingUpdateVersion = ''
      configStore.config.other.pendingUpdateBody = ''
      configStore.save()
    }
    updateVisible.value = false
  }

  return {
    updateVisible, updatePhase, updateVersion, updateBody, updateError,
    updateProgress, updateDownloadedBytes, updateTotalBytes, isChecking, currentVersion,
    loadCurrentVersion,
    openModal, closeModal, checkForUpdate, autoCheck, startDownload, restartToInstall, ignoreThisVersion
  }
}
