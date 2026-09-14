// UX bugfix 回归测试（本次修复的 6 个交互问题）：
//  1. Insert 键全局禁用（不再触发 CM toggleOverwrite 覆写模式）
//  2. 编辑器内 Delete 走正常删字符逻辑，不再误弹「删除文档」确认框
//     （焦点在 body 时 Delete 对文件树仍生效，保持原设计）
//  3. 点击文件树以外区域清除 _fileTreeCtx（防止残留上下文劫持快捷键）
//  4. 右键文件 → 新建文件(夹)菜单项可用；新建为同级；新建文件后自动打开
//  5. Ctrl+= / Ctrl+- / Ctrl+0 → WebView 整页缩放（Tauri set_webview_zoom）
//  6. globalSearch 默认键迁移 Ctrl+Shift+F → Ctrl+Shift+H（中文输入法 OS 层拦截）
const test = require('node:test');
const assert = require('node:assert');
const { withEditor, delay } = require('./helpers/app-env.cjs');

function dispatchKey(w, target, key, code, keyCode, mods = {}) {
  const evt = new w.KeyboardEvent('keydown', {
    key, code, keyCode, which: keyCode, ctrlKey: !!mods.ctrl, shiftKey: !!mods.shift,
    altKey: !!mods.alt, metaKey: !!mods.meta, bubbles: true, cancelable: true,
  });
  target.dispatchEvent(evt);
  return evt;
}

test('Insert 键在编辑器内被禁用（不触发覆写模式）', async () => {
  await withEditor({ captureInitErr: true }, (w, ed) => {
    const cm = ed.cm;
    const field = cm.display.input.getField();
    cm.focus();
    assert.strictEqual(cm.state.overwrite, false, '前置：初始应为插入模式');
    const evt = dispatchKey(w, field, 'Insert', 'Insert', 45);
    assert.strictEqual(evt.defaultPrevented, true, 'Insert 应被捕获阶段 preventDefault');
    assert.strictEqual(cm.state.overwrite, false, 'Insert 不应切换到覆写模式');
  });
});

test('Insert 键在对话框输入框内同样被禁用', async () => {
  await withEditor({ captureInitErr: true }, (w) => {
    const input = w.document.getElementById('find-input');
    const evt = dispatchKey(w, input, 'Insert', 'Insert', 45);
    assert.strictEqual(evt.defaultPrevented, true, '任意区域的 Insert 均应被拦截');
  });
});

test('编辑器内按 Delete 正常删除字符，不弹「删除文档」确认框', async () => {
  await withEditor({ captureInitErr: true }, (w, ed) => {
    const cm = ed.cm;
    const field = cm.display.input.getField();
    // 模拟用户此前点击过文件树文件（残留右键上下文）
    ed._fileTreeCtx = { path: '/ws/a.md', isDir: false, nodeEl: null };
    let confirmCalls = 0;
    ed.showConfirmDialog = async () => { confirmCalls++; return false; };
    cm.focus();
    cm.setValue('abc');
    cm.setCursor({ line: 0, ch: 0 });
    dispatchKey(w, field, 'Delete', 'Delete', 46);
    assert.strictEqual(confirmCalls, 0, '编辑器内 Delete 不应触发文件树删除确认');
    assert.strictEqual(cm.getValue(), 'bc', 'Delete 应由 CM 正常删除光标后字符');
  });
});

test('焦点不在编辑器时 Delete 仍对文件树选中项生效（原设计保留）', async () => {
  await withEditor({ captureInitErr: true }, async (w, ed) => {
    ed._fileTreeCtx = { path: '/ws/a.md', isDir: false, nodeEl: null };
    let confirmCalls = 0;
    ed.showConfirmDialog = async () => { confirmCalls++; return false; };
    const evt = dispatchKey(w, w.document.body, 'Delete', 'Delete', 46);
    assert.strictEqual(evt.defaultPrevented, true, '文件树 Delete 应被应用接管');
    await delay(20);
    assert.strictEqual(confirmCalls, 1, '焦点在 body 时 Delete 应弹删除确认框');
  });
});

test('点击文件树以外区域清除 _fileTreeCtx；树内点击保留', async () => {
  await withEditor({ captureInitErr: true }, (w, ed) => {
    ed._fileTreeCtx = { path: '/ws/a.md', isDir: false, nodeEl: null };
    w.document.getElementById('editor-wrapper')
      .dispatchEvent(new w.MouseEvent('click', { bubbles: true }));
    assert.strictEqual(ed._fileTreeCtx, null, '树外点击应清除上下文');

    ed._fileTreeCtx = { path: '/ws/a.md', isDir: false, nodeEl: null };
    w.document.getElementById('folder-tree')
      .dispatchEvent(new w.MouseEvent('click', { bubbles: true }));
    assert.ok(ed._fileTreeCtx, '树内点击应保留上下文');
  });
});

test('右键文件时新建文件(夹)菜单项可用', async () => {
  await withEditor({ captureInitErr: true }, (w, ed) => {
    const menu = w.document.getElementById('context-menu-file-tree');
    ed._fileTreeCtx = { path: '/ws/a.md', isDir: false, nodeEl: null };
    ed.updateFileTreeMenuState();
    assert.strictEqual(
      menu.querySelector('[data-action="file-new-file"]').classList.contains('disabled'),
      false, '右键文件时「新建文件」应可用（同级新建）');
    assert.strictEqual(
      menu.querySelector('[data-action="file-new-folder"]').classList.contains('disabled'),
      false, '右键文件时「新建文件夹」应可用（同级新建）');
  });
});

