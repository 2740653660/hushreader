# 准备 So Novel 下载后台资源（Windows 构建机专用）
#
# 从本机官方发布包复制 app.jar / runtime（JRE）/ rules 到
# src-tauri/resources/sonovel/，供 tauri build 打包进安装包。
# 该目录（约182MB）已在 .gitignore 中忽略，不入 git 仓库。
#
# 用法：PowerShell 中执行  .\scripts\prepare-sonovel.ps1
# 前置条件：本机存在官方 SoNovel 发布包（可改 $Source 变量指定路径）。

$ErrorActionPreference = 'Stop'

$Source = Join-Path $env:USERPROFILE 'Downloads\SoNovel'
$Dest = Join-Path $PSScriptRoot '..\src-tauri\resources\sonovel'

if (-not (Test-Path (Join-Path $Source 'app.jar'))) {
  Write-Error "未找到官方 SoNovel 发布包：$Source（app.jar 缺失）。请先下载官方发布包并放到该位置。"
  exit 1
}

New-Item -ItemType Directory -Force -Path $Dest | Out-Null

Write-Host '复制 app.jar ...'
Copy-Item (Join-Path $Source 'app.jar') (Join-Path $Dest 'app.jar') -Force

Write-Host '复制 runtime/（JRE，约145MB）...'
Copy-Item (Join-Path $Source 'runtime') (Join-Path $Dest 'runtime') -Recurse -Force

Write-Host '复制 rules/ ...'
Copy-Item (Join-Path $Source 'rules') (Join-Path $Dest 'rules') -Recurse -Force

$size = (Get-ChildItem $Dest -Recurse -File | Measure-Object -Property Length -Sum).Sum / 1MB
Write-Host "完成：So Novel 后台资源已就绪（约 $([math]::Round($size))MB）-> $Dest"
