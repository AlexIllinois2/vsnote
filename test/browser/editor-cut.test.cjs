/**
 * 无头浏览器回归测试：编辑器剪切（Ctrl+X）不再误删下一行
 *
 * 背景（WebKitGTK 真机 bug）：
 *   行号点击选行产生的整行选区 [N:0, N+1:0]（含行尾换行）后按 Ctrl+X，
 *   在 Tauri/WebKitGTK 下「一行消失两行、粘贴只能恢复下一行」。
 *   根因见 src/app.js initEditor 中 cut 捕获阶段接管注释：CM5 onCopyCut 的
 *   clipboardData 回读校验在 WebKitGTK 失败 → 落入不 preventDefault 的
 *   hidden-textarea 兜底 → 原生 cut 在 CM 已删过一轮的 DOM 上继续执行，
 *   旧 DOM 选区被重锚到下一行 → 下一行被误删并写入剪贴板。
 *
 *   修复后 cut 由应用捕获阶段接管（preventDefault + 精确写剪贴板 + 仅一次删除），
 *   行为与引擎无关。本测试用真实 Chrome（Blink）+ 真实键盘事件钉死该行为。
 *
 * 运行：由 scripts/run-tests.cjs 的 browser/ 分支自动拉起 dev-server(1420) 后执行。
 *   （也可手动：node test/browser/editor-cut.test.cjs，需 dev-server 在 1420）
 */
'use strict';
const fs = require('fs');

let puppeteer;
try {
  puppeteer = require('puppeteer-core');
} catch (e) {
  console.log(`  ⏭️  SKIP  puppeteer-core 不可用 (${e.message})，跳过浏览器测试`);
  process.exit(0);
}

function resolveChromePath() {
  if (process.env.CHROME_PATH && fs.existsSync(process.env.CHROME_PATH)) return process.env.CHROME_PATH;
  const paths = [
    '/usr/bin/google-chrome', '/usr/bin/chromium-browser', '/usr/bin/chromium',
    '/usr/bin/google-chrome-stable', '/run/current-system/sw/bin/google-chrome-stable',
    '/run/current-system/sw/bin/chromium',
  ];
  for (const p of paths) { if (fs.existsSync(p)) return p; }
  return 'C:/Program Files/Google/Application/chrome.exe';
}
const CHROME_PATH = resolveChromePath();
const URL = 'http://localhost:1420/';

