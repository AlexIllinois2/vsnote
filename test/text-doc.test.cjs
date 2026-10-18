// 纯文本文档判定与渲染单元测试：锁定 src/modules/text-doc.js 的纯函数行为。
// 这一层决定「哪些文件走 Markdown 管线」，也决定纯文本预览的保真度（空行/换行/锚点行号）。
const test = require('node:test');
const assert = require('node:assert');
const TextDoc = require('../src/modules/text-doc.js');

test('textDoc: 扩展名判定', async () => {
  assert.strictEqual(TextDoc.extOf('C:/a/b/Note.MD'), 'md');
  assert.strictEqual(TextDoc.extOf('C:/a/readme.markdown'), 'markdown');
  assert.strictEqual(TextDoc.extOf('C:/a/LICENSE'), '');
  assert.strictEqual(TextDoc.extOf('C:/a/.gitignore'), '', '点开头命名不算扩展名');
  assert.strictEqual(TextDoc.extOf('C:/a/log.txt'), 'txt');
});

test('textDoc: 仅 Markdown 扩展名走 Markdown 管线', async () => {
  assert.strictEqual(TextDoc.isMarkdownPath('C:/docs/a.md'), true);
  assert.strictEqual(TextDoc.isMarkdownPath('C:/docs/a.markdown'), true);
  assert.strictEqual(TextDoc.isMarkdownPath('C:/docs/a.txt'), false);
  assert.strictEqual(TextDoc.isMarkdownPath('C:/docs/a.log'), false);
  assert.strictEqual(TextDoc.isMarkdownPath('C:/docs/LICENSE'), false);
  // 无路径 = 未命名草稿，仍按 Markdown（本应用主用途）
  assert.strictEqual(TextDoc.isMarkdownPath(null), true);
  assert.strictEqual(TextDoc.isMarkdownPath(''), true);
  assert.strictEqual(TextDoc.docKind('C:/a.txt'), 'text');
  assert.strictEqual(TextDoc.docKind('C:/a.md'), 'markdown');
});

test('textDoc: CodeMirror 语法映射', async () => {
  assert.strictEqual(TextDoc.cmModeForPath('C:/a.md'), 'gfm');
  assert.strictEqual(TextDoc.cmModeForPath('C:/a.txt'), null, 'txt 必须是纯文本语法（null），不能套 gfm');
  assert.strictEqual(TextDoc.cmModeForPath('C:/a.log'), null);
  assert.strictEqual(TextDoc.cmModeForPath('C:/LICENSE'), null);
  assert.strictEqual(TextDoc.cmModeForPath('C:/a.js'), 'javascript');
  assert.strictEqual(TextDoc.cmModeForPath('C:/a.py'), 'python');
  assert.strictEqual(TextDoc.cmModeForPath('C:/a.css'), 'css');
  assert.strictEqual(TextDoc.cmModeForPath('C:/a.yaml'), 'yaml');
});

test('textDoc: 换行检测与归一化', async () => {
  assert.strictEqual(TextDoc.detectEol('a\r\nb'), '\r\n');
  assert.strictEqual(TextDoc.detectEol('a\nb'), '\n');
  assert.strictEqual(TextDoc.detectEol('a\rb'), '\n', '单独 CR 不视为 CRLF 文件');
  assert.strictEqual(TextDoc.normalizeNewlines('a\r\nb\rc\n'), 'a\nb\nc\n');
  // 关键性能路径：无 \r 时必须原样返回（同一字符串引用，不产生新副本）
  const lf = 'a\nb\nc';
  assert.strictEqual(TextDoc.normalizeNewlines(lf), lf);
  assert.ok(TextDoc.normalizeNewlines(lf) === lf, 'LF 文本不应被复制');
  assert.strictEqual(TextDoc.normalizeNewlines(null), '');
});

test('textDoc: 写回时还原 CRLF，LF 文件不受影响', async () => {
  assert.strictEqual(TextDoc.applyEol('a\nb\n', '\r\n'), 'a\r\nb\r\n');
  assert.strictEqual(TextDoc.applyEol('a\r\nb\r\n', '\r\n'), 'a\r\nb\r\n', '已 CRLF 时幂等，不得产出 \\r\\r\\n');
  assert.strictEqual(TextDoc.applyEol('a\nb', '\n'), 'a\nb');
  assert.strictEqual(TextDoc.applyEol('single line', '\r\n'), 'single line', '无换行不处理');
  assert.strictEqual(TextDoc.applyEol('', '\r\n'), '');
});

test('textDoc: 纯文本渲染保真（行数、空行、转义）', async () => {
  const html = TextDoc.renderPlainTextHtml('第一行\n\n第三行 <b>&"x"</b>', { startLine: 0 });
  assert.ok(html.startsWith('<pre class="plain-preview">'), '应包在 plain-preview 容器里');
  // 转义：HTML 不会被当成标记渲染
  assert.ok(html.includes('&lt;b&gt;&amp;"x"'), '标签与 & 应被转义（文本节点里的引号无需转义）');
  assert.ok(!html.includes('<b>'), '不应产生真实 <b> 元素');
  // 锚点：1-based 源码行号
  const anchors = [...html.matchAll(/data-source-line="(\d+)"/g)].map((m) => Number(m[1]));
  assert.deepStrictEqual(anchors, [1, 2, 3], '小文档逐行打锚点，行号从 1 开始');
  // 空行必须留得住：三行文本渲染出来仍是 3 行
  assert.strictEqual(html.replace(/^<pre[^>]*>|<\/pre>$/g, '').split('\n').length, 3);
});

test('textDoc: 切片偏移与锚点上限', async () => {
  const html = TextDoc.renderPlainTextHtml('a\nb\nc', { startLine: 10 });
  const anchors = [...html.matchAll(/data-source-line="(\d+)"/g)].map((m) => Number(m[1]));
  assert.deepStrictEqual(anchors, [11, 12, 13], '滑动窗口切片要还原成绝对行号');

  // 大文档：锚点数量受 maxAnchors 约束，但文本行数（换行数）必须与源文本一致
  const lines = [];
  for (let i = 0; i < 100; i++) lines.push('L' + i);
  const big = lines.join('\n');
  const bigHtml = TextDoc.renderPlainTextHtml(big, { startLine: 0, maxAnchors: 10 });
  const bigAnchors = [...bigHtml.matchAll(/data-source-line="(\d+)"/g)].map((m) => Number(m[1]));
  assert.strictEqual(bigAnchors.length, 10, '锚点分组到上限');
  assert.strictEqual(bigAnchors[0], 1);
  const body = bigHtml.replace(/^<pre[^>]*>|<\/pre>$/g, '').replace(/<\/?span[^>]*>/g, '');
  assert.strictEqual(body.split('\n').length, 100, '分组不得吃掉换行');
  assert.strictEqual(body.includes('L99'), true);
});

test('textDoc: escapeHtmlText 与无需转义的快路径', async () => {
  assert.strictEqual(TextDoc.escapeHtmlText('plain text'), 'plain text');
  assert.strictEqual(TextDoc.escapeHtmlText('<a> & "b"'), '&lt;a&gt; &amp; "b"');
});
