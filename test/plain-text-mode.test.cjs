// 纯文本（txt / log 等非 Markdown）文档模式行为测试：
//   ① 系统关联入口（file-open 事件 / 拖放）打开 .txt 走纯文本姿态
//   ② CodeMirror 语法、Enter/Tab 的 Markdown 专属行为、格式工具栏、状态栏标记
//   ③ 预览不再套 Markdown 管线
//   ④ 换行符保真（CRLF 文件保存回 CRLF，Markdown 保持旧的统一 LF 行为）
//   ⑤ 大文档惰性回写与折叠预览跳过的正确性
// 依赖共享 harness（jsdom + 真实 index.html + 真实 CodeMirror）。
const test = require('node:test');
const assert = require('node:assert');
const { buildEnv, cleanup, delay, waitForEditor } = require('./helpers/app-env.cjs');
const { loadUnifiedRenderer } = require('./helpers/load-bundle.cjs');

const TXT_LINES = '第一行日志\n第二行 | 带竖线\n  - 像列表的行\n# 像标题的行\n\n最后一行';

async function makeEnv(invokeImpl) {
  const { w, tauriListeners } = await buildEnv({ captureInitErr: true, invokeImpl });
  const ed = await waitForEditor(w);
  // Markdown 分支要真实渲染，需要 esbuild 产物（缺失时 helper 给出可操作指引）
  loadUnifiedRenderer(w);
  return { w, ed, listeners: tauriListeners };
}

function fire(listeners, name, payload) {
  const cbs = listeners[name] || [];
  return Promise.all(cbs.map((cb) => cb({ payload })));
}

// 静默掉与断言无关的副作用（自动保存 / 外部变更探测 / 会话落盘）
function isolate(ed) {
  ed.refreshFileMeta = async () => {};
  ed.saveSession = () => {};
  ed.setStatus = () => {};
  ed._scheduleAutoSave = () => {};
  ed.updateExternalChangeBanner = () => {};
  ed.highlightTreeActiveFile = () => {};
  ed.renderFolderTree = () => {};
}

test('plainText: file-open（系统关联二次实例）打开 txt 走纯文本模式', async () => {
  const { w, ed, listeners } = await makeEnv(async (cmd, args) => {
    if (cmd === 'read_file') return TXT_LINES;
    if (cmd === 'get_cli_args') return [];
    return undefined;
  });
  try {
    isolate(ed);
    await fire(listeners, 'file-open', ['C:/logs/app.txt']);
    await delay(120);
    const tab = ed.tabs.find((t) => t.filePath === 'C:/logs/app.txt');
    assert.ok(tab, '应打开 txt 标签');
    assert.strictEqual(ed._plainTextActive, true, '应判定为纯文本');
    assert.strictEqual(ed.cm.getOption('mode'), null, '语法必须是纯文本（null），不能是 gfm');
    assert.ok(!ed.cm.getOption('autoCloseBrackets'), '纯文本不应自动补括号');
    assert.ok(!ed.cm.getOption('matchBrackets'), '纯文本不应做括号配对高亮');
    const toolbar = w.document.getElementById('format-toolbar');
    assert.ok(toolbar.classList.contains('text-mode'), 'Markdown 格式工具栏应收起');
    const kind = w.document.getElementById('doc-kind');
    assert.strictEqual(kind.textContent, '纯文本', '状态栏应标注文档类型');
    assert.ok(!kind.classList.contains('hidden'), '状态栏标记应可见');
    assert.ok(w.document.querySelector('.editor-container').classList.contains('preview-collapsed'),
      '纯文本默认折叠预览区（预览与编辑内容一致，不必占半屏）');
    // 预览里原文照排，不做 Markdown 解释
    assert.ok(ed.preview.querySelector('pre.plain-preview'), '应渲染为纯文本容器');
    assert.ok(!ed.preview.querySelector('h1'), '# 开头的行不应被渲染成标题');
    assert.ok(ed.preview.textContent.includes('像标题的行'));
  } finally { cleanup(w); }
});

