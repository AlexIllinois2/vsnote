# vsnote

🌐 [简体中文](README.md) | **English**

<div align="center">

![vsnote](icon.png)

</div>

<p align="center">
  <b>⚡Lightweight &nbsp;·&nbsp; 🚀Blazing Fast &nbsp;·&nbsp; ✨Minimal &nbsp;·&nbsp; 🆓<font color="#16a34a">Free & Open Source</font></b>
  <br>
  <b>A clean, fast Markdown editor (enhanced fork of TizuMark)</b>
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

<p align="center" style="font-size:1.15em"><b>~7MB installer · &lt;50MB RAM · Double-click to launch</b></p>

> This project is a fork of [TizuMark](https://github.com/tizuio/TizuMark) by [@tizu](https://github.com/tizuio), with additional features and adjustments on top — see [Changes vs. the original project](#-changes-vs-the-original-project). Many thanks to the original author.

---

## Why vsnote?

<p align="center"><b>Open. Read. Edit. Export. That's it.</b></p>

The world isn't short of Markdown editors. But most fall into one of two camps: heavyweight monsters that hog hundreds of MB, or toys too barebones for real work. vsnote lands right in the sweet spot.

| Pain Point | The Usual Way | ✨ vsnote |
|---|---|---|
| 🐢 **Too heavy** | Hundreds of MB RAM, multi-GB install, slow boot | **Rust engine. ~7MB installer, <50MB RAM, up in a second** |
| 👯 **Split views** | Source on one side, render on the other — scroll and squint | **Live preview, WYSIWYG, editor/preview scroll auto-synced** |
| 🌀 **Long docs** | Endless scrolling, can't find that one section | **Auto-generated outline from headings, one click to any chapter** |
| 🧩 **Math & diagrams** | Install LaTeX, switch tools, export, paste, repeat | **Built-in KaTeX & Mermaid — math and diagrams render from code** |

---

## Key Features

> vsnote lands right in the sweet spot between heavyweight IDEs and barebones notepads — nailing the things writers care about most.

- ⚡ **Blazing fast & lightweight**: Built on **Rust + Tauri v2** (native WebView), ~**7MB** installer, **<50MB** RAM, launches in under a second — 4/5 less memory than Electron apps.
- 👁️ **Live WYSIWYG preview**: Write on the left, see it render on the right. Editor and preview scroll auto-synced — no window switching.
- 🧭 **Smart outline navigation**: Auto-parses heading hierarchy, one click to any chapter. Never get lost in long docs.
- 📐 **Built-in KaTeX math**: Inline formulas, display blocks, matrices, equation systems — papers, notes, formulas all handled.
- 📊 **Built-in Mermaid diagrams**: Flowcharts, sequence diagrams, Gantt charts, class diagrams, state diagrams… **draw with code, auto-adapts to light/dark theme**.
- 🖼️ **Paste-to-insert images**: Screenshots or drag-drop, auto-dedup via MD5. Store in `assets/` or inline as Base64. Relative paths resolve identically in preview and export.
- 📤 **Multi-format export**: Standalone HTML (full styling, fully offline), high-res PNG long screenshot, PDF (system print dialog) — all preserve dark/light theme.
- ⌨️ **Fully customizable shortcuts**: Every single shortcut can be rebound in `File → Keyboard Shortcuts`. The VSCode preset is the default, matching your muscle memory.
- 📂 **Multi-tab + workspace**: Edit multiple files at once, drag-drop batch open, folder workspace with sidebar file tree (drag-to-move & context menu), `.md` file association.
- 🚀 **Huge-doc smooth preview**: Sliding-window + virtual rendering for documents with tens of thousands of lines — never lags. External file changes auto-detected with reload prompt.
- 🎨 **Deep personalization**: Light / Dark / Follow System; 5 color schemes (Default / Sunset / Forest / Nord / Dusk), 2 font schemes (Sans / Serif), plus custom font import (`.ttf` / `.otf` / `.woff`) with separate editor & preview fonts.
- 🌐 **Bilingual UI**: Chinese / English interface toggle at any time.
- 💾 **Session restore**: Reopens tabs, folder workspace, and expanded directories from last session.

---

## 🔀 Changes vs. the Original Project

This project is based on [TizuMark v1.1.0](https://github.com/tizuio/TizuMark). Main changes:

### New Features

- ⌨️ **VSCode shortcuts by default**: `Ctrl+P` quick-open (list wraps around), `Ctrl+B` sidebar toggle, `Alt+↑/↓` move line/selection, etc. Other presets and full rebinding remain available in `File → Keyboard Shortcuts`.
- 📂 **File tree enhancements**:
  - Context menu (new / rename / delete, etc.);
  - **Drag-to-move files / folders** (name conflicts auto-suffixed with `(1)`, open tabs re-pathed automatically, expanded state migrated);
  - **Reveal current file** in the file tree with one click.
- 📝 **Auto-append extension**: Newly created files without an extension get `.md` appended automatically.
- 🔗 **Click-to-open links in the editor**: http/https links in the editor are directly clickable.
- 📭 **Empty state after closing all tabs**: No forced auto-created blank document — the editor switches to read-only with a "no file open" hint.

### Behavior Changes

- 🚫 **System tray removed**: Tray icon and "minimize to tray" options are gone; window close behavior is back to "ask every time".

### Engineering & Build

- 🏗️ **Three-platform CI releases**: Pushing a `v*` tag automatically builds **Windows (msi / NSIS) + Linux (deb / rpm / AppImage) + macOS (dmg)** and publishes them to the GitHub Release (no test runs).
- 🐛 **Bug fixes**: Hover mis-selection after file-search (`Ctrl+P`) list redraws, `Esc` inserting a stray `·`, arrow-key shortcut recording not working, and more.
- 🎨 **Color scheme & UI polish**.

---

## Feature Matrix

| 📝 Editing | 👁️ Preview | 📤 Export |
|---|---|---|
| Full GFM syntax highlighting | Live scroll-synced preview | Standalone HTML (with full styling) |
| 100+ language code highlighting | KaTeX math rendering | High-res long screenshot PNG |
| Find & replace with regex | Mermaid flowcharts, sequences, Gantt, state | Export PDF (`Ctrl+P` print dialog) |
| Cross-file search (`Ctrl+H`) / Global search (`Ctrl+Shift+F`) | Emoji shortcodes (`:rocket:` → 🚀) | Dark / light theme preserved |
| Collapsible format toolbar | Image viewer (drag-pan + scroll-zoom) | 100% offline |
| Auto bracket & quote pairing | Adaptive image sizing | CJK Emoji support |
| Image paste, auto-dedup (MD5) | Clickable task-list checkboxes | Custom image asset path |
| Insert menu (tables, callouts, TOC) | Auto width/height on image insert | |
| Click-to-open links in editor | | |

| ⚡ Productivity | 🎨 Style | 🔧 Power |
|---|---|---|
| Outline sidebar — jump anywhere | Light / Dark / Follow System | CLI file opening |
| Folder workspace (sidebar file tree) | Font size, line height, max width | File association: .md, .markdown |
| Cross-file search (regex + directory) | Tab width, word wrap toggle | Recently opened files list |
| Tab drag-to-reorder | Code block line numbers / auto-wrap | Unsaved-state markers + close prompt |
| Drag & drop, batch file open | Fully rebindable shortcuts | Close behavior options |
| File tree drag-to-move + context menu | Import custom fonts (editor & preview separately) | Status bar word & char count |
| Reveal current file in tree | 5 color schemes + 2 font schemes | External-change detection & reload prompt |
| Free-drag split pane ratio | 中文 / English UI toggle | Three-platform CI packaging |
| Find in preview (regex) + copy as HTML | Frameless custom window controls | |
| Soft line break toggle | Silent update check on startup | |
| Session restore (tabs & workspace) | | |

---

## Screenshots

<p align="center">
  <img src="screenshots/01-main.png" alt="Main Interface" width="45%">
  <img src="screenshots/02-tabs.png" alt="Tabs & Scrollable Tab Bar" width="45%">
  <br>
  <img src="screenshots/03-math.png" alt="KaTeX Math Rendering" width="45%">
  <img src="screenshots/04-mermaid.png" alt="Mermaid Diagram Rendering" width="45%">
  <br>
  <img src="screenshots/05-code.png" alt="Code Syntax Highlighting" width="45%">
  <img src="screenshots/06-theme.png" alt="Dark Theme" width="45%">
  <br>
  <img src="screenshots/07-font.png" alt="Font Scheme" width="45%">
  <img src="screenshots/08-shortcuts.png" alt="Customizable Shortcuts" width="45%">
  <br>
  <img src="screenshots/09-image.png" alt="Image Insert & Settings" width="45%">
  <img src="screenshots/10-large.png" alt="Smooth Large-Document Preview" width="45%">
  <br>
  <img src="screenshots/11-export.png" alt="Export Menu" width="45%">
  <img src="screenshots/12-find.png" alt="Find & Replace" width="45%">
  <br>
  <img src="screenshots/13-workspace.png" alt="Folder Workspace" width="45%">
  <img src="screenshots/14-callout.png" alt="Callout Rendering" width="45%">
</p>

---

## Quick Start

### Download

| Platform | Status |
|----------|--------|
| Windows | ✅ Supported (msi / NSIS installer) |
| macOS | ✅ Supported (dmg, Apple Silicon) |
| Linux | ✅ Supported (deb / rpm / AppImage) |

<b>Visit the release page to download:</b>

<a href="https://github.com/AlexIllinois2/vsnote/releases"><img src="https://img.shields.io/badge/⬇_Download_from_GitHub-181717?style=for-the-badge&logo=github&logoColor=white" alt="Download from GitHub"></a>

> After pushing a `v*` tag, CI automatically builds all three platforms and publishes them to the Release page above.

> On first launch, the user guide opens automatically. You can also find it in `Help → User Guide` anytime.

> 🔔 **On first install or after a version upgrade, vsnote automatically opens the User Guide and the full syntax demo (demo.md) to help you get started with new features.**

📖 Want to see every syntax in action? Open [demo.md](demo.md) for a full syntax showcase.

### Shortcuts

| Shortcut | Action | Shortcut | Action |
|---|---|---|---|
| `Ctrl+N` | New File | `Ctrl+W` | Close Tab |
| `Ctrl+O` | Open File | `Ctrl+F` | Find |
| `Ctrl+S` | Save File | `Ctrl+H` | Find & Replace |
| `Ctrl+B` | Toggle Sidebar | `Ctrl+I` | Italic |
| `Ctrl+P` | Quick Open File | `Alt+↑/↓` | Move Line/Selection |

> The VSCode preset is the default; all shortcuts are customizable via `File → Keyboard Shortcuts`

### Build from Source

```bash
git clone https://github.com/AlexIllinois2/vsnote.git
cd vsnote
npm install
npm run dev      # dev mode
npm run build    # production build
```

---

## 🛠 Architecture

```
┌──────────────────────────────────────────────────┐
│                 Frontend (WebView)                 │
│   CodeMirror 5  │  highlight.js  │    KaTeX       │
│     Mermaid     │  html2canvas   │     ...        │
└──────────────┬───────────────────────────────────┘
               │ IPC (ipc: / tauri:)
┌──────────────┴───────────────────────────────────┐
│                  Backend (Rust)                    │
│     Tauri 2.5    │    pulldown-cmark               │
│     File I/O     │    Native Dialogs               │
└──────────────┬───────────────────────────────────┘
               │
        ┌──────┴──────┐
        │  OS Native   │
        │ Win / Mac /  │
        │   Linux      │
        └─────────────┘
```

> Tauri v2 uses the OS native WebView — ~7MB installer, ~1/5 the footprint of Electron-based alternatives. **Installers for all three platforms are built and published by CI automatically.**

---

## FAQ

<details open>
<summary><b>Is vsnote really free?</b></summary>

Yes. Free forever, open source, no feature paywalls.
</details>

<details open>
<summary><b>What's the relationship between vsnote and TizuMark?</b></summary>

vsnote is an enhanced fork of [TizuMark](https://github.com/tizuio/TizuMark). It keeps everything from the original and adds the VSCode shortcut preset, file tree drag-to-move & context menu, three-platform CI releases, and more — see [Changes vs. the original project](#-changes-vs-the-original-project).
</details>

<details open>
<summary><b>How do I restore default settings?</b></summary>

Click "Restore Default" in `File → Settings` or `File → Keyboard Shortcuts`.
</details>

<details open>
<summary><b>What file formats are supported?</b></summary>

`.md`, `.markdown`, `.txt`. More coming.
</details>

<details open>
<summary><b>How do I report a bug or request a feature?</b></summary>

- [GitHub Issues](https://github.com/AlexIllinois2/vsnote/issues)
- Issues inherited from the original project may also go to [TizuMark Issues](https://github.com/tizuio/TizuMark/issues)
</details>

---

## Acknowledgements

- Thanks to the original author [@tizu](https://github.com/tizuio) and the [TizuMark](https://github.com/tizuio/TizuMark) project — everything in vsnote is built on top of it. If you find this useful, please star the original project too.
- Original project home: [GitHub](https://github.com/tizuio/TizuMark) · [Gitee](https://gitee.com/tizu/tizu-mark)

---

## License

Copyright (c) 2024-2026 TizuMark

This software is released under the [GNU General Public License v3.0](LICENSE). You are free to use, modify, and distribute it, but derivative works must remain under GPL v3. As a derivative work of TizuMark, vsnote remains open source under the same GPL v3.

Bundled open-source components are licensed under their respective terms. See `Help → About` in the app for details.

---

<p align="center">
  <b>✨ vsnote — Stupidly light. Exactly fast enough.</b><br><br>
  <a href="https://github.com/AlexIllinois2/vsnote/releases"><img src="https://img.shields.io/badge/⬇_Download_from_GitHub-black?style=for-the-badge&logo=github" alt="GitHub Download"></a>
  <br><br>
  <a href="https://github.com/AlexIllinois2/vsnote">⭐ GitHub Star</a>
  &nbsp;·&nbsp;
  <a href="https://github.com/AlexIllinois2/vsnote/issues">🐛 Report Bug</a>
  &nbsp;·&nbsp;
  <a href="https://github.com/tizuio/TizuMark">Original Project: TizuMark</a>
</p>
