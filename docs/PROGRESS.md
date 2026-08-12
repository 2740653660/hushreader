# 隐阅盒项目进度

最后更新：2026-08-12

## 当前阶段

步骤 3（搜索与下载）开发完成并通过产品负责人 Windows 验收，阶段 3 收尾完成。步骤 4（完整隐蔽阅读）待产品负责人确认后开始。

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
- 步骤 1 完成：Tauri 外壳骨架（透明无边框置顶窗口、全局快捷键、托盘、单实例），并确认本机构建工具链（Node 24 / Rust 1.95 MSVC / VS2022 / SDK 26100 / WebView2）。发现并修复 F12 占用崩溃（D-023/D-024）和拖动权限缺失两个缺陷。
- 步骤 2 完成（2026-08-11）：
  - 彻底移除对 ZTools 的全部依赖：文件读写、文件对话框、存储、悬浮窗、通信全部替换为 Tauri 原生能力（新增 `src/platform.ts` 平台层）。
  - 书架、导入（EPUB/TXT/MOBI）、进度/书签/阅读时长、分类排序、批量操作、全文搜索、备份恢复等旧功能全部保留并在新外壳运行。
  - 悬浮阅读窗口完整保留：透明无边框置顶、拖动/缩放、滚轮与快捷键翻页、鼠标移出隐藏、进度编辑、右键菜单、阅读定时器（Tauri 窗口 + 事件通信实现）。
  - 全局快捷键（老板键 F12、左右方向键翻页）冲突时只标记不阻断启动（D-023）；翻页键与设置联动。
  - 托盘与"关闭到托盘"（D-012）、单实例、开机启动开关（注册表）已实现。
  - 存储方案：书架/设置 localStorage，封面/章节缓存 IndexedDB，均落在应用数据目录（D-025）。
  - 拖拽导入走 Tauri 原生拖放事件拿到真实路径（D-026）。
  - 本机冒烟验证通过：进程正常启动（内存约 41MB）、主窗口创建、关闭窗口进程保留（缩托盘）、F9 临时验证老板键切换主窗口正常、单实例正常、前端类型检查与构建通过、安装包生成成功。
  - 本机构建产物（release）：
    - 主程序：`src-tauri/target/release/app.exe`（约 10.6MB）
    - 安装包：`src-tauri/target/release/bundle/nsis/HushReader_0.1.0_x64-setup.exe`（约 2.5MB）
    - MSI：`src-tauri/target/release/bundle/msi/HushReader_0.1.0_x64_en-US.msi`（约 3.6MB）
- 步骤 2 验收问题修复完成（2026-08-12）：
  - 根因：运行期在 IPC 主线程中创建 WebView2 悬浮窗造成整窗卡死（打开书籍没反应，随后导入/退出全失效）；托盘单击被触发两次造成闪烁；全局快捷键抢占用户打字按键。
  - 修复：悬浮窗改为启动时预创建、运行期只显示/隐藏（D-029）；托盘单击只在"抬起"事件切换（D-028）；整体移除全局快捷键（D-030，悬浮窗内快捷键保留）。
  - 本机验证：点击书籍不再卡死、可重复打开、章节加载正常、导入对话框正常弹出、主线程命令全程存活；`cargo build --release` 与前端构建通过。
  - 待产品负责人用新版构建复验：托盘单击不闪烁、打开书籍正常、打字时左右键恢复、托盘右键退出正常。
  - 产品负责人 Windows 复验通过（2026-08-12）：托盘单击、打开书籍、打字左右键、导入书籍等基本功能全部正常。
- 步骤 2 后续两个想法完成（2026-08-12，D-031/D-032）：
  - 主界面"正在读"角标与"关闭悬浮窗"按钮：悬浮窗激活且未隐藏时，书架中正在读的书卡片显示绿色"正在读"角标（读完角标让位），主界面右上角显示带文字的"关闭悬浮窗"按钮，点击仅隐藏悬浮窗、进度保留；不读书时不显示按钮。
  - 悬浮窗调整手柄：鼠标移到右/下边缘或右下角时显示半透明调整手柄（拖动中持续显示），移开即隐藏，平时保持隐蔽外观；尺寸锁定时不显示手柄。
  - 验证：`vue-tsc` 类型检查与 `vite build` 通过，改动已确认进入构建产物；受限于无桌面浏览器环境，未做运行时 GUI 冒烟验证，等待产品负责人在 Windows 上体验确认。
  - 产品负责人在 Windows 安装产物上体验通过（2026-08-12）："正在读"角标、关闭悬浮窗按钮、悬浮窗调整手柄均正常，两个想法文件已移入 `ideas/已解决/`。
- 编译与打包（2026-08-12，应产品负责人要求在 Windows 本机执行）：
  - `npm run build` 通过；`cargo build --release` 通过（首次全量编译约 3 分钟）。
  - `npx tauri build` 首次失败：Tauri 内部下载 WiX（MSI 打包工具）走直连超时（`timeout: global`）。解决：给命令设置 `HTTPS_PROXY=http://127.0.0.1:7897` 等代理环境变量后重跑成功（手动放置的 `AppData\Local\tauri\WixTools` 未被 Tauri 识别，代理方式才真正生效）。
  - 产物：主程序 `src-tauri/target/release/app.exe`（约 10.3MB）；NSIS 安装包 `src-tauri/target/release/bundle/nsis/HushReader_0.1.0_x64-setup.exe`（约 2.4MB）；MSI `src-tauri/target/release/bundle/msi/HushReader_0.1.0_x64_en-US.msi`（约 3.5MB）。
  - 构建输出有两条 Warn：`__TAURI_BUNDLE_TYPE variable not found in binary`（仅影响将来应用内更新插件定位，第一版阶段 6 才涉及，现无影响）。
