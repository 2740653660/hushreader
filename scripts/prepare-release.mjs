#!/usr/bin/env node
/**
 * 生成 GitHub Release 更新清单（latest.json）—— node 实现（无编码坑）。
 *
 * 每次发布新版本时，在 tauri build 完成后运行本脚本：
 *   1. 读取最新版本号（src-tauri/tauri.conf.json）
 *   2. 读取构建产物的 NSIS 安装包与对应 .sig 签名文件
 *   3. 生成 latest.json（含版本号、更新说明、签名、下载地址）
 *
 * 发布时把以下 3 个文件一起上传到 GitHub Releases 页面（tag = v版本号）：
 *   - bundle/nsis/HushReader_x.y.z_x64-setup.exe
 *   - bundle/nsis/HushReader_x.y.z_x64-setup.exe.sig
 *   - latest.json
 *
 * 完整发布流程见 AGENTS.md「发布与密钥管理」章节。
 * 用法：node scripts/prepare-release.mjs [--notes "更新说明文本"]（默认读取 CHANGELOG.md 最新条目）
 */
import fs from 'node:fs'
import path from 'node:path'
import { fileURLToPath } from 'node:url'

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..')

const notesArgIdx = process.argv.indexOf('--notes')
let notesOverride = ''
if (notesArgIdx >= 0 && process.argv[notesArgIdx + 1]) {
  notesOverride = process.argv[notesArgIdx + 1]
}

// 读取版本号
const conf = JSON.parse(fs.readFileSync(path.join(root, 'src-tauri', 'tauri.conf.json'), 'utf8'))
const version = conf.version
if (!version) {
  console.error('无法读取 tauri.conf.json 中的版本号。')
  process.exit(1)
}

// 定位 NSIS 安装包与其签名文件
const nsisDir = path.join(root, 'src-tauri', 'target', 'release', 'bundle', 'nsis')
const files = fs.existsSync(nsisDir) ? fs.readdirSync(nsisDir) : []
const installerName = files
  .filter(f => f.startsWith(`HushReader_${version}_`) && f.endsWith('x64-setup.exe'))
  .sort((a, b) => fs.statSync(path.join(nsisDir, b)).mtimeMs - fs.statSync(path.join(nsisDir, a)).mtimeMs)[0]

if (!installerName) {
  console.error(`未找到 NSIS 安装包：${nsisDir}\\HushReader_${version}_*_x64-setup.exe。请先完成 tauri build。`)
  process.exit(1)
}
const installerPath = path.join(nsisDir, installerName)
const sigPath = `${installerPath}.sig`
if (!fs.existsSync(sigPath)) {
  console.error(`未找到签名文件：${sigPath}。请确认 tauri build 时已设置 TAURI_SIGNING_PRIVATE_KEY。`)
  process.exit(1)
}

// 更新说明：优先 --notes，否则取 CHANGELOG.md 最新版本条目
let notes = notesOverride
if (!notes) {
  const changelogPath = path.join(root, 'CHANGELOG.md')
  if (fs.existsSync(changelogPath)) {
    const lines = fs.readFileSync(changelogPath, 'utf8').split(/\r?\n/)
    // 版本标题形如 "## [0.1.0] - 日期" 或 "## 0.1.0"
    const inCurrent = lines.findIndex(l => /^##\s+\[?v?\d/.test(l))
    let out = []
    for (let i = inCurrent + 1; i < lines.length; i++) {
      if (/^##\s+\[?v?\d/.test(lines[i])) break
      out.push(lines[i])
    }
    notes = out.join('\n').trim()
  }
}

const signature = fs.readFileSync(sigPath, 'utf8').trim()
const downloadUrl = `https://github.com/2740653660/hushreader/releases/latest/download/${installerName}`

const latest = {
  version,
  notes,
  pub_date: new Date().toISOString(),
  platforms: {
    'windows-x86_64': {
      signature,
      url: downloadUrl,
    },
  },
}

fs.writeFileSync(path.join(root, 'latest.json'), JSON.stringify(latest, null, 2) + '\n', 'utf8')

console.log(`已生成 latest.json（版本 ${version}）`)
console.log(`  安装包：${installerName}`)
console.log(`  下载地址：${downloadUrl}`)
console.log('\n下一步：在 GitHub Releases（tag = v' + version + '）上传以下 3 个文件：')
console.log(`  1. ${installerPath}`)
console.log(`  2. ${sigPath}`)
console.log(`  3. ${path.join(root, 'latest.json')}`)
