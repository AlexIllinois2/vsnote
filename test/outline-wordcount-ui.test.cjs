// 大纲面板 UI 与字数统计 UI 测试：updateOutline 渲染/点击跳转/折叠、updateWordCount、
// headingToId / escapeHtml / escapeAttr / escapeMdText
// 使用 withEditor 串行化，避免 node:test 并发子测试互相踩踏共享的 global.window/document。
const test = require('node:test');
const assert = require('node:assert');
const { withEditor } = require('./helpers/app-env.cjs');

const DOC = '# 一级标题\n\n正文\n\n## 二级 A\n\n### 三级 B\n\n## 二级 C\n';

test('outline-ui: 无标题渲染空提示', async () => withEditor({ captureInitErr: true }, async (w, ed) => {
  ed.cm.setValue('只有正文，没有标题');
  ed.setOutlineVisible(true); // 大纲面板默认隐藏，updateOutline 有 !outlineVisible 早退
  const oc = w.document.getElementById('outline-content');
  assert.ok(oc.querySelector('.outline-empty'), '应渲染 outline-empty');
}));

test('outline-ui: 渲染层级结构与 data-line', async () => withEditor({ captureInitErr: true }, async (w, ed) => {
  ed.cm.setValue(DOC);
  ed.setOutlineVisible(true); // 大纲面板默认隐藏，updateOutline 有 !outlineVisible 早退
  const oc = w.document.getElementById('outline-content');
  const items = [...oc.querySelectorAll('.outline-item')];
  assert.strictEqual(items.length, 4, '应渲染 4 个标题项');
  const lines = items.map((i) => i.dataset.line);
  assert.deepStrictEqual(lines, ['0', '4', '6', '8'], 'data-line 应为标题所在行号');
  assert.ok(oc.textContent.includes('一级标题') && oc.textContent.includes('二级 C'));
}));

test('outline-ui: 点击标题项跳转编辑器光标并激活', async () => withEditor({ captureInitErr: true }, async (w, ed) => {
  ed.cm.setValue(DOC);
  ed.setOutlineVisible(true); // 大纲面板默认隐藏，updateOutline 有 !outlineVisible 早退
  const oc = w.document.getElementById('outline-content');
  const target = [...oc.querySelectorAll('.outline-item')].find((i) => i.dataset.line === '6');
  assert.ok(target, '应找到三级 B 项');
  target.dispatchEvent(new w.MouseEvent('click', { bubbles: true }));
  assert.strictEqual(ed.cm.getCursor().line, 6, '光标应跳到标题行');
  assert.ok(target.classList.contains('active'), '被点击项应激活');
  const actives = oc.querySelectorAll('.outline-item.active');
  assert.strictEqual(actives.length, 1, '只应有一个激活项');
}));

test('outline-ui: 点击折叠开关切换子级显隐', async () => withEditor({ captureInitErr: true }, async (w, ed) => {
  ed.cm.setValue(DOC);
  ed.setOutlineVisible(true); // 大纲面板默认隐藏，updateOutline 有 !outlineVisible 早退
  const oc = w.document.getElementById('outline-content');
  const toggle = oc.querySelector('.outline-toggle');
  assert.ok(toggle, '有子级的标题应有折叠开关');
  const wrapper = toggle.closest('.outline-item-wrapper');
  const children = wrapper.querySelector('.outline-children');
  assert.ok(children && !children.classList.contains('collapsed'), '初始应展开');
  toggle.dispatchEvent(new w.MouseEvent('click', { bubbles: true }));
  assert.ok(children.classList.contains('collapsed'), '点击后应折叠');
  assert.strictEqual(toggle.textContent, '▶');
  toggle.dispatchEvent(new w.MouseEvent('click', { bubbles: true }));
  assert.ok(!children.classList.contains('collapsed'), '再点应展开');
  assert.strictEqual(toggle.textContent, '▼');
}));

test('wordcount-ui: updateWordCount 更新状态栏三项计数', async () => withEditor({ captureInitErr: true }, async (w, ed) => {
  ed.settings.language = 'zh';
  ed.cm.setValue('hello world 你好\n第二行 abc');
  ed.updateWordCount();
  assert.ok(/\d+/.test(ed.wordCountEl.textContent), '词数应为数字');
  assert.ok(ed.lineCountEl.textContent.endsWith('2'), '行数应为 2');
  const chars = parseInt(ed.charCountEl.textContent.match(/(\d+)/)[1], 10);
  assert.strictEqual(chars, 'hello world 你好\n第二行 abc'.length, '字符数应为全文长度');

  ed.cm.setValue('');
  ed.updateWordCount();
  assert.ok(ed.wordCountEl.textContent.endsWith('0'), '空文档词数为 0');
}));

test('outline-ui: headingToId 生成规则', async () => withEditor({ captureInitErr: true }, async (w, ed) => {
  assert.strictEqual(ed.headingToId('Hello World'), 'hello-world');
  assert.strictEqual(ed.headingToId('中文 标题'), '中文-标题');
  assert.strictEqual(ed.headingToId('  A -- B__C  '), 'a-b-c');
  assert.strictEqual(ed.headingToId('!!!'), '', '纯符号应为空串');
  assert.strictEqual(ed.headingToId('Ver 2.0'), 'ver-20', '点号被移除');
}));

test('outline-ui: escapeHtml / escapeAttr / escapeMdText 转义', async () => withEditor({ captureInitErr: true }, async (w, ed) => {
  assert.strictEqual(ed.escapeHtml('<b>&"x"'), '&lt;b&gt;&amp;"x"');
  assert.strictEqual(ed.escapeAttr('a"b<c>&d'), 'a&quot;b&lt;c&gt;&amp;d');
  assert.strictEqual(ed.escapeMdText('a]b\\c'), 'a\\]b\\\\c');
}));

test('outline-ui: 纯符号标题点击不抛 SyntaxError', async () => withEditor({ captureInitErr: true }, async (w, ed) => {
  // headingToId('===') 产出空串；点击空 data-id 时 querySelector('#') 曾抛 SyntaxError
  ed.cm.setValue('# ===\n\n## 正常标题');
  ed.setOutlineVisible(true); // 大纲面板默认隐藏，updateOutline 有 !outlineVisible 早退
  const oc = w.document.getElementById('outline-content');
  const symbolItem = [...oc.querySelectorAll('.outline-item')].find((i) => i.dataset.id === '');
  assert.ok(symbolItem, '纯符号标题应渲染为空 data-id');
  assert.doesNotThrow(() => {
    symbolItem.dispatchEvent(new w.MouseEvent('click', { bubbles: true }));
  }, '点击空 id 标题不得抛 SyntaxError');
  assert.strictEqual(ed.cm.getCursor().line, 0, '光标应跳到标题行');
}));