test('plainText: md 文件仍是 Markdown 姿态', async () => {
  const { w, ed } = await makeEnv(async (cmd) => {
    if (cmd === 'read_file') return '# 标题\n\n正文';
    if (cmd === 'get_cli_args') return [];
    return undefined;
  });
  try {
    isolate(ed);
    await ed.openFilePath('C:/docs/note.md');
    await delay(80);
    assert.strictEqual(ed._plainTextActive, false);
    assert.strictEqual(ed.cm.getOption('mode'), 'gfm');
    assert.ok(ed.cm.getOption('autoCloseBrackets'), 'Markdown 保留括号自动配对');
    const toolbar = w.document.getElementById('format-toolbar');
    assert.ok(!toolbar.classList.contains('text-mode'), 'Markdown 应显示格式工具栏');
    assert.ok(w.document.getElementById('doc-kind').classList.contains('hidden'),
      'Markdown 不显示类型标记');
    assert.ok(ed.preview.querySelector('h1'), 'Markdown 应走预览排版');

    // 正对照：Markdown 里表格行按 Enter 仍自动补齐结构 —— 证明纯文本的差异是刻意生效，
    // 不是把编辑行为整体做坏（这条走 app.js 自己的逻辑，不依赖 addon）
    ed.cm.setValue('| a | b |');
    ed.cm.setCursor({ line: 0, ch: 9 });
    ed._handleTableEnter(ed.cm);
    assert.strictEqual(ed.cm.getLine(1), '|   |   |', 'Markdown 应自动补齐等列表格行');
  } finally { cleanup(w); }
});

test('plainText: 设置关闭后 txt 仍按 Markdown 处理', async () => {
  const { w, ed } = await makeEnv(async (cmd) => {
    if (cmd === 'read_file') return '# 伪标题';
    if (cmd === 'get_cli_args') return [];
    return undefined;
  });
  try {
    isolate(ed);
    ed.settings.plainTextMode = false;
    await ed.openFilePath('C:/docs/plain.txt');
    await delay(80);
    assert.strictEqual(ed._plainTextActive, false, '关闭纯文本模式后 txt 按 Markdown');
    assert.strictEqual(ed.cm.getOption('mode'), 'gfm');
  } finally { cleanup(w); }
});

test('plainText: Enter 不续写列表 / 表格，Tab 不整行缩进', async () => {
  const { w, ed } = await makeEnv(async (cmd) => {
    if (cmd === 'read_file') return '- 甲\n\n| a | b |\n|---|---|';
    if (cmd === 'get_cli_args') return [];
    return undefined;
  });
  try {
    isolate(ed);
    await ed.openFilePath('C:/notes/list.txt');
    await delay(80);
    const lineIdx = (text) => ed.cm.getValue().split('\n').indexOf(text);

    const li = lineIdx('- 甲');
    ed.cm.setCursor({ line: li, ch: 3 });
    ed._handleTableEnter(ed.cm);
    assert.strictEqual(ed.cm.getLine(li + 1), '', '纯文本 Enter 只换行，不应补出 "- " 列表前缀');

    const row = lineIdx('| a | b |');
    ed.cm.setCursor({ line: row, ch: '| a | b |'.length });
    ed._handleTableEnter(ed.cm);
    assert.strictEqual(ed.cm.getLine(row + 1), '', '纯文本 Enter 不应自动补表格行');

    // Tab：纯文本里即使行首是 "- " 也只是插入缩进空格，不整行下压一级
    const before = ed.cm.getLine(li);
    ed.cm.setCursor({ line: li, ch: 0 });
    const extraKeys = ed.cm.getOption('extraKeys');
    extraKeys.Tab(ed.cm);
    assert.strictEqual(ed.cm.getLine(li), '    ' + before, 'Tab 应插入空格而非整行列表缩进');
  } finally { cleanup(w); }
});

