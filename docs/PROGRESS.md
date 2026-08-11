# 隐阅盒项目进度

最后更新：2026-08-11

## 当前阶段

第一版实施方案已获批准（含 Tauri 技术选型 D-022），正在执行步骤 1：架构与核心能力验证。

## 已完成

- 检查 HushReader 和 So Novel 上游仓库。
- 确认 HushReader 当前是 Vue/TypeScript 的 ZTools 插件。
- 确认 So Novel 是 Java 21 应用，已有搜索、下载、进度、文件和书源规则能力。
- 确认两个项目可以通过受控的本机后台进程整合。
- 识别打包、内存、接口安全、隐私上报、依赖和许可证问题。
- 确认产品方向和第一版主要功能。
- 建立持续维护的代理规则、产品需求、路线图、决策和进度文档。
- 精简 `AGENTS.md`，补充产品负责人的非技术背景、双方分工和沟通规则，并将易变内容留在项目文档中维护。
- 初始化 CodeGraph，当前索引包含 20 个代码文件、525 个节点和 1,844 条关系，状态正常。
- 确认第一版全部待确认问题并记录决定（D-017 至 D-021）。
- 提交第一版实施方案 `docs/PLAN_V1.md` 并获产品负责人批准。
- 确认桌面外壳采用 Tauri（D-022），方案相应更新，开始步骤 1。

## 尚未开始

- 替换 ZTools 接口。
- 引入或打包 So Novel 源码。
- 产品界面开发。
- 下载整合。
- Windows 安装包。
- GitHub 应用内更新。

## 当前授权状态

第一版方案已批准，步骤 1（架构与核心能力验证）已获授权并开始执行。后续每个步骤完成时向产品负责人演示确认后再继续。

## 当前工作

- 步骤 1 进行中：Tauri 外壳骨架已搭建，并已在 Windows 本机（`D:\AI_Project\hushreader`）成功编译打包。
  - 已实现并编译通过：透明无边框置顶窗口、全局快捷键（F9 老板键、左右方向键翻页）、系统托盘、单实例、状态页轮询、可拖动区域悬停显示"移动"光标。
  - 本机构建产物（release）：
    - 主程序：`src-tauri/target/release/app.exe`（约 9MB）
    - 安装包：`src-tauri/target/release/bundle/nsis/hushreader-capability-check_0.1.0_x64-setup.exe`（约 1.9MB）
    - MSI：`src-tauri/target/release/bundle/msi/hushreader-capability-check_0.1.0_x64_en-US.msi`（约 3MB）
  - 本机工具链：Node 24 + npm、Rust 1.95（MSVC）、VS 2022 生成工具、Windows SDK 10.0.26100、WebView2 已安装。Rust 依赖下载需走代理（127.0.0.1:7897）才稳定。
  - 发现并修复缺陷：本机 F12 被其他程序占用，原批量注册快捷键的写法一启动即崩溃；已改为逐个注册、单个冲突仅标记提示（D-023），本验证程序老板键默认改为 F9（D-024）。
  - 发现并修复缺陷：拖动窗口无效——原因是在 `capabilities/default.json` 缺少 `core:window:allow-start-dragging` 权限，标题栏拖拽被权限系统拦截；已补上该权限并重新构建验证。
  - Windows 实际运行验证需产品负责人在真实 Windows 电脑上完成，步骤见 `capability-check/README.md`。
