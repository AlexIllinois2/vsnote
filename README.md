# vsnote

🌐 **简体中文** | [English](README.en.md)

<div align="center">

![vsnote](icon.png)

</div>

<p align="center">
  <b>⚡轻量 &nbsp;·&nbsp; 🚀高速 &nbsp;·&nbsp; ✨简洁 &nbsp;·&nbsp; 🆓<font color="#16a34a">开源免费</font></b>
  <br>
  <b>一个纯粹、快速的 Markdown 编辑器（基于 TizuMark 的增强分支）</b>
</p>

<p align="center">
  <img src="https://img.shields.io/badge/Version-1.1.0-blue" alt="Version">
  <img src="https://img.shields.io/badge/Windows-7%2B-brightgreen" alt="Windows">
  <img src="https://img.shields.io/badge/macOS-10.15%2B-brightgreen" alt="macOS">
  <img src="https://img.shields.io/badge/Linux-x64-brightgreen" alt="Linux">
  <img src="https://img.shields.io/badge/Tauri-2.x-orange" alt="Tauri">
  <img src="https://img.shields.io/badge/Rust-1.77%2B-black" alt="Rust">
  <img src="https://img.shields.io/badge/License-GPL--3.0-blue" alt="License">
</p>

<p align="center" style="font-size:1.15em"><b>安装包仅 ~7MB · 内存占用 < 50MB · 双击即开</b></p>

