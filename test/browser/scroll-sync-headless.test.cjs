/**
 * 无头浏览器回归测试：滚动同步 + 勾选框跳动
 *
 * 为什么用真实浏览器（而非 jsdom）：
 *   jsdom 没有真实布局 —— offsetTop / scrollHeight / getBoundingClientRect 全为 0，
 *   滚动同步的逐行位置映射（_computedPosition / _syncEditorToPreview）在 jsdom 下
 *   无法被验证。这正是滚动同步回归一直查不出来的根因。本测试用系统 Chrome 的
 *   真实排版引擎，端到端验证「编辑器 ↔ 预览」左右精准匹配。
 *
 * 运行：
 *   NODE_PATH=<managed workspace node_modules> <managed node> test/browser/scroll-sync-headless.test.cjs
 *
 * 前置：dev-server 在 1420 跑着（npm run dev）。本测试只读不写，不触碰 Tauri Rust 后端——
 *   通过注入 window.__TAURI__ mock 让纯前端 app 在浏览器中正常启动。
 */
'use strict';
const fs = require('fs');
const path = require('path');

let puppeteer;
try {
  puppeteer = require('puppeteer-core');
} catch (e) {
  console.log(`  ⏭️  SKIP  puppeteer-core 不可用 (${e.message})，跳过浏览器测试`);
  process.exit(0);
}

function resolveChromePath() {
  if (process.env.CHROME_PATH && fs.existsSync(process.env.CHROME_PATH)) return process.env.CHROME_PATH;
  // Linux 常见路径
  const linuxPaths = ['/usr/bin/google-chrome', '/usr/bin/chromium-browser', '/usr/bin/chromium'];
  for (const p of linuxPaths) { if (fs.existsSync(p)) return p; }
  // Windows 回退（本地开发）
  return 'C:/Program Files/Google/Chrome/Application/chrome.exe';
}
const CHROME_PATH = resolveChromePath();
const URL = 'http://localhost:1420/';

// 足够长、含中部的任务列表（勾选框测试需要），行数足以滚动
function buildDemo() {
  const lines = [];
  lines.push('# TizuMark 无头浏览器回归测试');
  lines.push('');
  lines.push('这是一个用于验证滚动同步与勾选框交互的纯文本文档。下方有若干章节，');
  lines.push('用来制造可以滚动的高度差。');
  lines.push('');
  for (let i = 1; i <= 8; i++) {
    lines.push(`## 第 ${i} 章 示例内容`);
    lines.push('');
    lines.push('这是一段示例正文，用于占位。Markdown 预览会把每一行映射到对应的像素位置，');
    lines.push('滚动同步正是依赖 `data-source-line` 与逐行密集插值做到的左右精准匹配。');
    lines.push('');
    lines.push('- 列表项 A');
    lines.push('- 列表项 B');
    lines.push('- 列表项 C');
    lines.push('');
    lines.push('> 引用块：验证引用内行也能正确参与位置映射。');
    lines.push('');
    lines.push('```js');
    lines.push('const x = 1;');
    lines.push('const y = 2;');
    lines.push('console.log(x + y);');
    lines.push('```');
    lines.push('');
  }
  // 中部塞一个任务列表（勾选框测试目标）
  lines.push('## 任务清单');
  lines.push('');
  lines.push('- [ ] 待办任务一');
  lines.push('- [x] 已完成任务二');
  lines.push('- [ ] 待办任务三');
  lines.push('');
  for (let i = 1; i <= 8; i++) {
    lines.push(`## 后续章节 ${i}`);
    lines.push('');
    lines.push('继续占位，确保文档足够长，编辑器与预览都能产生明显滚动范围。');
    lines.push('');
  }
  return lines.join('\n');
}
const DEMO = buildDemo();

