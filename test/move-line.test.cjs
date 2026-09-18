// 上下移动行/选中段落 回归测试。
// 背景：CM5 核心无 moveLineUp/moveLineDown 命令，_moveLine 自实现交换相邻行块。
// 覆盖：单行上下移、多行选区块移动、文档边界 no-op、光标/选区位置跟随平移、
//       handler 在 editorMap 注册并可经快捷键键名找到。

const test = require('node:test');
const assert = require('node:assert');
const { withEditor } = require('./helpers/app-env.cjs');

test('上移单行：第 2 行内容升到第 1 行，光标跟随上移', async () => withEditor({ captureInitErr: true }, async (w, ed) => {
  ed.cm.setValue('第一行\n第二行\n第三行');
  ed.cm.setCursor({ line: 1, ch: 0 }); // 光标在「第二行」
  ed._moveLine(-1);
  assert.strictEqual(ed.cm.getLine(0), '第二行', '上移后第 1 行应为原第 2 行');
  assert.strictEqual(ed.cm.getLine(1), '第一行', '上移后第 2 行应为原第 1 行');
  assert.strictEqual(ed.cm.getLine(2), '第三行', '未涉及行不变');
  assert.strictEqual(ed.cm.getCursor().line, 0, '光标应跟随上移到第 1 行');
}));

test('下移单行：第 1 行内容降到第 2 行，光标跟随后移', async () => withEditor({ captureInitErr: true }, async (w, ed) => {
  ed.cm.setValue('第一行\n第二行\n第三行');
  ed.cm.setCursor({ line: 0, ch: 0 }); // 光标在「第一行」
  ed._moveLine(1);
  assert.strictEqual(ed.cm.getLine(0), '第二行', '下移后第 1 行应为原第 2 行');
  assert.strictEqual(ed.cm.getLine(1), '第一行', '下移后第 2 行应为原第 1 行');
  assert.strictEqual(ed.cm.getCursor().line, 1, '光标应跟随后移到第 2 行');
}));

test('多行选区上移：整块 2-3 行升到 1-2 行，选区保持块内范围', async () => withEditor({ captureInitErr: true }, async (w, ed) => {
  ed.cm.setValue('A\nB\nC\nD');
  // 选中 B、C 两行（line 1 到 line 2）
  ed.cm.setSelection({ line: 1, ch: 0 }, { line: 2, ch: 1 });
  ed._moveLine(-1);
  assert.strictEqual(ed.cm.getLine(0), 'B', '块上移后第 1 行为 B');
  assert.strictEqual(ed.cm.getLine(1), 'C', '块上移后第 2 行为 C');
  assert.strictEqual(ed.cm.getLine(2), 'A', '原第 1 行 A 落到块尾后');
  assert.strictEqual(ed.cm.getLine(3), 'D', '未涉及行不变');
  // 选区应跟随块整体上移一行（原 1-2 → 现 0-1）
  const sel = ed.cm.listSelections()[0];
  assert.strictEqual(Math.min(sel.anchor.line, sel.head.line), 0, '选区起点跟随上移');
  assert.strictEqual(Math.max(sel.anchor.line, sel.head.line), 1, '选区终点跟随上移');
}));

test('多行选区下移：整块 1-2 行降到 2-3 行', async () => withEditor({ captureInitErr: true }, async (w, ed) => {
  ed.cm.setValue('A\nB\nC\nD');
  ed.cm.setSelection({ line: 0, ch: 0 }, { line: 1, ch: 1 }); // 选中 A、B
  ed._moveLine(1);
  assert.strictEqual(ed.cm.getLine(0), 'C', '原下一行 C 升到块首');
  assert.strictEqual(ed.cm.getLine(1), 'A', '块下移后 A 在第 2 行');
  assert.strictEqual(ed.cm.getLine(2), 'B', '块下移后 B 在第 3 行');
  assert.strictEqual(ed.cm.getLine(3), 'D', '未涉及行不变');
  const sel = ed.cm.listSelections()[0];
  assert.strictEqual(Math.min(sel.anchor.line, sel.head.line), 1, '选区起点跟随后移');
  assert.strictEqual(Math.max(sel.anchor.line, sel.head.line), 2, '选区终点跟随后移');
}));

test('边界 no-op：第一行上移/最后一行下移不改变文档', async () => withEditor({ captureInitErr: true }, async (w, ed) => {
  ed.cm.setValue('A\nB\nC');
  const before = ed.cm.getValue();
  ed.cm.setCursor({ line: 0, ch: 0 });
  ed._moveLine(-1); // 第一行上移 → no-op
  assert.strictEqual(ed.cm.getValue(), before, '第一行上移应 no-op');
  ed.cm.setCursor({ line: 2, ch: 0 });
  ed._moveLine(1); // 最后一行下移 → no-op
  assert.strictEqual(ed.cm.getValue(), before, '最后一行下移应 no-op');
}));