- 步骤 3 开发完成（2026-08-12）：
  - 引入 So Novel 后台资源（官方发布包 app.jar、JRE21 runtime、rules 书源规则）到 `src-tauri/resources/sonovel/`（约 182MB，`.gitignore` 忽略，`scripts/prepare-sonovel.ps1` 负责复制）；`tauri.conf.json` bundle.resources 打包。
  - 新增 `src-tauri/src/sonovel.rs`：后台进程管理（隐藏启动/退出清理/异常残留 pid 清理）、端口 7765 起自动探测空闲、按配置生成 config.ini（download-path=书库目录）、等待服务就绪；搜索/书源/下载/local-books 全部 Rust 转发；SSE 下载进度转发为 `sonovel-progress` 事件，下载结束发 `sonovel-fetch-done`；下载一次一本互斥。
  - 数据目录：后台运行目录 `%APPDATA%\com.hushreader.desktop\sonovel\`，书库目录默认 `%APPDATA%\com.hushreader.desktop\books`（可设置修改，修改后重启后台）。
  - 前端：App.vue 启用路由（书架/找书），书架头部新增"找书"入口；公共导入函数抽取为 `src/utils/importer.ts`（书架本地导入与下载入架共用）；新 store `src/stores/download.ts`；新页面 `src/components/FindBook/`（搜索、多书源结果卡片、EPUB/TXT 下载按钮、进度条、失败重试、放弃任务、后台未就绪提示）；设置页新增"书库目录"设置。
  - 移除设备信息上报：从上游 `freeok/so-novel` v1.11.0 源码移除 `ClientReportRepository.report()` 调用及类文件（D-038）；Web 服务只绑定 127.0.0.1（D-039）；用便携 JDK21+Maven（`D:\Ai_Project\tools\`，走 127.0.0.1:7897 代理）在 Windows 本机构建改造后的 app.jar 并替换。
  - 本机冒烟验证通过：后台启动/搜索/EPUB 与 TXT 下载/文件落盘/SSE 进度全链路；改造后 jar 无上报代码、netstat 仅监听 127.0.0.1、局域网 IP 访问失败；`vue-tsc`、`vite build`、`cargo check` 通过。
  - 修复打包期问题：① Tauri 的 `resource_dir()` 返回带 `\\?\` 前缀的扩展路径，直接传给 java 会被命令行解析破坏（`-jar` 报 ClassNotFound），已在 sonovel.rs 统一剥离前缀（`strip_verbatim`）；② JRE 的 AOT 缓存（`classes.jsa`/`classes_nocoops.jsa`，各 12MB）被 Windows Defender 实时扫描锁定导致 tauri build 复制失败，已从捆绑资源剔除（JVM 自动回退，仅首次启动略慢），资源体积随之减小。
  - 端到端验证（app.exe 实机运行）：后台自动拉起（java 进程、仅监听 127.0.0.1）、搜索接口正常、退出后清理、异常残留按 pid 自动清理；`vue-tsc`、`vite build`、`cargo build --release`、`npx tauri build` 全部通过。
  - 构建产物（release，2026-08-12）：主程序 `src-tauri/target/release/app.exe`（约 10.1MB）；NSIS 安装包 `bundle/nsis/HushReader_0.1.0_x64-setup.exe`（约 50.3MB）；MSI `bundle/msi/HushReader_0.1.0_x64_en-US.msi`（约 68.0MB）。
  - 产品负责人 Windows 验收通过（2026-08-12）：找书搜索、下载、自动入架、阅读全流程功能正常（"功能没问题"）。步骤 3 结束。
  - 已推送 GitHub：`github.com/2740653660/hushreader`（2026-08-12）。

## 尚未开始

- 旧版 HushReader（ZTools 插件）书架数据一次性迁移导入（见"当前授权状态"）。
- 测量安装包、启动速度和内存占用（阶段 1 剩余）。
- 干净环境安装/卸载测试（步骤 2 已产出安装包但未测）。
- GitHub 应用内更新（阶段 6）。
- 书源规则更新、书源失效手动切换（阶段 5）。

## 当前授权状态

步骤 1、2、3 均已完成开发并验收通过。下一步为步骤 4（完整隐蔽阅读），待产品负责人确认后开始。注意：旧数据迁移功能（一次性导入旧版书架）尚未实现——方案要求步骤 2 包含它，但实现它需要先确认旧版数据实际存储位置与格式，需要产品负责人提供一台装有旧版 ZTools 插件的环境才能验证；若验收时不需要，将迁移推迟到后续版本并在文档中记录。

## 当前工作

- 步骤 3 已验收通过并推送 GitHub，等待产品负责人决定何时开始步骤 4（完整隐蔽阅读）。
- 本机开发环境说明：Rust 依赖下载需走代理（127.0.0.1:7897）才稳定；Windows 编译按协作规则由产品负责人在本机执行（已产出产物）。
- 构建提示：`tauri build` 前若 JRE 大文件被 Defender 锁定（`拒绝访问`），先删除 `src-tauri/resources/sonovel/runtime/bin/server/classes*.jsa` 再构建。
