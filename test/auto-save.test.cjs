// 自动保存测试：settings.autoSave（停止输入后 N 秒写盘，0=关闭）。
// 覆盖开关生效/关闭、未命名草稿不打断、外部变更待裁决时不抢写、关闭标签后不再补写、
// 设置下拉改回「关闭」时作废排队中的定时器。
// invoke 经 buildEnv 的 invokeImpl 注入以捕获命令（app.js 在 eval 时捕获 invoke 引用）。
const test = require('node:test');
const assert = require('node:assert');
const { buildEnv, cleanup, delay } = require('./helpers/app-env.cjs');

async function makeEditor(invokeImpl) {
  const { w } = await buildEnv({ captureInitErr: true, invokeImpl });
  await delay(300);
  const ed = w.editor;
  // 隔离副作用：只关心是否调用 write_file
  ed.refreshFileMeta = async () => {};
  ed.updateTabDisplay = () => {};
  ed.updatePreview = async () => {};
  ed.debounceUpdatePreview = () => {};
  ed.updateOutline = () => {};
  ed.updateWordCount = () => {};
  ed.saveSession = () => {};
  ed.setStatus = () => {};
  ed._noteFsChanged = () => {};
  return { w, ed };
}

test('autoSave: 关闭（默认）时打字不写盘', async () => {
  const calls = [];
  const { w, ed } = await makeEditor(async (cmd, args) => { calls.push({ cmd, args }); });
  try {
    ed.settings.autoSave = ed.defaultSettings().autoSave;
    await ed.addTab('a.md', 'x', 'C:/tmp/a.md');
    ed.cm.replaceRange('hello', { line: 0, ch: 0 });
    await delay(1200);
    assert.strictEqual(calls.filter(c => c.cmd === 'write_file').length, 0, '默认关闭不应自动写盘');
  } finally { cleanup(w); }
});

test('autoSave: 开启后停止输入即写盘并清掉修改标记', async () => {
  const calls = [];
  const { w, ed } = await makeEditor(async (cmd, args) => { calls.push({ cmd, args }); });
  try {
    ed.settings.autoSave = 1;
    await ed.addTab('a.md', '', 'C:/tmp/a.md');
    ed.cm.replaceRange('hello ', { line: 0, ch: 0 });
    ed.cm.replaceRange('world', { line: 0, ch: 6 });
    await delay(1400);
    const writes = calls.filter(c => c.cmd === 'write_file');
    assert.strictEqual(writes.length, 1, '连续输入应合并为一次自动保存');
    assert.strictEqual(writes[0].args.path, 'C:/tmp/a.md');
    assert.strictEqual(writes[0].args.content, 'hello world', '应写入停止输入时的最新内容');
    assert.strictEqual(ed.activeTab.isModified, false, '自动保存后不应仍是未保存状态');
  } finally { cleanup(w); }
});

test('autoSave: 未命名草稿不写盘也不弹保存框', async () => {
  const calls = [];
  const { w, ed } = await makeEditor(async (cmd, args) => { calls.push({ cmd, args }); });
  try {
    ed.settings.autoSave = 1;
    await ed.addTab('未命名', 'abc', null);
    ed.cm.replaceRange('!', { line: 0, ch: 0 });
    await delay(1400);
    assert.strictEqual(calls.filter(c => c.cmd === 'write_file').length, 0, '无路径标签不应自动写盘');
    assert.strictEqual(calls.filter(c => c.cmd === 'plugin:dialog|save').length, 0, '不应弹出保存对话框打断写作');
  } finally { cleanup(w); }
});

test('autoSave: 外部变更待用户裁决时不抢写', async () => {
  const calls = [];
  const { w, ed } = await makeEditor(async (cmd, args) => { calls.push({ cmd, args }); });
  try {
    ed.settings.autoSave = 1;
    await ed.addTab('a.md', 'x', 'C:/tmp/a.md');
    ed.cm.replaceRange('mine', { line: 0, ch: 0 });
    ed.activeTab.pendingExternalChange = true;
    await delay(1400);
    assert.strictEqual(calls.filter(c => c.cmd === 'write_file').length, 0, '冲突未决时不得覆盖磁盘内容');
  } finally { cleanup(w); }
});

test('autoSave: 标签关闭后排队中的自动保存不再补写', async () => {
  const calls = [];
  const { w, ed } = await makeEditor(async (cmd, args) => { calls.push({ cmd, args }); });
  try {
    ed.settings.autoSave = 2;
    await ed.addTab('a.md', 'x', 'C:/tmp/a.md');
    const tab = ed.activeTab;
    ed.cm.replaceRange('temp', { line: 0, ch: 0 });
    ed.showSaveDialog = async () => 'discard'; // 关闭时选择丢弃改动
    await ed.closeTab(ed.tabs.indexOf(tab));
    assert.ok(!ed.tabs.includes(tab), '标签应已关闭');
    await delay(2400);
    assert.strictEqual(calls.filter(c => c.cmd === 'write_file').length, 0, '已丢弃的改动不该被自动保存写回');
  } finally { cleanup(w); }
});

test('autoSave: 设置改回关闭时作废排队中的自动保存', async () => {
  const calls = [];
  const { w, ed } = await makeEditor(async (cmd, args) => { calls.push({ cmd, args }); });
  try {
    const sel = w.document.getElementById('set-autosave');
    assert.ok(sel, '设置弹窗应有自动保存下拉');
    sel.value = '1';
    sel.dispatchEvent(new w.Event('change'));
    assert.strictEqual(ed.settings.autoSave, 1, '下拉变更应写回设置');

    await ed.addTab('a.md', 'x', 'C:/tmp/a.md');
    ed.cm.replaceRange('draft', { line: 0, ch: 0 });

    sel.value = '0';
    sel.dispatchEvent(new w.Event('change'));
    assert.strictEqual(ed.settings.autoSave, 0);
    await delay(1400);
    assert.strictEqual(calls.filter(c => c.cmd === 'write_file').length, 0, '切回关闭后不应再自动写盘');
  } finally { cleanup(w); }
});
