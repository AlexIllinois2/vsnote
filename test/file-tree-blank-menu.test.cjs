// 文件树空白处右键菜单（VSCode 行为）回归测试：
//  1. 右键节点 → 节点菜单；右键空白处 → 空白菜单，目标为工作区根目录
//  2. 空白菜单的 新建文件/新建文件夹 可用；剪贴板为空时「粘贴」禁用
//  3. blank 上下文不得让 F2/Delete/Ctrl+X/Ctrl+C 作用到整个工作区
//  4. 未打开工作区时右键空白处不弹菜单
//  5. file-refresh / file-reveal 动作分发正确

const test = require('node:test');
const assert = require('node:assert');
const { withEditor, delay } = require('./helpers/app-env.cjs');

function dispatchCtxMenu(w, target) {
  const evt = new w.MouseEvent('contextmenu', { bubbles: true, cancelable: true });
  target.dispatchEvent(evt);
  return evt;
}

function dispatchKey(w, target, key, code, keyCode, mods = {}) {
  const evt = new w.KeyboardEvent('keydown', {
    key, code, keyCode, which: keyCode, ctrlKey: !!mods.ctrl, shiftKey: !!mods.shift,
    altKey: !!mods.alt, metaKey: !!mods.meta, bubbles: true, cancelable: true,
  });
  target.dispatchEvent(evt);
  return evt;
}

// 在 #folder-tree 下造一个文件节点，返回 { tree, node, blank }
function makeTreeWithNode(w) {
  const tree = w.document.getElementById('folder-tree');
  const node = w.document.createElement('div');
  node.className = 'tree-node tree-file';
  node.dataset.path = 'C:/ws/a.md';
  const row = w.document.createElement('div');
  row.className = 'tree-row';
  node.appendChild(row);
  tree.appendChild(node);
  return { tree, node, blank: tree };
}

test('空白处右键弹出空白菜单，目标为工作区根目录', async () => {
  await withEditor({ captureInitErr: true }, (w, ed) => {
    const { tree } = makeTreeWithNode(w);
    ed.workspaceFolder = 'C:/ws';
    const menu = w.document.getElementById('context-menu-file-tree-blank');
    const nodeMenu = w.document.getElementById('context-menu-file-tree');
    assert.ok(menu, '空白菜单应存在于 DOM');

    dispatchCtxMenu(w, tree);

    assert.ok(!menu.classList.contains('hidden'), '空白处右键应显示空白菜单');
    assert.ok(nodeMenu.classList.contains('hidden'), '不应同时显示节点菜单');
    assert.ok(ed._fileTreeCtx, '应设置文件树上下文');
    assert.strictEqual(ed._fileTreeCtx.path, 'C:/ws', '目标目录应为工作区根目录');
    assert.strictEqual(ed._fileTreeCtx.isDir, true, '根目录按目录处理');
    assert.strictEqual(ed._fileTreeCtx.blank, true, '应带 blank 标记');
  });
});

test('空白处右键：新建文件/文件夹可用，剪贴板为空时粘贴禁用', async () => {
  await withEditor({ captureInitErr: true }, (w, ed) => {
    const { tree } = makeTreeWithNode(w);
    ed.workspaceFolder = 'C:/ws';
    ed._fileClipboard = null;
    const menu = w.document.getElementById('context-menu-file-tree-blank');
    const isDisabled = (a) => menu.querySelector(`[data-action="${a}"]`).classList.contains('disabled');

    dispatchCtxMenu(w, tree);
    assert.strictEqual(isDisabled('file-new-file'), false, '新建文件应可用');
    assert.strictEqual(isDisabled('file-new-folder'), false, '新建文件夹应可用');
    assert.strictEqual(isDisabled('file-paste'), true, '剪贴板为空时粘贴应禁用');
    assert.strictEqual(isDisabled('file-close-folder'), false, '已打开工作区时关闭文件夹应可用');

    // 剪贴板有内容后粘贴应解禁
    ed._fileClipboard = { op: 'copy', path: 'C:/ws/x.md', isDir: false };
    ed.updateFileTreeBlankMenuState();
    assert.strictEqual(isDisabled('file-paste'), false, '剪贴板非空时粘贴应可用');
  });
});

test('右键节点仍弹出节点菜单（不与空白菜单冲突）', async () => {
  await withEditor({ captureInitErr: true }, (w, ed) => {
    const { node } = makeTreeWithNode(w);
    ed.workspaceFolder = 'C:/ws';
    const menu = w.document.getElementById('context-menu-file-tree-blank');
    const nodeMenu = w.document.getElementById('context-menu-file-tree');

    dispatchCtxMenu(w, node);

    assert.ok(!nodeMenu.classList.contains('hidden'), '右键节点应显示节点菜单');
    assert.ok(menu.classList.contains('hidden'), '不应显示空白菜单');
    assert.strictEqual(ed._fileTreeCtx.path, 'C:/ws/a.md');
    assert.ok(!ed._fileTreeCtx.blank, '节点上下文不应带 blank 标记');
  });
});

