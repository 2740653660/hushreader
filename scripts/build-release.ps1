# 在 Windows 本机执行 release 构建（代理/签名密钥/签名密码统一处理）
#
# 供 AI 代理直接在本机（产品负责人 Windows 电脑）构建 release 产物使用：
#   - 从本机密钥目录读取签名密码（不打印密码本身）
#   - 设置签名私钥路径与代理环境变量
#   - 执行 npx tauri build --bundles nsis
#
# 产物：src-tauri/target/release/bundle/nsis/隐阅阁_<版本>_x64-setup.exe 及 .sig
# 发布流程见 AGENTS.md「发布与密钥管理」。
#
# 用法：PowerShell 中执行  .\scripts\build-release.ps1
# 可选：-PasswordFile <密码文件路径>（默认 %USERPROFILE%\.hushreader-update-key\password.txt）

param(
  [string]$PasswordFile = ''
)

$ErrorActionPreference = 'Stop'

if (-not $PasswordFile) {
  $PasswordFile = Join-Path $env:USERPROFILE '.hushreader-update-key\password.txt'
}
if (-not (Test-Path $PasswordFile)) {
  Write-Error "未找到签名密码文件：$PasswordFile"
  exit 1
}

$keyFile = Join-Path $env:USERPROFILE '.hushreader-update-key\hushreader.key'
if (-not (Test-Path $keyFile)) {
  Write-Error "未找到签名私钥：$keyFile"
  exit 1
}

# 读取密码（文件内可能带换行，Trim 掉）
$password = (Get-Content $PasswordFile -Raw -Encoding UTF8).Trim()
if (-not $password) {
  Write-Error '签名密码为空。'
  exit 1
}

# 代理（GitHub/下载依赖用）
$env:HTTPS_PROXY = 'http://127.0.0.1:7897'
$env:HTTP_PROXY = 'http://127.0.0.1:7897'

# 签名环境变量
$env:TAURI_SIGNING_PRIVATE_KEY = $keyFile
$env:TAURI_SIGNING_PRIVATE_KEY_PASSWORD = $password

Write-Host '已读取签名私钥与密码（密码不显示）。'
Write-Host '开始 tauri build（release）...'

npx tauri build --bundles nsis
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
Write-Host '构建完成。'