test('默认键位：moveLineUp/Down 出厂即绑定 Alt+ArrowUp/Down 并注册到 extraKeys', async () => withEditor({ captureInitErr: true }, async (w, ed) => {
  // 全新安装（vscode 默认方案，无覆盖表）：出厂即有 Alt+方向键
  assert.strictEqual(ed.shortcuts.moveLineUp.key, 'Alt+ArrowUp', 'moveLineUp 默认键应为 Alt+ArrowUp');
  assert.strictEqual(ed.shortcuts.moveLineDown.key, 'Alt+ArrowDown', 'moveLineDown 默认键应为 Alt+ArrowDown');
  const extraKeys = ed.cm.getOption('extraKeys');
  assert.strictEqual(typeof extraKeys['Alt-Up'], 'function', '默认键 Alt+ArrowUp 应注册到 extraKeys[Alt-Up]');
  assert.strictEqual(typeof extraKeys['Alt-Down'], 'function', '默认键 Alt+ArrowDown 应注册到 extraKeys[Alt-Down]');
  // 触发验证：默认键位直接可用
  ed.cm.setValue('A\nB\nC');
  ed.cm.setCursor({ line: 1, ch: 0 });
  extraKeys['Alt-Up'](ed.cm);
  assert.strictEqual(ed.cm.getLine(0), 'B', '默认 Alt+Up 应能上移当前行');
}));

test('快捷键注册：moveLineUp/Down 在 editorMap 且经 applyShortcuts 绑定到 extraKeys', async () => withEditor({ captureInitErr: true }, async (w, ed) => {
  // 模拟用户为 moveLineUp 分配 Ctrl+J（与现有快捷键不冲突）
  ed.shortcuts.moveLineUp.key = 'Ctrl+J';
  ed.shortcuts.moveLineDown.key = 'Ctrl+K';
  ed.applyShortcuts();
  const extraKeys = ed.cm.getOption('extraKeys');
  assert.strictEqual(typeof extraKeys['Ctrl-J'], 'function', 'moveLineUp 应绑定到 extraKeys[Ctrl-J]');
  assert.strictEqual(typeof extraKeys['Ctrl-K'], 'function', 'moveLineDown 应绑定到 extraKeys[Ctrl-K]');
  // 实际触发 handler：通过键名调用，验证行真的移动
  ed.cm.setValue('A\nB\nC');
  ed.cm.setCursor({ line: 1, ch: 0 });
  extraKeys['Ctrl-J'](ed.cm); // 上移「B」
  assert.strictEqual(ed.cm.getLine(0), 'B', '经快捷键 handler 触发上移应生效');
}));

test('方向键录制：DOM e.key=ArrowUp 须规范化为 CM5 的 Up，否则 extraKeys 键名不匹配', async () => withEditor({ captureInitErr: true }, async (w, ed) => {
  // 复现用户报告：录 Alt+ArrowUp（移动行的直觉键位）后不生效。
  // 根因：handleShortcutRecording 存 DOM e.key='ArrowUp'，但 CM5 keyName() 用
  // keyNames[keyCode] 产出 'Up'。toCmKey 须把 ArrowUp→Up，否则 extraKeys
  // 注册 'Alt-ArrowUp' 而 CM 查找 'Alt-Up'，handler 永不触发。
  ed.shortcuts.moveLineUp.key = 'Alt+ArrowUp';
  ed.shortcuts.moveLineDown.key = 'Alt+ArrowDown';
  ed.applyShortcuts();
  const extraKeys = ed.cm.getOption('extraKeys');
  assert.strictEqual(typeof extraKeys['Alt-Up'], 'function',
    'Alt+ArrowUp 应规范化为 Alt-Up 注册到 extraKeys（而非 Alt-ArrowUp）');
  assert.strictEqual(typeof extraKeys['Alt-Down'], 'function',
    'Alt+ArrowDown 应规范化为 Alt-Down 注册到 extraKeys');
  assert.strictEqual(extraKeys['Alt-ArrowUp'], undefined, '不应残留未规范化的 Alt-ArrowUp 键');
  // 触发验证：经规范化键名调用 handler，行真的上移
  ed.cm.setValue('A\nB\nC');
  ed.cm.setCursor({ line: 1, ch: 0 });
  extraKeys['Alt-Up'](ed.cm);
  assert.strictEqual(ed.cm.getLine(0), 'B', 'Alt+ArrowUp 经规范化后应能触发上移');
}));

test('Ctrl+Shift+方向键录制：修饰键顺序与方向键规范化同时正确', async () => withEditor({ captureInitErr: true }, async (w, ed) => {
  ed.shortcuts.moveLineUp.key = 'Ctrl+Shift+ArrowUp';
  ed.applyShortcuts();
  const extraKeys = ed.cm.getOption('extraKeys');
  // CM5 addModifierNames 顺序：Alt, Ctrl, Cmd, Shift(最后) → 'Shift-Ctrl-Up'
  assert.strictEqual(typeof extraKeys['Shift-Ctrl-Up'], 'function',
    'Ctrl+Shift+ArrowUp 应规范化并按 CM5 修饰键顺序注册为 Shift-Ctrl-Up');
  assert.strictEqual(extraKeys['Shift-Ctrl-ArrowUp'], undefined, '不应残留未规范化键名');
}));
