/**
 * text-doc.js —— 文档类型判定与纯文本渲染的纯函数
 *
 * 为什么需要它：本应用是 Markdown 编辑器，但工作区/系统关联里大量文件是 .txt、
 * .log 等非 Markdown 文本。早期版本对这些文件同样按 Markdown 处理（gfm 高亮 +
 * remark 解析 + [TOC]/mermaid/KaTeX 后处理），既让日志里的 `#`、`|`、`*` 变成
 * 莫名其妙的语法色，也让打开/编辑大文本时付出整篇 Markdown 解析的代价。
 *
 * 本模块只做「与 DOM / 全局状态无关」的判定与字符串处理，方便单测：
 *  - docKind / isMarkdownPath：是否走 Markdown 管线
 *  - cmModeForPath：CodeMirror 语法（Markdown → gfm，代码 → 对应 mode，其余 → null 纯文本）
 *  - detectEol / normalizeNewlines / applyEol：换行符保真（读入归一为 LF，写回还原原有风格）
 *  - renderPlainTextHtml：把原文按行渲染成可选中的 <pre>（不做任何 Markdown 解释）
 *
 * 双导出：浏览器挂 window.TextDoc；node 走 module.exports（互斥式，N29）。
 */
(function () {
  'use strict';

  // 走 Markdown 管线的扩展名。除此之外的一律按文本处理，未知扩展名不猜语法。
  const MARKDOWN_EXTS = ['md', 'markdown', 'mdown', 'mkd', 'rmd'];

  // 有对应 CodeMirror mode 的常见代码/配置扩展名（mode 文件已在 index.html 引入）。
  // 这些文件同样不属于 Markdown，只是语法着色比纯文本更有用。
  const SYNTAX_MODES = {
    js: 'javascript', mjs: 'javascript', cjs: 'javascript', jsx: 'javascript',
    ts: 'javascript', tsx: 'javascript', json: 'javascript', jsonc: 'javascript',
    py: 'python', rs: 'rust', css: 'css',
    html: 'htmlmixed', htm: 'htmlmixed', xml: 'xml', svg: 'xml',
    yml: 'yaml', yaml: 'yaml', sh: 'shell', bash: 'shell',
  };

  function extOf(path) {
    const name = String(path == null ? '' : path).split(/[/\\]/).pop();
    const dot = name.lastIndexOf('.');
    // 无扩展名（LICENSE）与点开头命名（.gitignore）都不视为 Markdown
    if (dot <= 0) return '';
    return name.slice(dot + 1).toLowerCase();
  }

  // 无 filePath 的新建草稿仍按 Markdown（本应用的主用途）
  function isMarkdownPath(path) {
    if (path == null || path === '') return true;
    return MARKDOWN_EXTS.indexOf(extOf(path)) !== -1;
  }

  function docKind(path) {
    return isMarkdownPath(path) ? 'markdown' : 'text';
  }

  // CodeMirror mode：Markdown 用 gfm，已知代码扩展名用对应 mode，其余 null（纯文本）。
  // null 而非 'gfm' 是关键性能点：gfm/markdown 会对整篇做语法着色扫描，
  // 大文本每行都白付一次 tokenizer 开销。
  function cmModeForPath(path) {
    if (isMarkdownPath(path)) return 'gfm';
    const mode = SYNTAX_MODES[extOf(path)];
    return mode || null;
  }

  // 读入时的换行风格（必须在归一化之前对原始文本判定）：
  // 只要出现 CRLF 就认为该文件是 CRLF，写回时还原，避免编辑一次就把 Windows
  // 记事本/日志文件悄悄改成 LF（diff 会显示整篇改动）。
  function detectEol(raw) {
    const s = String(raw == null ? '' : raw);
    const crlf = s.indexOf('\r\n');
    if (crlf !== -1) return '\r\n';
    return '\n';
  }

  // 归一化为 LF：无 \r 时直接返回原串，省掉对整篇文本的两次正则替换
  // （几 MB 文本上这两次 replace 是打开耗时里可见的一段）。
  function normalizeNewlines(raw) {
    const s = String(raw == null ? '' : raw);
    if (s.indexOf('\r') === -1) return s;
    return s.replace(/\r\n/g, '\n').replace(/\r/g, '\n');
  }

  // 写回时按原风格还原换行；LF 文件与无换行文本直接原样返回。
  // 匹配 \r?\n 而非 \n：即使文本里已存在 CRLF 也只补齐一次，幂等且不会产出 \r\r\n。
  function applyEol(text, eol) {
    const s = String(text == null ? '' : text);
    if (eol !== '\r\n' || s.indexOf('\n') === -1) return s;
    return s.replace(/\r?\n/g, '\r\n');
  }

  function escapeHtmlText(s) {
    // 绝大多数文本行不含 & < >，先探测再决定是否付出正则替换的代价
    if (!/[&<>]/.test(s)) return s;
    return s.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;');
  }

  // 纯文本预览：整篇包在一个 <pre class="plain-preview"> 里，按行分组打
  // data-source-line（1-based，与 Markdown 渲染的锚点约定一致），供滚动同步 /
  // 预览内查找定位使用。
  //
  // 为什么不是一行一个元素：5000 行文本产生 5000 个 DOM 节点，innerHTML 解析与
  // 布局成本明显；也不是一块一个 <pre>：分块会在块边界吃掉换行，空行数量对不齐。
  // 分组跨度由 maxAnchors 自适应（小文档逐行、大文档按组），文本本身始终连续，
  // 因此既保真又可控。
  function renderPlainTextHtml(text, opts) {
    opts = opts || {};
    const startLine = Number.isFinite(opts.startLine) ? opts.startLine : 0; // 0-based 切片起始行
    const maxAnchors = Number.isFinite(opts.maxAnchors) && opts.maxAnchors > 0 ? opts.maxAnchors : 3000;
    const src = String(text == null ? '' : text);
    const lines = src.split('\n');
    const total = lines.length;
    const group = Math.max(1, Math.ceil(total / maxAnchors));
    const out = [];
    for (let i = 0; i < total; i += group) {
      const chunk = lines.slice(i, i + group);
      out.push('<span data-source-line="' + (startLine + i + 1) + '">' +
               escapeHtmlText(chunk.join('\n')) + '</span>');
    }
    // 组之间补回被 slice 丢掉的换行，保证渲染出来的行数与源文本严格一致
    return '<pre class="plain-preview">' + out.join('\n') + '</pre>';
  }

  const api = {
    MARKDOWN_EXTS: MARKDOWN_EXTS,
    SYNTAX_MODES: SYNTAX_MODES,
    extOf: extOf,
    isMarkdownPath: isMarkdownPath,
    docKind: docKind,
    cmModeForPath: cmModeForPath,
    detectEol: detectEol,
    normalizeNewlines: normalizeNewlines,
    applyEol: applyEol,
    escapeHtmlText: escapeHtmlText,
    renderPlainTextHtml: renderPlainTextHtml,
  };

  if (typeof window !== 'undefined' && typeof module === 'undefined') {
    window.TextDoc = api;
  } else if (typeof module !== 'undefined' && module.exports) {
    module.exports = api;
  }
})();
