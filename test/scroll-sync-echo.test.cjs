// 滚动同步「回声」回归测试：编辑模式滚动时，编辑区不得被预览拽着上下跑
//
// 背景（用户可感症状）：
//   分栏编辑模式下滚动编辑区，编辑区自己会被上下拽动，影响操作。
//   根因是「预览 → 编辑器」反向同步被两类非用户滚动触发：
//     1) 回声 —— 代码写入 preview.scrollTop 后浏览器必然补发一个 scroll 事件，
//        事件层面与用户真实滚动预览无法区分；
//     2) 预览自己动 —— 图片/公式/mermaid 异步加载后预览变高，浏览器自动调整
//        preview.scrollTop，同样派发 scroll 事件。
//   原先只靠 _canScroll 双标志 + 100ms 解锁定时器挡回声，但长文档上
//   _computedPosition() 一次可达上百毫秒，定时器会在回声到达之前先解锁，
//   回声随即越权驱动编辑区。
//
// 修复：_setPreviewScrollTop/_setEditorScrollTop 写入时留下「值标记」，
// 收到 scroll 事件时按值比对消费（_consumeScrollEcho）；再加「滚动方向所有权」
// （_claimScrollOwner）挡住浏览器自己发起的预览滚动。本测试锁定这两条契约。
//
// 注：jsdom 无真实布局，预览的 scrollTop/scrollHeight/clientHeight 用 defineProperty
// 造出来，编辑器用 stub cm —— 足以验证「谁有资格驱动对方」这一层决策。
const fs = require('fs');
const path = require('path');
const { test } = require('node:test');
const assert = require('node:assert');
const { JSDOM } = require('jsdom');

const appjs = fs.readFileSync(path.join(__dirname, '..', 'src', 'app.js'), 'utf8');
const tauriApiSrc = fs.readFileSync(path.join(__dirname, '..', 'src', 'modules', 'tauri-api.js'), 'utf8');
const previewControllerSrc = fs.readFileSync(path.join(__dirname, '..', 'src', 'controllers', 'preview-controller.js'), 'utf8');
// app.js 的文档类型判定与换行归一化依赖 window.TextDoc（同样按 index.html 顺序先注入）
const textDocSrc = fs.readFileSync(path.join(__dirname, '..', 'src', 'modules', 'text-doc.js'), 'utf8');

const HTML = `<!DOCTYPE html><html><body>
  <div class="editor-container">
    <div class="editor-pane"></div>
    <div class="preview-pane"><div class="preview-content"></div></div>
  </div>
  <div id="editor-pane"></div>
  <div id="preview-pane"></div>
  <button id="btn-view-preview"></button>
  <button id="btn-view-edit"></button>
  <button id="btn-side-right"></button>
</body></html>`;

const dom = new JSDOM(HTML, { runScripts: 'dangerously', pretendToBeVisual: true, url: 'http://localhost/' });
dom.window.__TAURI__ = { core: { invoke: () => Promise.resolve(null) }, path: {}, app: {}, event: {}, shell: {} };

const NAMES = [
  'A1: 编辑器滚动 → 预览同步到映射位置 (150 → 300)',
  'A2: 预览回声不驱动编辑区（cm.scrollTo 未被调用）',
  'A3: 回声标记被消费后清空',
  'A4: 用户真实滚动预览仍会驱动编辑区',
  'A5: 编辑器回声不回写预览',
  'A6: 编辑区刚滚过时，预览的被动滚动不驱动编辑区',
  'A7: 程序化写入预览滚动会收敛到最大可滚范围',
  'A8: 程序化写入预览滚动留下回声标记',
  'A9: 所有权过期后另一侧可接管',
];