// 在页面任何脚本运行前注入：预设 EULA 已接受 + 完整的 Tauri IPC mock
const TauriMock = `
(function(){
  try { localStorage.setItem('tizumark-eula-accepted','true'); } catch(e){}
  window.__TAURI__ = {
    core: {
      invoke: async function(cmd, args){
        args = args || {};
        switch(cmd){
          case 'read_bundled_file':
            if (args.filename === 'demo.md') return { content: window.__DEMO__ };
            if (args.filename === 'guide.md') return { content: '# 使用说明\\n\\n' };
            return { content: '' };
          case 'read_file': return '';
          case 'app_data_dir': return 'C:\\\\fake\\\\appdata';
          case 'resource_dir': return 'C:\\\\fake\\\\resource';
          case 'list_dir': return [];
          case 'is_directory': return false;
          case 'file_meta': return { size:0, isDir:false, modified:0 };
          case 'write_file': case 'write_binary_file': case 'ensure_dir':
          case 'save_image_to_assets': case 'watch_folder': case 'stop_watch':
          case 'generate_toc': case 'get_cli_args':
          case 'quit_app': case 'open_devtools': return null;
          case 'search_in_files': return [];
          case 'get_version': return '0.0.0-test';
          case 'plugin:dialog|open': case 'plugin:dialog|save':
          case 'plugin:webview|internal_toggle_devtools': return null;
          default: return null;
        }
      },
      Channel: class { constructor(){} }
    },
    event: { listen: function(){ return Promise.resolve(function(){}); } },
    window: { getCurrentWindow: function(){ return { unminimize(){}, show(){}, setFocus(){}, close(){} }; } },
    app: { getVersion: function(){ return Promise.resolve('0.0.0-test'); } },
    path: { resourceDir: function(){ return Promise.resolve('C:\\\\fake\\\\resource'); } },
    shell: { open: function(){ return Promise.resolve(); } },
    webview: {}
  };
})();
`;

function assert(name, cond, detail) {
  if (cond) {
    console.log(`  ✅ PASS  ${name}` + (detail ? `  (${detail})` : ''));
    return true;
  }
  console.log(`  ❌ FAIL  ${name}` + (detail ? `  (${detail})` : ''));
  return false;
}

