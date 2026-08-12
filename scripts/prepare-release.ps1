# 生成 GitHub Release 更新清单（latest.json）
#
# 薄壳：实际生成逻辑在 scripts/prepare-release.mjs（node 实现，无编码坑）。
# 每次发布新版本时，在 tauri build 完成后运行本脚本：
#   1. 读取最新版本号（src-tauri/tauri.conf.json）
#   2. 读取构建产物的 NSIS 安装包与对应 .sig 签名文件
#   3. 生成 latest.json（含版本号、更新说明、签名、下载地址）
#
# 发布时把以下 3 个文件一起上传到 GitHub Releases 页面（tag = v版本号）：
#   - bundle/nsis/HushReader_x.y.z_x64-setup.exe
#   - bundle/nsis/HushReader_x.y.z_x64-setup.exe.sig
#   - latest.json
#
# 完整发布流程见 AGENTS.md「发布与密钥管理」章节。
# 用法：PowerShell 中执行  .\scripts\prepare-release.ps1
# 可选：-Notes "更新说明文本"（默认读取 CHANGELOG.md 最新条目）。

param(
  [string]$Notes = ''
)

$ErrorActionPreference = 'Stop'

$script = Join-Path $PSScriptRoot 'prepare-release.mjs'
$args = @()
if ($Notes) { $args = @('--notes', $Notes) }
node $script @args
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