test('右键文件 → 新建文件创建为同级且自动打开', async () => {
  const writes = [];
  await withEditor({
    captureInitErr: true,
    invokeImpl: (cmd, args) => {
      if (cmd === 'write_file') { writes.push(args); return undefined; }
      if (cmd === 'list_dir') return [{ name: 'a.md', is_dir: false }];
      if (cmd === 'read_file') return '';
      return undefined;
    },
  }, async (w, ed) => {
    ed._fileTreeCtx = { path: '/ws/sub/a.md', isDir: false, nodeEl: null };
    ed.showPromptDialog = async () => 'note.md';
    await ed.fileTreeNewFile();
    // 注：app.js 运行在 jsdom vm realm，跨 realm 对象 deepStrictEqual 必败，用逐字段断言
    assert.strictEqual(writes.length, 1, '应恰好创建一个文件');
    assert.strictEqual(writes[0].path, '/ws/sub/note.md', '应在文件同级目录（/ws/sub）创建新文件');
    assert.strictEqual(writes[0].content, '', '新文件内容为空');
    assert.strictEqual(ed.activeTab.filePath, '/ws/sub/note.md',
      '新建文件后应自动打开（activeTab 指向新文件）');
  });
});

test('Ctrl+= / Ctrl+- / Ctrl+0 触发 WebView 缩放（Tauri set_webview_zoom）', async () => {
  const zoomCalls = [];
  await withEditor({
    captureInitErr: true,
    invokeImpl: (cmd, args) => {
      if (cmd === 'plugin:webview|set_webview_zoom') { zoomCalls.push(args && args.value); return undefined; }
      return undefined;
    },
  }, async (w) => {
    const body = w.document.body;
    const evtIn = dispatchKey(w, body, '=', 'Equal', 187, { ctrl: true });
    assert.strictEqual(evtIn.defaultPrevented, true, 'Tauri 环境下 Ctrl+= 应接管缩放');
    const evtOut = dispatchKey(w, body, '-', 'Minus', 189, { ctrl: true });
    assert.strictEqual(evtOut.defaultPrevented, true, 'Tauri 环境下 Ctrl+- 应接管缩放');
    const evtIn2 = dispatchKey(w, body, '=', 'Equal', 187, { ctrl: true });
    assert.strictEqual(evtIn2.defaultPrevented, true, 'Ctrl+= 连续放大应生效');
    const evtReset = dispatchKey(w, body, '0', 'Digit0', 48, { ctrl: true });
    assert.strictEqual(evtReset.defaultPrevented, true, 'Tauri 环境下 Ctrl+0 应接管缩放');
    await delay(20);
    assert.deepStrictEqual(zoomCalls, [1.1, 1, 1.1, 1],
      '放大步进 +0.1、缩小步进 -0.1、Ctrl+0 复位为 1.0');
  });
});

test('globalSearch 旧键 Ctrl+Shift+F 迁移到 Ctrl+Shift+H 且派发生效', async () => {
  await withEditor({ captureInitErr: true }, (w, ed) => {
    // 全新安装（vscode 方案）：默认键已改为 Ctrl+Shift+H
    assert.strictEqual(typeof ed.globalShortcutLookup['Ctrl+Shift+H'], 'function',
      '新环境 globalSearch 应绑定 Ctrl+Shift+H');
    assert.strictEqual(ed.globalShortcutLookup['Ctrl+Shift+F'], undefined,
      'Ctrl+Shift+F（中文输入法 OS 层拦截）不应再绑定');

    // 模拟旧版本保存过快捷键（globalSearch 为旧键）的用户：loadShortcuts 应迁移
    const savedOld = {
      find: { key: 'Ctrl+F', label: '查找替换' },
      crossSearch: { key: 'Ctrl+H', label: '跨文件搜索' },
      globalSearch: { key: 'Ctrl+Shift+F', label: '全局搜索' },
    };
    w.localStorage.setItem('tizumark-shortcuts', JSON.stringify(savedOld));
    w.localStorage.removeItem('tizumark-shortcut-globalsearch-migrated');
    const merged = ed.loadShortcuts();
    assert.strictEqual(merged.globalSearch.key, 'Ctrl+Shift+H', '旧键应迁移到 Ctrl+Shift+H');
    assert.strictEqual(w.localStorage.getItem('tizumark-shortcut-globalsearch-migrated'), '1',
      '迁移应打标记');

    // 已打标记后，用户再次清空不再被强制迁移（尊重用户自定义）
    w.localStorage.setItem('tizumark-shortcuts', JSON.stringify({
      ...savedOld,
      globalSearch: { key: '', label: '全局搜索' },
    }));
    const merged2 = ed.loadShortcuts();
    assert.strictEqual(merged2.globalSearch.key, '', '打过标记后应尊重用户的清空');

    // 派发验证：编辑器有焦点时 Ctrl+Shift+F 打开全局搜索
    ed.shortcuts = merged;
    ed.applyShortcuts();
    const dlg = w.document.getElementById('global-search-dialog');
    assert.ok(dlg.classList.contains('hidden'), '前置：全局搜索对话框应关闭');
    dispatchKey(w, w.document.body, 'F', 'KeyF', 72, { ctrl: true, shift: true });
    assert.ok(!dlg.classList.contains('hidden'), 'Ctrl+Shift+F 应打开全局搜索');
  });
});