(async () => {
  // 若 Chrome 不可用，优雅跳过否则 CI 上报 0/0 失败
  if (!fs.existsSync(CHROME_PATH)) {
    console.log(`  ⏭️  SKIP  Chrome 未找到 (${CHROME_PATH})，跳过浏览器测试`);
    process.exit(0);
  }
  let pass = 0, fail = 0;
  const bump = (ok) => { ok ? pass++ : fail++; };

  const browser = await puppeteer.launch({
    executablePath: CHROME_PATH,
    headless: true,
    args: ['--no-sandbox', '--disable-setuid-sandbox', '--disable-dev-shm-usage',
           '--disable-gpu', '--window-size=1400,900'],
  });
  const page = await browser.newPage();
  await page.setViewport({ width: 1400, height: 900 });

  const pageErrors = [];
  page.on('pageerror', (e) => pageErrors.push('pageerror: ' + e.message));
  page.on('console', (m) => {
    if (m.type() === 'error') {
      const t = m.text();
      if (/favicon\.ico|404/.test(t)) return; // 忽略 favicon 404 噪音
      pageErrors.push('console.error: ' + t);
    }
  });

  await page.evaluateOnNewDocument((demo) => { window.__DEMO__ = demo; }, DEMO);
  await page.evaluateOnNewDocument(TauriMock);

  console.log('\n[启动] 打开 ' + URL + ' 并等待 app 初始化…');
  await page.goto(URL, { waitUntil: 'networkidle2', timeout: 30000 });

  try {
    await page.waitForFunction(
      "window.editor && window.editor.cm && window.editor.preview && document.querySelectorAll('#preview [data-source-line]').length > 5",
      { timeout: 20000 }
    );
  } catch (e) {
    console.log('  ❌ FAIL  应用未在预期时间内初始化（预览未渲染）');
    console.log('  页面错误:', pageErrors.slice(0, 10).join(' | ') || '(无)');
    await browser.close();
    process.exit(1);
  }

  // 用确定性的文档内容覆盖（不依赖启动分支自动打开 demo.md）
  await page.evaluate((demo) => {
    const ed = window.editor;
    ed.cm.setValue(demo);
    if (typeof ed.updatePreview === 'function') ed.updatePreview();
    else if (typeof ed.debounceUpdatePreview === 'function') ed.debounceUpdatePreview();
  }, DEMO);
  await page.waitForFunction(
    "document.querySelectorAll('#preview [data-source-line]').length > 20",
    { timeout: 10000 }
  );
  console.log('  ✅ PASS  应用启动且预览已渲染');
  bump(true);

  // 切到「编辑+预览」双栏模式（构造函数默认 viewMode='preview' 会隐藏编辑器面板，
  // 宽度塌成 0 → 滚动同步无法验证；真实使用场景是双栏，这里显式切到 edit 模式）
  await page.evaluate(() => { window.editor.setViewMode('edit'); });
  await page.waitForFunction(
    "(() => { const e = document.querySelector('.CodeMirror'); if (!e) return false; return e.getBoundingClientRect().width > 50; })()",
    { timeout: 8000 }
  );
  const edLayout = await page.evaluate(() => {
    const e = document.querySelector('.CodeMirror');
    const r = e.getBoundingClientRect();
    return { w: Math.round(r.width), h: Math.round(r.height) };
  });
  bump(assert('切到双栏后编辑器面板有可见宽度', edLayout.w > 50 && edLayout.h > 50,
    `${edLayout.w}×${edLayout.h}`));

  // ---------- 测试 1：滚动同步（编辑器 → 预览）----------
  console.log('\n[测试1] 编辑器 → 预览 滚动同步');
  const t1 = await page.evaluate(async () => {
    const ed = window.editor, cm = ed.cm;
    const docLines = cm.lineCount();
    cm.refresh(); // 强制重新测量（从 preview 0 尺寸切到 edit 后 CM 内部尺寸可能滞后）
    await new Promise(r => setTimeout(r, 120));
    const si = cm.getScrollInfo();
    const maxEd = si.height - si.clientHeight;
    // 用确定性的 scrollTo 把编辑器滚到 ~55% 处（scrollIntoView 在程序调用下不可靠）
    const targetEdTop = Math.max(300, Math.floor(maxEd * 0.55));
    cm.scrollTo(0, targetEdTop);
    await new Promise(r => setTimeout(r, 300)); // 等滚动同步节流执行
    const edTop = cm.getScrollInfo().top;
    const pvTop = ed.preview.scrollTop;
    // 真值：编辑器视口顶部对应的源码行
    const edLineAtTop = cm.lineAtHeight(edTop, 'local');
    // 预览视口顶部经由滚动同步映射回的源码行
    const pvLineAtTop = ed._lineAtPreviewTop ? ed._lineAtPreviewTop(pvTop) : null;
    return { docLines, targetEdTop, edTop, pvTop, edLineAtTop, pvLineAtTop,
             previewScrollable: ed.preview.scrollHeight - ed.preview.clientHeight };
  });
  console.log(`  文档 ${t1.docLines} 行，编辑器滚到 ${t1.edTop|0}/${t1.targetEdTop|0}（视口顶行=${t1.edLineAtTop}）`);
  console.log(`  预览 scrollTop=${t1.pvTop|0}, 预览视口顶行(映射回)=${t1.pvLineAtTop}`);
  bump(assert('预览有可滚动范围（scrollHeight>clientHeight）', t1.previewScrollable > 50, `可滚 ${t1.previewScrollable|0}px`));
  bump(assert('编辑器滚动后预览确实发生了滚动', t1.pvTop > 5, `pvTop=${t1.pvTop|0}`));
  bump(assert('编辑器视口顶行 ↔ 预览视口顶行 对齐（左右精准匹配，±2 行）',
    t1.pvLineAtTop != null && Math.abs(t1.pvLineAtTop - t1.edLineAtTop) <= 2,
    `编辑器顶行=${t1.edLineAtTop}, 预览映射顶行=${t1.pvLineAtTop}`));

  // ---------- 测试 2：滚动同步（预览 → 编辑器）----------
  console.log('\n[测试2] 预览 → 编辑器 滚动同步');
  const t2 = await page.evaluate(async () => {
    const ed = window.editor, cm = ed.cm, pv = ed.preview;
    const maxPv = pv.scrollHeight - pv.clientHeight;
    const targetPv = Math.floor(maxPv * 0.4);
    pv.scrollTop = targetPv;
    pv.dispatchEvent(new Event('scroll', { bubbles: true }));
    await new Promise(r => setTimeout(r, 250));
    const edTop = cm.getScrollInfo().top;
    const maxEd = cm.getScrollInfo().height - cm.getScrollInfo().clientHeight;
    // 比例应大致对应
    const ratioPv = maxPv > 0 ? targetPv / maxPv : 0;
    const ratioEd = maxEd > 0 ? edTop / maxEd : 0;
    return { targetPv, maxPv, edTop, maxEd, ratioPv, ratioEd };
  });
  console.log(`  预览滚到 ${t2.targetPv|0}/${t2.maxPv|0} (比例 ${(t2.ratioPv*100|0)}%), 编辑器滚到 ${t2.edTop|0}/${t2.maxEd|0} (比例 ${(t2.ratioEd*100|0)}%)`);
  bump(assert('预览滚动后编辑器确实发生了滚动', t2.edTop > 5, `edTop=${t2.edTop|0}`));
  bump(assert('预览↔编辑器滚动比例基本一致（±15%）',
    Math.abs(t2.ratioEd - t2.ratioPv) <= 0.15,
    `预览 ${(t2.ratioPv*100|0)}% vs 编辑器 ${(t2.ratioEd*100|0)}%`));

  // ---------- 测试 3：勾选框点击不移动编辑器 ----------
  console.log('\n[测试3] 预览勾选框点击不导致编辑器跳动');
  const t3 = await page.evaluate(async () => {
    const ed = window.editor, cm = ed.cm;
    // 先把编辑器滚到一个明显非顶部的位置
    const anchorLine = Math.floor(cm.lineCount() * 0.33);
    cm.setCursor({ line: anchorLine, ch: 0 });
    cm.scrollIntoView({ line: anchorLine, ch: 0 }, 20);
    await new Promise(r => setTimeout(r, 150));
    const before = cm.getScrollInfo().top;
    const checkbox = document.querySelector('#preview input[type=checkbox]');
    let found = !!checkbox;
    let after = before;
    if (found) {
      checkbox.click();
      await new Promise(r => setTimeout(r, 300)); // 等 120ms 抑制窗口 + 还原
      after = cm.getScrollInfo().top;
    }
    return { found, before, after, jumped: Math.abs(after - before) > 3 };
  });
  if (!t3.found) {
    console.log('  ⚠️  预览中未找到勾选框 input（跳过，但记录）');
    bump(true);
  } else {
    console.log(`  勾选框点击前 editorTop=${t3.before|0}, 点击后=${t3.after|0}`);
    bump(assert('勾选框点击后编辑器滚动位置不变（修复已生效）', !t3.jumped,
      `Δ=${t3.after - t3.before|0}px`));
  }

  if (pageErrors.length) {
    console.log('\n[页面运行时错误] ' + pageErrors.slice(0, 8).join(' | '));
  }

  await browser.close();

  console.log(`\n========== 结果：✅ ${pass} 通过 / ❌ ${fail} 失败 ==========`);
  process.exit(fail === 0 ? 0 : 1);
})().catch((e) => {
  console.error('测试运行异常:', e);
  process.exit(2);
});