test('plainText: CRLF 文件保存回 CRLF，Markdown 仍统一 LF', async () => {
  const writes = [];
  const { w, ed } = await makeEnv(async (cmd, args) => {
    if (cmd === 'read_file') return '行一\r\n行二\r\n';
    if (cmd === 'write_file') { writes.push(args); return undefined; }
    if (cmd === 'get_cli_args') return [];
    return undefined;
  });
  try {
    isolate(ed);
    await ed.openFilePath('C:/notes/win.txt');
    await delay(80);
    const txtTab = ed.activeTab;
    assert.strictEqual(txtTab.eol, '\r\n', '应记住原文件换行风格');
    assert.ok(txtTab.content.indexOf('\r') === -1, '编辑器内统一 LF');

    ed.cm.replaceRange('!', { line: 0, ch: 2 });
    await ed.saveFile();
    const txtWrite = writes[writes.length - 1];
    assert.strictEqual(txtWrite.path, 'C:/notes/win.txt');
    assert.strictEqual(txtWrite.content, '行一!\r\n行二\r\n', '保存应还原 CRLF');

    // Markdown：旧行为不变（编辑态 LF，写盘 LF）
    await ed.openFilePath('C:/notes/win.md');
    await delay(80);
    ed.cm.replaceRange('?', { line: 0, ch: 2 });
    await ed.saveFile();
    const mdWrite = writes[writes.length - 1];
    assert.strictEqual(mdWrite.content, '行一?\n行二\n', 'Markdown 保存保持 LF');
  } finally { cleanup(w); }
});

test('plainText: 大文档惰性回写，保存写入的是最新内容', async () => {
  const writes = [];
  const big = 'A'.repeat(60) + '\n'.repeat(1);
  const lines = [];
  for (let i = 0; i < 5000; i++) lines.push(big + i);
  const content = lines.join('\n');   // 约 350KB > LAZY_CONTENT_CHARS
  const { w, ed } = await makeEnv(async (cmd, args) => {
    if (cmd === 'read_file') return content;
    if (cmd === 'write_file') { writes.push(args.content); return undefined; }
    if (cmd === 'get_cli_args') return [];
    return undefined;
  });
  try {
    isolate(ed);
    await ed.openFilePath('C:/big/huge.txt');
    await delay(80);
    const tab = ed.activeTab;
    assert.ok(content.length > 256 * 1024, '用例文本需超过惰性阈值');
    assert.ok(ed._isBigDoc(tab), '应识别为大文档');

    // 连续编辑：不应把整篇文本拷回 tab.content（那是每次按键的 O(n) 拷贝），只标脏
    for (let i = 0; i < 5; i++) ed.cm.replaceRange('x', { line: 0, ch: i });
    assert.strictEqual(tab._contentDirty, true, '大文档编辑应只打脏标记');
    assert.strictEqual(tab.content, content, 'content 允许滞后于编辑器缓冲区');
    assert.ok(tab.isModified, '脏标记即视为已修改');

    await ed.saveFile();
    assert.strictEqual(writes[writes.length - 1].slice(0, 5), 'xxxxx', '保存必须先兑现最新内容');
    assert.strictEqual(tab._contentDirty, false, '保存后脏标记清除');
    assert.ok(!tab.isModified, '保存后不再显示已修改');
  } finally { cleanup(w); }
});

test('plainText: 含超长单行的文本禁用自动换行并说明原因', async () => {
  const longLine = 'x'.repeat(4000);
  const { w, ed } = await makeEnv(async (cmd, args) => {
    if (cmd === 'read_file') return String(args.path).indexOf('dump') !== -1 ? longLine + '\n普通短行' : '普通短行\n另一行';
    if (cmd === 'get_cli_args') return [];
    return undefined;
  });
  try {
    isolate(ed);
    assert.strictEqual(ed.settings.lineWrap, true, '用例前提是用户开着自动换行');
    await ed.openFilePath('C:/t/dump.txt');
    await delay(80);
    assert.strictEqual(ed.cm.getOption('lineWrapping'), false,
      '超长单行在自动换行下要拆成几千个可视行，应改为横向滚动');
    assert.ok(w.document.getElementById('doc-kind').textContent.includes('超长行'),
      '状态栏要解释为什么换行没生效，否则像设置坏了');

    // 正常文本仍按设置自动换行
    await ed.openFilePath('C:/t/normal.txt');
    await delay(80);
    assert.strictEqual(ed.cm.getOption('lineWrapping'), true);
  } finally { cleanup(w); }
});