const harnessFn = function () {
  const results = [];
  const R = (name, ok) => results.push([name, !!ok]);
  if (typeof MarkdownEditor !== 'function') { results.push(['加载', false]); return results; }
  const now = () => (typeof performance !== 'undefined' && performance.now) ? performance.now() : Date.now();

  const ed = Object.create(MarkdownEditor.prototype);

  // ---- 预览（jsdom 无布局：手工造 scrollTop / scrollHeight / clientHeight）----
  const preview = document.createElement('div');
  preview.id = 'preview';
  let pvTop = 0;
  Object.defineProperty(preview, 'scrollTop', { get: () => pvTop, set: (v) => { pvTop = v; }, configurable: true });
  Object.defineProperty(preview, 'scrollHeight', { get: () => 1000, configurable: true });
  Object.defineProperty(preview, 'clientHeight', { get: () => 400, configurable: true });
  document.body.appendChild(preview);
  ed.preview = preview;

  // 双栏（编辑模式）容器：无 preview-mode / *-collapsed 类
  const container = document.createElement('div');
  container.className = 'editor-container';
  document.body.appendChild(container);

  // ---- 编辑器 stub ----
  const cmScroll = { top: 0, left: 0, clientHeight: 500, height: 5000 };
  const scrollToCalls = [];
  ed.cm = {
    getScrollInfo: () => ({ ...cmScroll }),
    scrollTo: (left, top) => {
      scrollToCalls.push({ left, top });
      if (typeof top === 'number') cmScroll.top = top;
      if (typeof left === 'number') cmScroll.left = left;
    },
  };

  // activeTab 是原型 getter（tabs[activeTabIndex]），用真实数组喂它
  const fakeTab = { scrollPos: { top: 0, left: 0 }, previewScrollTop: 0 };
  ed.tabs = [fakeTab];
  ed.activeTabIndex = 0;
  ed.settings = { scrollSync: true };
  ed._canScroll = { editor: true, preview: true };
  ed._scrollThrottleTimer = null;
  ed._scrollThrottlePending = null;
  ed._scrollDebounceTimer = null;
  ed._previewEchoTop = null;
  ed._editorEchoTop = null;
  ed._scrollOwner = null;
  ed._scrollOwnerUntil = 0;
  ed._previewVirtual = false;
  ed.previewWindow = null;

  // 位置映射：编辑 4 行 ↔ 预览 4 段（编辑 100px/行，预览 200px/段）
  ed._editorElementList = [0, 100, 200, 300];
  ed._previewElementList = [0, 200, 400, 600];
  ed._computedPosition = () => {}; // 保持上面的映射，不被真实测量覆盖

  // 节流/防抖改为同步执行 —— 刻意模拟最坏时序：
  // 「同步还没结束，100ms 解锁定时器就已经把 _canScroll 恢复成 true」。
  ed._throttleScroll = (fn) => fn();
  ed._debounceScroll = (fn) => fn();

  // ---- A1：编辑器滚动 → 预览同步 ----
  cmScroll.top = 150;
  ed._onEditorScroll();
  R('A1: 编辑器滚动 → 预览同步到映射位置 (150 → 300)', pvTop === 300);

  // ---- A2/A3：浏览器为上面那次写入补发 scroll 事件（回声）----
  // 刻意清掉所有权：模拟「同步耗时 > 200ms，所有权也过期」的最坏时序。
  // 此时只剩回声值标记一道防线，若它被摘掉，A2 必须失败。
  const beforeEcho = scrollToCalls.length;
  ed._scrollOwner = null;
  ed._scrollOwnerUntil = 0;
  ed._onPreviewScroll();
  R('A2: 预览回声不驱动编辑区（cm.scrollTo 未被调用）', scrollToCalls.length === beforeEcho);
  R('A3: 回声标记被消费后清空', ed._previewEchoTop === null);

  // ---- A4：用户真实滚动预览（无标记）仍应驱动编辑区 ----
  pvTop = 500;
  ed._scrollOwner = null; // 所有权已过期
  ed._onPreviewScroll();
  R('A4: 用户真实滚动预览仍会驱动编辑区', scrollToCalls.length === beforeEcho + 1 && cmScroll.top === 250);

  // ---- A5：上一步 cm.scrollTo 的回声不得回写预览 ----
  const pvBefore = pvTop;
  ed._onEditorScroll();
  R('A5: 编辑器回声不回写预览', pvTop === pvBefore);

  // ---- A6：编辑区刚滚过 → 预览被异步内容顶动，不得反向驱动 ----
  ed._scrollOwner = 'editor';
  ed._scrollOwnerUntil = now() + 200;
  const n6 = scrollToCalls.length;
  pvTop = 520; // 无回声标记的预览滚动（如图片加载后浏览器自动调整）
  ed._onPreviewScroll();
  R('A6: 编辑区刚滚过时，预览的被动滚动不驱动编辑区', scrollToCalls.length === n6);

  // ---- A7/A8：程序化写入的收敛与标记 ----
  ed._setPreviewScrollTop(9999);
  R('A7: 程序化写入预览滚动会收敛到最大可滚范围', pvTop === 600);
  R('A8: 程序化写入预览滚动留下回声标记', ed._previewEchoTop === 600);

  // ---- A9：所有权过期后可被另一侧接管 ----
  ed._scrollOwnerUntil = 0;
  R('A9: 所有权过期后另一侧可接管', ed._claimScrollOwner('preview') === true);

  return results;
};

const combined = tauriApiSrc + '\n;\n' + textDocSrc + '\n;\n' + previewControllerSrc + '\n;\n' + appjs +
  '\n;window.__harnessPromise = (' + harnessFn.toString() + ')();';
const s = dom.window.document.createElement('script');
s.textContent = combined;
dom.window.document.body.appendChild(s);

for (const name of NAMES) {
  test(name, async () => {
    const results = await dom.window.__harnessPromise;
    if (results.length === 1 && results[0][0] === '加载') {
      assert.fail('MarkdownEditor 未加载，无法测试');
    }
    const item = results.find(r => r[0] === name);
    assert.ok(item, '结果缺失: ' + name);
    assert.ok(item[1] === true, name);
  });
}
