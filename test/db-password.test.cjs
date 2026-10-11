// 加密 SQLite db 的密码流程回归测试。
//
// 背景 bug：启动恢复会话时 restoreSession → _ensureDbOpen 会弹密码框，而开机加载遮罩
// #loading-overlay 是 z-index:10000、.dialog-overlay 只有 1000 —— 密码框被整片盖住点不到，
// 表现成「打开带密码的 db 就卡死」（要等 20s 安全定时器强制隐藏遮罩才露出来）。
// 因此除了交互流程，这里还锁住「密码框必须高于加载遮罩」这条样式契约。
const test = require('node:test');
const assert = require('node:assert');
const fs = require('fs');
const path = require('path');
const { buildEnv, cleanup, delay, waitForEditor, ROOT } = require('./helpers/app-env.cjs');

const DB = 'C:/notes/secret.db';

async function waitVisible(w, id, timeoutMs = 5000) {
  const el = w.document.getElementById(id);
  const start = Date.now();
  while (el.classList.contains('hidden')) {
    if (Date.now() - start > timeoutMs) throw new Error(`${id} 未在 ${timeoutMs}ms 内显示`);
    await delay(10);
  }
  return el;
}

// 后端桩：db_open 按「有无口令」决定返回 PasswordRequired 还是成功；
// autoUnlock=true 模拟本机已记住派生密钥、无口令也能直接解锁。
function mkInvoke(st) {
  return (cmd, args) => {
    st.calls.push([cmd, args]);
    if (cmd === 'get_cli_args') return [];
    if (cmd === 'db_open') {
      if (args.password) return Promise.resolve({ encrypted: true, created: false });
      if (st.autoUnlock) return Promise.resolve({ encrypted: true, created: false });
      return Promise.reject(JSON.stringify({ kind: 'PasswordRequired', path: args.path, message: '该 SQLite db 已加密，需要密码' }));
    }
    if (cmd === 'db_remember_password') { st.remembered = true; return Promise.resolve(null); }
    if (cmd === 'db_forget_password') return Promise.resolve(st.hasCached === true);
    return undefined;
  };
}

test('密码框 z-index 必须高于启动加载遮罩（防再次卡死在加载页）', () => {
  const css = fs.readFileSync(path.join(ROOT, 'src', 'styles.css'), 'utf8');
  const loadingZ = Number(css.match(/\.loading-overlay\s*\{[^}]*?z-index:\s*(\d+)/)[1]);
  const pwRule = css.match(/#db-password-dialog\.dialog-overlay[^{]*\{([^}]*)\}/);
  assert.ok(pwRule, 'styles.css 应有 #db-password-dialog.dialog-overlay 规则');
  const pwZ = Number(pwRule[1].match(/z-index:\s*(\d+)/)[1]);
  assert.ok(pwZ > loadingZ, `密码框(${pwZ}) 必须高于加载遮罩(${loadingZ})，否则启动时被盖住点不到`);
});

test('手输密码通过后询问是否记住，点「记住」调用 db_remember_password', async () => {
  const st = { calls: [], remembered: false };
  const { w } = await buildEnv({ invokeImpl: mkInvoke(st) });
  const ed = await waitForEditor(w);

  const done = ed._ensureDbOpen(DB);
  await waitVisible(w, 'db-password-dialog');
  w.document.getElementById('db-password-dialog-input').value = 'hunter2';
  w.document.getElementById('db-password-dialog-confirm').click();

  await waitVisible(w, 'confirm-dialog');
  assert.strictEqual(w.document.getElementById('confirm-dialog-confirm').textContent, '记住',
    '确认按钮应为「记住」而非通用「确认」');
  assert.strictEqual(w.document.getElementById('confirm-dialog-cancel').textContent, '不记住');
  w.document.getElementById('confirm-dialog-confirm').click();

  assert.strictEqual(await done, true, '密码通过后应打开成功');
  assert.strictEqual(st.remembered, true, '点「记住」应调用 db_remember_password');
  cleanup(w);
});

test('点「不记住」不落盘凭据，但 db 仍正常打开', async () => {
  const st = { calls: [], remembered: false };
  const { w } = await buildEnv({ invokeImpl: mkInvoke(st) });
  const ed = await waitForEditor(w);

  const done = ed._ensureDbOpen(DB);
  await waitVisible(w, 'db-password-dialog');
  w.document.getElementById('db-password-dialog-input').value = 'hunter2';
  w.document.getElementById('db-password-dialog-confirm').click();
  await waitVisible(w, 'confirm-dialog');
  w.document.getElementById('confirm-dialog-cancel').click();

  assert.strictEqual(await done, true);
  assert.strictEqual(st.remembered, false, '「不记住」不应写入缓存');
  assert.ok(!st.calls.some(([c]) => c === 'db_remember_password'));
  cleanup(w);
});

test('本机已记住密钥时自动解锁，不再弹询问框打扰', async () => {
  const st = { calls: [], remembered: false, autoUnlock: true };
  const { w } = await buildEnv({ invokeImpl: mkInvoke(st) });
  const ed = await waitForEditor(w);

  assert.strictEqual(await ed._ensureDbOpen(DB), true);
  await delay(50);
  assert.ok(w.document.getElementById('db-password-dialog').classList.contains('hidden'),
    '自动解锁不该弹密码框');
  assert.ok(w.document.getElementById('confirm-dialog').classList.contains('hidden'),
    '自动解锁不该弹「记住密码」询问');
  assert.strictEqual(st.remembered, false);
  cleanup(w);
});

test('密码框被取消时放弃打开，且不问是否记住', async () => {
  const st = { calls: [], remembered: false };
  const { w } = await buildEnv({ invokeImpl: mkInvoke(st) });
  const ed = await waitForEditor(w);

  const done = ed._ensureDbOpen(DB);
  await waitVisible(w, 'db-password-dialog');
  w.document.getElementById('db-password-dialog-cancel').click();

  assert.strictEqual(await done, false, '取消密码框应返回打开失败');
  await delay(50);
  assert.ok(w.document.getElementById('confirm-dialog').classList.contains('hidden'));
  cleanup(w);
});

test('「清除已记住的密码」只对 db 工作区生效并回传是否有缓存', async () => {
  const st = { calls: [], remembered: false, hasCached: true };
  const { w } = await buildEnv({ invokeImpl: mkInvoke(st) });
  const ed = await waitForEditor(w);

  ed.workspaceFolder = DB;
  await ed.forgetRememberedDbPassword();
  const call = st.calls.find(([c]) => c === 'db_forget_password');
  assert.ok(call, '应调用 db_forget_password');
  assert.strictEqual(call[1].path, DB);

  // 非 db 工作区（文件夹 / 未打开）不应发无意义的清除请求
  st.calls.length = 0;
  ed.workspaceFolder = 'C:/notes/plain-folder';
  await ed.forgetRememberedDbPassword();
  assert.strictEqual(st.calls.length, 0, '文件夹工作区应直接跳过');
  cleanup(w);
});