test('plainText: 打开 txt 后状态栏计数与内容一致（含 CRLF 原文件）', async () => {
  const rows = [];
  for (let i = 0; i < 400; i++) rows.push('2026-10-18 09:0' + (i % 10) + ':00 [INFO ] worker-' + (i % 8) + ' - 处理请求 id=' + i);
  const rawCrlf = rows.join('\r\n') + '\r\n';
  const { w, ed } = await makeEnv(async (cmd) => {
    if (cmd === 'read_file') return rawCrlf;
    if (cmd === 'get_cli_args') return [];
    return undefined;
  });
  try {
    isolate(ed);
    await ed.openFilePath('C:/t/status.txt');
    await delay(120);
    const text = ed.cm.getValue();
    const expected = require('../src/modules/word-count.js').countStats(text);
    assert.strictEqual(text.indexOf('\r'), -1, '编辑器内统一 LF');
    assert.strictEqual(w.document.getElementById('line-count').textContent, '行数: ' + expected.lines);
    assert.strictEqual(w.document.getElementById('char-count').textContent, '字符: ' + expected.chars);
    assert.strictEqual(w.document.getElementById('word-count').textContent, '字数: ' + expected.words);
    assert.strictEqual(expected.lines, 401, 'CRLF 归一化后行数应与源一致');
  } finally { cleanup(w); }
});

test('plainText: 预览折叠时跳过渲染，展开时补渲染', async () => {
  const { w, ed } = await makeEnv(async (cmd) => {
    if (cmd === 'read_file') return '行 A\n行 B';
    if (cmd === 'get_cli_args') return [];
    return undefined;
  });
  try {
    isolate(ed);
    await ed.openFilePath('C:/t/a.txt');
    await delay(80);
    let renders = 0;
    const realRender = ed.updatePreview.bind(ed);
    ed.updatePreview = async (...args) => { renders++; return realRender(...args); };
    assert.ok(w.document.querySelector('.editor-container').classList.contains('preview-collapsed'));

    ed.cm.replaceRange('Z', { line: 0, ch: 0 });
    ed.debounceUpdatePreview();
    await delay(400);
    assert.strictEqual(renders, 0, '折叠预览时打字不应重渲染');
    assert.strictEqual(ed._previewRenderSkipped, true, '应记录有渲染被跳过');
    assert.strictEqual(ed.activeTab.content, ed.cm.getValue(), '跳过的同时正文仍要回写');

    ed.togglePreviewCollapse();
    await delay(40);
    assert.ok(renders >= 1, '展开预览后补渲染一次');
    assert.strictEqual(ed._previewRenderSkipped, false);
    assert.ok(ed.preview.textContent.includes('Z行 A'), '补渲染的是当前内容');
  } finally { cleanup(w); }
});

test('plainText: 大文档窗口计算走 preview-window 模块的 O(n) 实现', async () => {
  const lines = [];
  for (let i = 0; i < 6000; i++) lines.push(`段落行 ${i} 内容`);
  const { w, ed } = await makeEnv(async (cmd) => {
    if (cmd === 'read_file') return lines.join('\n');
    if (cmd === 'get_cli_args') return [];
    return undefined;
  });
  try {
    isolate(ed);
    const calls = [];
    const real = w.PreviewWindow.computePreviewWindow;
    w.PreviewWindow.computePreviewWindow = (content, focus, opts) => {
      calls.push({ focus, opts });
      return real(content, focus, opts);
    };
    assert.strictEqual(ed._computePreviewWindow, undefined, 'app.js 内的 O(n²) 副本应已删除');

    await ed.openFilePath('C:/t/large.md');
    await delay(200);
    assert.ok(calls.length >= 1, '超过阈值时应交给模块计算窗口');
    assert.strictEqual(calls[0].opts.windowLines, 1200);
    assert.ok(ed.previewWindow && ed.previewWindow.end - ed.previewWindow.start <= 1700,
      '渲染窗口应被限制在千行级，而不是整篇');
  } finally { cleanup(w); }
});
