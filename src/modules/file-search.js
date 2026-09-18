// 文件搜索模块 (VSCode 风格 Ctrl+P)
// 扫描工作区目录，提供模糊文件名搜索和键盘导航

const __fs_invoke = window.__TAURI__?.core?.invoke;

let __fs_dialog, __fs_inputEl, __fs_listEl;
let __fs_allFiles = [];
let __fs_filteredFiles = [];
let __fs_selectedIndex = -1;
let __fs_workspaceFolder = null;
// 记录鼠标真实位置，用于过滤 DOM 重绘触发的合成 mouseenter（光标未动时不应自动选中）
let __fs_lastMouseX = -1;
let __fs_lastMouseY = -1;

const FILE_ICON = '<svg class="fs-icon" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M14 3H7a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h10a2 2 0 0 0 2-2V8z"/><polyline points="14 3 14 8 19 8"/></svg>';

function initFileSearch() {
  __fs_dialog = document.getElementById('file-search-dialog');
  __fs_inputEl = document.getElementById('file-search-input');
  __fs_listEl = document.getElementById('file-search-list');
  if (!__fs_dialog || !__fs_inputEl || !__fs_listEl) return;

  const closeBtn = document.getElementById('file-search-close');
  if (closeBtn) closeBtn.addEventListener('click', fsCloseDialog);

  __fs_dialog.addEventListener('click', (e) => {
    if (e.target === __fs_dialog) fsCloseDialog();
  });

  // 阻止所有键盘事件冒泡到编辑器
  __fs_dialog.addEventListener('keydown', (e) => { e.stopPropagation(); });

  // 全局追踪鼠标真实移动位置（区分真实 hover 与 DOM 重绘产生的合成事件）
  document.addEventListener('mousemove', (e) => {
    __fs_lastMouseX = e.clientX;
    __fs_lastMouseY = e.clientY;
  });

  __fs_inputEl.addEventListener('input', () => {
    const q = __fs_inputEl.value.trim().toLowerCase();
    if (!q) {
      __fs_filteredFiles = __fs_allFiles.slice(0, 50);
    } else {
      __fs_filteredFiles = __fs_allFiles.filter(f => f.name.toLowerCase().includes(q));
    }
    __fs_selectedIndex = -1;
    fsRenderList();
  });

  __fs_inputEl.addEventListener('keydown', (e) => {
    if (e.key === 'Escape') { e.preventDefault(); e.stopPropagation(); fsCloseDialog(); return; }
    const len = __fs_filteredFiles.length;
    if (!len) return;
    if (e.key === 'ArrowDown') {
      e.preventDefault();
      __fs_selectedIndex = __fs_selectedIndex < 0 ? 1 : (__fs_selectedIndex + 1) % len;
      fsRenderList(); fsScrollToSelected(); return;
    }
    if (e.key === 'ArrowUp') {
      e.preventDefault();
      __fs_selectedIndex = __fs_selectedIndex < 0 ? len - 1 : (__fs_selectedIndex - 1 + len) % len;
      fsRenderList(); fsScrollToSelected(); return;
    }
    if (e.key === 'Enter') {
      e.preventDefault();
      const idx = __fs_selectedIndex >= 0 ? __fs_selectedIndex : 0;
      if (__fs_filteredFiles[idx]) fsOpenFile(__fs_filteredFiles[idx]);
      return;
    }
  });
}

function fsScrollToSelected() {
  const items = __fs_listEl.querySelectorAll('.file-search-item');
  if (items[__fs_selectedIndex]) items[__fs_selectedIndex].scrollIntoView({ block: 'nearest' });
}

function fsRenderList() {
  if (!__fs_listEl) return;
  __fs_listEl.innerHTML = __fs_filteredFiles.map((f, i) => {
    const cls = i === __fs_selectedIndex ? 'file-search-item selected' : 'file-search-item';
    return `<div class="${cls}" data-index="${i}">
      ${FILE_ICON}
      <span>${f.name}</span>
      <span class="fs-path">${f.relativePath || ''}</span>
    </div>`;
  }).join('');

  __fs_listEl.querySelectorAll('.file-search-item').forEach(el => {
    el.addEventListener('click', () => {
      const idx = parseInt(el.dataset.index, 10);
      if (__fs_filteredFiles[idx]) fsOpenFile(__fs_filteredFiles[idx]);
    });
    el.addEventListener('mouseenter', (e) => {
      // 光标坐标未变化说明是列表重绘触发的合成事件，忽略；仅响应真实鼠标移动
      if (e.clientX === __fs_lastMouseX && e.clientY === __fs_lastMouseY) return;
      const idx = parseInt(el.dataset.index, 10);
      if (idx !== __fs_selectedIndex) { __fs_selectedIndex = idx; fsRenderList(); }
    });
  });
}

function fsOpenFile(file) {
  if (window.editor && window.editor.openFilePath) {
    window.editor.openFilePath(file.path);
  }
  fsCloseDialog();
}

function fsCloseDialog() {
  if (__fs_dialog) __fs_dialog.classList.add('hidden');
  if (__fs_inputEl) __fs_inputEl.value = '';
  if (__fs_listEl) __fs_listEl.innerHTML = '';
  __fs_selectedIndex = -1;
  if (window.editor && window.editor.cm) window.editor.cm.focus();
}

async function fsScanDir(dirPath, result, rootDir) {
  if (!__fs_invoke) return;
  try {
    const entries = await __fs_invoke('list_dir', { path: dirPath });
    for (const entry of entries) {
      if (!entry.is_dir) {
        const ext = entry.name.split('.').pop().toLowerCase();
        if (['md', 'markdown', 'txt'].includes(ext)) {
          const relativePath = entry.path.startsWith(rootDir)
            ? entry.path.slice(rootDir.length).replace(/^[/\\]/, '')
            : entry.name;
          result.push({ name: entry.name, path: entry.path, relativePath });
        }
      }
    }
    for (const entry of entries) {
      if (entry.is_dir) await fsScanDir(entry.path, result, rootDir);
    }
  } catch (e) {}
}

async function fsScanWorkspace(dir) {
  __fs_allFiles = [];
  const mdFiles = [];
  await fsScanDir(dir, mdFiles, dir);
  mdFiles.sort((a, b) => a.name.localeCompare(b.name));
  __fs_allFiles = mdFiles;
  __fs_filteredFiles = __fs_allFiles.slice(0, 50);
  __fs_selectedIndex = -1;
  fsRenderList();
}

function openFileSearchDialog() {
  if (!__fs_dialog) initFileSearch();
  if (!__fs_dialog) return;
  __fs_dialog.classList.remove('hidden');
  if (__fs_inputEl) { __fs_inputEl.value = ''; __fs_inputEl.focus(); }

  const ws = window.editor && window.editor.workspaceFolder;
  if (ws && ws !== __fs_workspaceFolder) {
    __fs_workspaceFolder = ws;
    fsScanWorkspace(ws);
  } else if (ws) {
    fsScanWorkspace(ws);
  } else {
    const tab = window.editor && window.editor.activeTab;
    if (tab && tab.filePath) {
      const dir = tab.filePath.replace(/[/\\][^/\\]*$/, '');
      __fs_workspaceFolder = dir;
      fsScanWorkspace(dir);
    } else {
      __fs_allFiles = [];
      __fs_filteredFiles = [];
      fsRenderList();
    }
  }
}