test('未打开工作区时右键空白处不弹菜单', async () => {
  await withEditor({ captureInitErr: true }, (w, ed) => {
    const { tree } = makeTreeWithNode(w);
    ed.workspaceFolder = null;
    const menu = w.document.getElementById('context-menu-file-tree-blank');
    dispatchCtxMenu(w, tree);
    assert.ok(menu.classList.contains('hidden'), '无工作区时不应弹出空白菜单');
    assert.strictEqual(ed._fileTreeCtx, null, '无工作区时不应设置上下文');
  });
});

test('blank 上下文下 Delete 不触发删除确认（防止误删整个工作区）', async () => {
  await withEditor({ captureInitErr: true }, async (w, ed) => {
    ed.workspaceFolder = 'C:/ws';
    ed._fileTreeCtx = { path: 'C:/ws', isDir: true, nodeEl: null, blank: true };
    let confirmCalls = 0;
    ed.showConfirmDialog = async () => { confirmCalls++; return false; };

    dispatchKey(w, w.document.body, 'Delete', 'Delete', 46);
    await delay(20);
    assert.strictEqual(confirmCalls, 0, '空白处上下文不应弹删除确认框');

    // 对照：真实文件节点的 Delete 仍然生效
    ed._fileTreeCtx = { path: 'C:/ws/a.md', isDir: false, nodeEl: null };
    dispatchKey(w, w.document.body, 'Delete', 'Delete', 46);
    await delay(20);
    assert.strictEqual(confirmCalls, 1, '文件节点的 Delete 应弹删除确认框');
  });
});

test('blank 上下文下 F2/Ctrl+X/Ctrl+C 被跳过，Ctrl+V 仍可用', async () => {
  await withEditor({ captureInitErr: true }, (w, ed) => {
    ed.workspaceFolder = 'C:/ws';
    ed._fileTreeCtx = { path: 'C:/ws', isDir: true, nodeEl: null, blank: true };
    let renamed = 0, cut = 0, copied = 0, pasted = 0;
    ed.fileTreeRename = async () => { renamed++; };
    ed.fileTreeCut = () => { cut++; };
    ed.fileTreeCopy = () => { copied++; };
    ed.fileTreePaste = async () => { pasted++; };

    dispatchKey(w, w.document.body, 'F2', 'F2', 113);
    dispatchKey(w, w.document.body, 'x', 'KeyX', 88, { ctrl: true });
    dispatchKey(w, w.document.body, 'c', 'KeyC', 67, { ctrl: true });
    dispatchKey(w, w.document.body, 'v', 'KeyV', 86, { ctrl: true });

    assert.strictEqual(renamed, 0, 'blank 不应触发重命名');
    assert.strictEqual(cut, 0, 'blank 不应触发剪切');
    assert.strictEqual(copied, 0, 'blank 不应触发复制');
    assert.strictEqual(pasted, 1, 'blank 下粘贴到根目录应可用');
  });
});

test('file-refresh 重建文件树；file-reveal 调用 reveal_in_file_manager', async () => {
  const calls = [];
  let renders = 0;
  await withEditor({
    captureInitErr: true,
    invokeImpl: async (cmd, args) => { calls.push({ cmd, args }); return undefined; },
  }, async (w, ed) => {
    ed.workspaceFolder = 'C:/ws';
    ed.renderFolderTree = async () => { renders++; };
    ed.setStatus = () => {};

    ed._fileTreeCtx = { path: 'C:/ws', isDir: true, nodeEl: null, blank: true };
    ed.executeMenuAction('file-refresh');
    await delay(10);
    assert.strictEqual(renders, 1, 'file-refresh 应重建文件树');

    ed.executeMenuAction('file-reveal');
    await delay(10);
    const reveal = calls.find(c => c.cmd === 'reveal_in_file_manager');
    assert.ok(reveal, 'file-reveal 应调用 reveal_in_file_manager');
    assert.strictEqual(reveal.args.path, 'C:/ws', 'reveal 目标应为工作区根目录');
  });
});

test('file-close-folder 关闭工作区', async () => {
  await withEditor({ captureInitErr: true }, (w, ed) => {
    ed.workspaceFolder = 'C:/ws';
    ed.renderFolderTree = async () => {};
    ed.updateWorkspaceMenuState = () => {};
    ed.saveSession = () => {};
    ed.executeMenuAction('file-close-folder');
    assert.strictEqual(ed.workspaceFolder, null, '关闭文件夹后工作区应为空');
  });
});