// 在页面任何脚本运行前注入：预设 EULA 已接受 + 完整的 Tauri IPC mock
const TauriMock = `
(function(){
  try { localStorage.setItem('tizumark-eula-accepted','true'); } catch(e){}
  window.__DEMO__ = '# test\\n';
  window.__TAURI__ = {
    core: {
      invoke: async function(cmd, args){
        args = args || {};
        switch(cmd){
          case 'read_bundled_file': return { content: window.__DEMO__ };
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
          case 'plugin:dialog|open': case 'plugin:dialog|save': return null;
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
  if (!fs.existsSync(CHROME_PATH)) {
    console.log(`  ⏭️  SKIP  Chrome 未找到 (${CHROME_PATH})，跳过浏览器测试`);
    process.exit(0);
  }
  let pass = 0, fail = 0;
  const bump = (ok) => { ok ? pass++ : fail++; };

  const browser = await puppeteer.launch({
    executablePath: CHROME_PATH,
    headless: true,
    args: ['--no-sandbox', '--disable-setuid-sandbox', '--disable-dev-shm-usage', '--disable-gpu'],
  });
  try {
    await browser.defaultBrowserContext().overridePermissions(
      'http://localhost:1420', ['clipboard-read', 'clipboard-write', 'clipboard-sanitized-write']);
    const page = await browser.newPage();
    await page.setViewport({ width: 1200, height: 800 });
    const pageErrors = [];
    page.on('pageerror', (e) => pageErrors.push('pageerror: ' + e.message));

    await page.evaluateOnNewDocument(TauriMock);
    await page.goto(URL, { waitUntil: 'networkidle2', timeout: 30000 });
    await page.waitForFunction('window.editor && window.editor.cm', { timeout: 20000 });
    await page.evaluate(() => { window.editor.setViewMode('edit'); });
    await page.waitForFunction(
      "(() => { const e = document.querySelector('.CodeMirror'); return e && e.getBoundingClientRect().width > 50; })()",
      { timeout: 8000 });
    bump(assert('应用启动且编辑器面板可见', true));

    const setContent = () => page.evaluate(() => { window.editor.cm.setValue('aaa\nbbb\nccc\nddd\n'); });
    await setContent();

    // ---- 用例 1：整行选区（行号点击语义 [N:0, N+1:0]）+ 合成 cut 事件 ----
    const r1 = await page.evaluate(() => {
      const ed = window.editor;
      ed.cm.focus();
      ed.cm.setSelection({ line: 1, ch: 0 }, { line: 2, ch: 0 }); // 选中 "bbb\n"
      const dt = new DataTransfer();
      const target = ed.cm.display.lineDiv;
      const ev = new ClipboardEvent('cut', { clipboardData: dt, bubbles: true, cancelable: true });
      target.dispatchEvent(ev);
      return { doc: ed.cm.getValue(), clip: dt.getData('text/plain'), prevented: ev.defaultPrevented };
    });
    bump(assert('合成 cut：文档只删当前行（不吞下一行）', r1.doc === 'aaa\nccc\nddd\n', `doc=${JSON.stringify(r1.doc)}`));
    bump(assert('合成 cut：剪贴板为整行文本（含换行）', r1.clip === 'bbb\n', `clip=${JSON.stringify(r1.clip)}`));
    bump(assert('合成 cut：原生 cut 被阻断（preventDefault）', r1.prevented === true));

    // ---- 用例 2：真实 Ctrl+X（可信键盘事件，贴近真机）----
    await setContent();
    await page.evaluate(() => {
      const ed = window.editor;
      ed.cm.focus();
      ed.cm.setSelection({ line: 1, ch: 0 }, { line: 2, ch: 0 }); // 选中 "bbb\n"
    });
    await page.keyboard.down('Control');
    await page.keyboard.press('KeyX');
    await page.keyboard.up('Control');
    await new Promise((r) => setTimeout(r, 300));
    const r2 = await page.evaluate(async () => {
      let clip = null;
      try { clip = await navigator.clipboard.readText(); } catch (e) { clip = 'read-err:' + e.message; }
      return { doc: window.editor.cm.getValue(), clip };
    });
    bump(assert('真实 Ctrl+X：文档只删当前行', r2.doc === 'aaa\nccc\nddd\n', `doc=${JSON.stringify(r2.doc)}`));
    bump(assert('真实 Ctrl+X：系统剪贴板为整行文本', r2.clip === 'bbb\n', `clip=${JSON.stringify(r2.clip)}`));

    // ---- 用例 3：粘贴完整恢复被剪切行 ----
    await page.keyboard.down('Control');
    await page.keyboard.press('KeyV');
    await page.keyboard.up('Control');
    await new Promise((r) => setTimeout(r, 300));
    const r3 = await page.evaluate(() => window.editor.cm.getValue());
    bump(assert('Ctrl+V 恢复被剪切的行（当前行回来了）', r3 === 'aaa\nbbb\nccc\nddd\n', `doc=${JSON.stringify(r3)}`));

    // ---- 用例 4：部分选区剪切仍精确 ----
    await setContent();
    await page.evaluate(() => {
      const ed = window.editor;
      ed.cm.focus();
      ed.cm.setSelection({ line: 1, ch: 1 }, { line: 1, ch: 2 }); // 选中 "b"
    });
    await page.keyboard.down('Control');
    await page.keyboard.press('KeyX');
    await page.keyboard.up('Control');
    await new Promise((r) => setTimeout(r, 300));
    const r4 = await page.evaluate(async () => {
      let clip = null;
      try { clip = await navigator.clipboard.readText(); } catch (e) { clip = 'read-err:' + e.message; }
      return { doc: window.editor.cm.getValue(), clip };
    });
    bump(assert('部分选区：只删选中字符', r4.doc === 'aaa\nbb\nccc\nddd\n', `doc=${JSON.stringify(r4.doc)}`));
    bump(assert('部分选区：剪贴板为选中字符', r4.clip === 'b', `clip=${JSON.stringify(r4.clip)}`));

    if (pageErrors.length) {
      console.log('\n[页面运行时错误] ' + pageErrors.slice(0, 8).join(' | '));
      bump(assert('无页面运行时错误', false));
    }
  } finally {
    await browser.close();
  }
  console.log(`\n========== 结果：✅ ${pass} 通过 / ❌ ${fail} 失败 ==========`);
  process.exit(fail === 0 ? 0 : 1);
})().catch((e) => {
  console.error('测试运行异常:', e);
  process.exit(2);
});