> 本项目 fork 自 [TizuMark](https://github.com/tizuio/TizuMark)（原作者 [@tizu](https://github.com/tizuio)），在其基础上做了若干功能增强与调整，详见下文[「相对原项目的修改」](#-相对原项目的修改)。感谢原作者的优秀工作。

---

## 为什么你需要 vsnote？

<p align="center"><b>打开、查看、编辑、导出——就是这么简单。</b></p>

市面上不缺 Markdown 编辑器，但大多走向两个极端：要么是动辄几百 MB 的"重型武器"，要么功能简陋得无法日常使用。vsnote 卡在中间，刚好合适——把写作者最在意的几件事做到极致。

| 痛点 | 常见方案 | ✨ vsnote |
|---|---|---|
| 🐢 **太重** | 几百 MB 内存、几个 G 安装包，启动等半天 | **Rust 原生引擎，安装包 ~7MB、内存 <50MB，双击一秒即开** |
| 👯 **分屏** | 源码和渲染各一屏，看文档得来回切窗口 | **实时预览、所见即所得，编辑/预览滚动自动同步** |
| 🌀 **长文档** | 滚轮滚到手酸，回头找段落得翻半天 | **大纲自动解析标题层级，一键跳转任意章节** |
| 🧩 **画公式/图** | 装 LaTeX、切画图工具、导出再粘贴 | **内置 KaTeX 与 Mermaid，公式与图表代码即渲染** |

---

## ✨ 核心特色

> vsnote 卡在"重型 IDE"和"简陋记事本"之间，刚好合适——把写作者最在意的几件事做到极致。

- ⚡ **极速轻量**：基于 **Rust + Tauri v2**（系统原生 WebView），安装包约 **7MB**、内存占用 **< 50MB**，双击一秒即开，比 Electron 类应用省 4/5 内存。
- 👁️ **实时预览所见即所得**：左边写、右边渲染，编辑/预览滚动自动同步，无需来回切换窗口。
- 🧭 **智能大纲导航**：自动解析标题层级，一键跳转任意章节，长文档永不迷路。
- 📐 **内置 KaTeX 数学公式**：行内公式、独立公式块、矩阵、方程组全支持——写论文、做笔记、记公式直接搞定。
- 📊 **内置 Mermaid 图表**：流程图、时序图、甘特图、类图、状态图……**用代码画图，自动跟随明暗主题切换配色**。
- 🖼️ **图片粘贴即插入**：截图/拖拽直接粘贴，支持 assets 目录存储或 Base64 内联，自动去重；导出时相对路径解析与预览完全一致。
- 📤 **多种导出**：HTML 单文件（完整样式、完全离线）、高清长图 PNG、PDF（系统打印对话框），均保留暗黑/亮色主题样式。
- ⌨️ **快捷键全自定义**：每一条快捷键都能在 `文件 → 快捷键设置` 中改键，默认采用 VSCode 方案，贴合你的肌肉记忆。
- 📂 **多标签页 + 工作区**：同时编辑多个文件、拖拽批量打开、文件夹工作区、文件树拖拽移动与右键菜单、`.md` 文件关联。
- 🚀 **海量文档流畅预览**：数万行超大文档采用滑动窗口 + 虚拟渲染，只渲染当前阅读区域，编辑器永不卡顿；外部磁盘变更自动检测并提示重新加载，多工具协作不丢稿。
- 🎨 **深度个性化**：亮色 / 暗黑 / 跟随系统一键切换；内置 5 套配色方案（基准 / 暖橙 / 翠林 / 极夜 / 暮紫）、2 套字体方案（简约 / 印刷），并可在 `文件 → 设置 → 自定义字体` 导入本地 `.ttf` / `.otf` / `.woff` 字体，编辑器与预览分别指定。
- 🌐 **中 / 英 界面语言一键切换**：内置完整双语界面，随时在设置中切换，新手与外文写作都友好。
- 💾 **会话记忆**：重启自动恢复上次打开的标签页、文件夹工作区与展开目录，打开即回到上次状态。

---

## 🔀 相对原项目的修改

本项目基于 [TizuMark v1.1.0](https://github.com/tizuio/TizuMark) 修改而来，主要变化：

### 功能

- ⌨️ **默认 VSCode 快捷键方案**：`Ctrl+P` 快速打开文件（列表循环）、`Ctrl+B` 侧边栏开关、`Alt+↑/↓` 上下移动行/选区等，可在 `文件 → 快捷键设置` 中切换回其他方案或继续自定义。
  - **拖拽移动文件 / 文件夹**（目标重名自动加 `(1)` 后缀，已打开标签页路径自动同步，目录展开状态一并迁移）；
  - **一键定位当前文件**在文件树中的位置。
- 📝 **新建文件自动补全后缀**：新建无扩展名的文件时自动追加 `.md`。
- 🔗 **编辑区点击打开链接**：编辑器中的 http/https 链接可直接点击访问。

### 行为调整

- 🚫 **移除系统托盘**：去掉托盘图标与"最小化到托盘"选项，窗口关闭行为回归"每次询问"。

---

## 功能一览

| 📝 编辑 | 👁️ 预览 | 📤 导出 |
|---|---|---|
| GFM 完整语法高亮 | 实时同步滚动 | 导出 HTML 单文件（完整样式） |
| 代码块 100+ 语言着色 | KaTeX 数学公式渲染 | 导出高清长图 PNG |
| 查找替换（支持正则） | Mermaid 流程图/时序图/甘特图/状态图 | 导出 PDF（`Ctrl+P` 系统打印） |
| 跨文件搜索（`Ctrl+H`）/ 全局搜索（`Ctrl+Shift+F`） | Emoji 短代码 (`:rocket:` → 🚀) | 保留暗黑/亮色主题样式 |
| 可折叠格式工具栏 | 图片查看器（拖拽平移 + 滚轮缩放） | 完全离线，无需联网 |
| 自动补全括号、引号 | 自适应图片尺寸 | 中英文 Emoji 完美适配 |
| 图片粘贴插入、自动去重（MD5） | 任务列表预览可点击勾选 | 自定义图片存储路径 |
| 插入菜单（表格/提示块/目录等） | 图片自动带宽高属性 | |
| 编辑区点击链接直接打开 | | |

| ⚡ 效率 | 🎨 个性化 | 🔧 专业 |
|---|---|---|
| 大纲导航一键跳转 | 亮色 / 暗黑 / 跟随系统 | CLI 命令行打开文件 |
| 文件夹工作区（侧边栏文件树） | 字体大小/行高/内容宽度可调 | 文件关联 .md / .markdown |
| 跨文件搜索（正则 + 目录搜索） | Tab 宽度 / 自动换行开关 | 最近打开文件列表 |
| 标签页拖拽排序 | 代码块行号 / 代码自动换行开关 | 未保存状态标记 + 关闭提醒 |
| 拖拽、批量打开文件 | 全套快捷键可自定义 | 关闭行为可选 |
| 文件树拖拽移动 + 右键菜单 | 导入自定义字体（编辑器/预览分别指定） | 状态栏实时字数统计 |
| 一键定位当前文件 | 5 套配色方案 + 2 套字体方案 | 外部变更检测 + 重新加载提示 |
| 编辑/预览分屏比例自由拖拽 | 中 / 英 界面语言一键切换 | 三平台 CI 自动打包发布 |
| 预览内查找（支持正则）+ 复制为 HTML | 自绘无边框窗口（自定义最小化/最大化/关闭） | |
| 软换行（回车即换行）开关 | 静默检查更新，有新版自动提示 | |
| 会话记忆（重启恢复标签页/文件夹） | | |

---

## 界面预览

<p align="center">
  <img src="screenshots/01-main.png" alt="主界面全貌" width="45%">
  <img src="screenshots/02-tabs.png" alt="多标签与标签栏滚动" width="45%">
  <br>
  <img src="screenshots/03-math.png" alt="KaTeX 数学公式渲染" width="45%">
  <img src="screenshots/04-mermaid.png" alt="Mermaid 图表渲染" width="45%">
  <br>
  <img src="screenshots/05-code.png" alt="代码语法高亮" width="45%">
  <img src="screenshots/06-theme.png" alt="暗色主题" width="45%">
  <br>
  <img src="screenshots/07-font.png" alt="字体方案" width="45%">
  <img src="screenshots/08-shortcuts.png" alt="快捷键自定义设置" width="45%">
  <br>
  <img src="screenshots/09-image.png" alt="图片插入与设置" width="45%">
  <img src="screenshots/10-large.png" alt="超大文档流畅预览" width="45%">
  <br>
  <img src="screenshots/11-export.png" alt="导出菜单" width="45%">
  <img src="screenshots/12-find.png" alt="查找替换" width="45%">
  <br>
  <img src="screenshots/13-workspace.png" alt="文件夹工作区" width="45%">
  <img src="screenshots/14-callout.png" alt="提示框渲染" width="45%">
</p>

---

## 快速开始

### 下载安装

| 平台 | 状态 |
|------|------|
| Windows | ✅ 已支持（msi / NSIS 安装包） |
| macOS | ✅ 已支持（dmg，Apple Silicon） |
| Linux | ✅ 已支持（deb / rpm / AppImage） |

<b>请打开产品发布页面下载：</b>

<a href="https://github.com/AlexIllinois2/vsnote/releases"><img src="https://img.shields.io/badge/⬇_从_GitHub_下载-181717?style=for-the-badge&logo=github&logoColor=white" alt="从 GitHub 下载"></a>

> 推送 `v*` 标签后，CI 会自动构建三平台安装包并发布到上述 Release 页面。

> 首次打开会自动展示使用说明，也可在 `帮助 → 使用说明` 中随时查看。

> 🔔 **首次安装 / 版本升级后，vsnote 会自动打开使用说明和完整语法演示 demo.md，帮你快速上手新功能。**

📖 想一眼看全所有语法效果？打开 [demo.md](demo.md) 查看完整语法演示。

### 快捷键速览

| 快捷键 | 功能 | 快捷键 | 功能 |
|---|---|---|---|
| `Ctrl+N` | 新建文件 | `Ctrl+W` | 关闭标签 |
| `Ctrl+O` | 打开文件 | `Ctrl+F` | 查找 |
| `Ctrl+S` | 保存文件 | `Ctrl+H` | 查找替换 |
| `Ctrl+B` | 开关侧边栏 | `Ctrl+I` | 斜体 |
| `Ctrl+P` | 快速打开文件 | `Alt+↑/↓` | 移动行/选区 |

> 默认采用 VSCode 方案，所有快捷键可在 `文件 → 快捷键设置` 中自定义

### 从源码构建

```bash
git clone https://github.com/AlexIllinois2/vsnote.git
cd vsnote
npm install
npm run dev      # 开发模式
npm run build    # 构建发布版本
```

---

## 🛠 技术架构

```
┌──────────────────────────────────────────────────┐
│                  前端 (WebView)                   │
│   CodeMirror 5  │  highlight.js  │    KaTeX      │
│     Mermaid     │  html2canvas   │     ...       │
└──────────────┬───────────────────────────────────┘
               │ IPC (ipc: / tauri:)
┌──────────────┴───────────────────────────────────┐
│                  后端 (Rust)                      │
│     Tauri 2.5    │    pulldown-cmark              │
│     文件 I/O     │     系统对话框                  │
└──────────────┬───────────────────────────────────┘
               │
        ┌──────┴──────┐
        │  OS Native   │
        │ Win / Mac /  │
        │   Linux      │
        └─────────────┘
```

> Tauri v2 使用系统原生 WebView，安装包仅 ~7MB，内存占用不到 Electron 类应用的 1/5。**三平台安装包均由 CI 自动构建发布。**

---

## 常见问题

<details open>
<summary><b>vsnote 是免费的吗？</b></summary>

是的，永久免费且开源。基础功能没有任何限制。
</details>

<details open>
<summary><b>vsnote 和 TizuMark 是什么关系？</b></summary>

vsnote 是基于 [TizuMark](https://github.com/tizuio/TizuMark) 的增强分支，保留了原项目全部能力，并增加了 VSCode 快捷键方案、文件树拖拽移动/右键菜单、三平台 CI 自动发布等修改，详见[「相对原项目的修改」](#-相对原项目的修改)。
</details>

<details open>
<summary><b>如何恢复默认设置？</b></summary>

在 `文件 → 设置` 或 `文件 → 快捷键设置` 中点击「恢复默认」按钮即可。
</details>

<details open>
<summary><b>支持哪些文件格式？</b></summary>

支持 `.md`、`.markdown`、`.txt` 文件。更多格式支持计划中。
</details>

<details open>
<summary><b>如何反馈问题或建议？</b></summary>

- [GitHub Issues](https://github.com/AlexIllinois2/vsnote/issues)
- 原项目相关问题也可前往 [TizuMark Issues](https://github.com/tizuio/TizuMark/issues)
</details>

---

## 致谢

- 感谢原作者 [@tizu](https://github.com/tizuio) 与 [TizuMark](https://github.com/tizuio/TizuMark) 项目——vsnote 的一切基础都来自它，如果觉得好用，请也给原项目点个 Star。
- 原项目主页：[GitHub](https://github.com/tizuio/TizuMark) · [Gitee](https://gitee.com/tizu/tizu-mark)

---

## 许可证

Copyright (c) 2024-2026 TizuMark

本软件基于 [GNU General Public License v3.0](LICENSE) 开源发布。你可以自由使用、修改和分发，但衍生作品必须延续 GPL v3 协议。vsnote 作为 TizuMark 的衍生作品，同样以 GPL v3 继续开源。

内置开源组件按其各自许可证授权，详见应用内 `帮助 → 关于` 页面。

---

<p align="center">
  <b>✨ vsnote — 轻得不像话，快得刚刚好</b><br><br>
  <a href="https://github.com/AlexIllinois2/vsnote/releases"><img src="https://img.shields.io/badge/⬇_GitHub_下载-black?style=for-the-badge&logo=github" alt="GitHub Download"></a>
  <br><br>
  <a href="https://github.com/AlexIllinois2/vsnote">⭐ GitHub Star</a>
  &nbsp;·&nbsp;
  <a href="https://github.com/AlexIllinois2/vsnote/issues">🐛 反馈问题</a>
  &nbsp;·&nbsp;
  <a href="https://github.com/tizuio/TizuMark">原项目 TizuMark</a>
</p>
