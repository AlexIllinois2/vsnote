// 快捷键方案体系回归测试：仅 vscode / typora / sublime 三个方案（无 default/custom）。
//
// 设计原则（与项目现有测试一致）：
//   从 src/app.js 抽取真实方法（balanced-brace 抽取 + eval），在桩实例上断言
//   方案出厂键位、globalSearch 默认键、per-scheme 差异持久化、录制直接写入当前
//   方案（不再切换 custom）、单项/整方案重置、旧体系一次性迁移、预览不落盘等行为。

const { JSDOM } = require('jsdom');
const fs = require('fs');
const path = require('path');
const { execSync } = require('child_process');
const test = require('node:test');
const assert = require('node:assert');

const ROOT = path.resolve(__dirname, '..');
const APP = fs.readFileSync(path.join(ROOT, 'src', 'app.js'), 'utf8');

// localStorage 依赖
const dom = new JSDOM('<!DOCTYPE html><html><body></body></html>', { url: 'http://localhost/', pretendToBeVisual: true });
global.localStorage = dom.window.localStorage;

// ---- 从源码抽取真实方法（balanced-brace 扫描 + eval）----
function extractMethod(needle) {
  const sigIdx = APP.indexOf(needle);
  assert.ok(sigIdx !== -1, '应在 app.js 中找到: ' + needle);
  let i = APP.indexOf('{', sigIdx), depth = 0;
  for (; i < APP.length; i++) {
    const c = APP[i];
    if (c === '{') depth++;
    else if (c === '}') { depth--; if (depth === 0) break; }
  }
  const name = needle.split('(')[0].trim();
  return eval('(' + APP.slice(sigIdx, i + 1).replace(new RegExp('^\\s*' + name), 'function ' + name) + ')');
}

const getDefaultShortcuts = extractMethod('getDefaultShortcuts() {');
const getShortcutPresets = extractMethod('getShortcutPresets() {');
const buildSchemeShortcuts = extractMethod('buildSchemeShortcuts(name) {');
const _loadShortcutOverrides = extractMethod('_loadShortcutOverrides() {');
const _migrateLegacyShortcuts = extractMethod('_migrateLegacyShortcuts() {');
const loadSchemeShortcuts = extractMethod('loadSchemeShortcuts(scheme) {');
const loadShortcuts = extractMethod('loadShortcuts() {');
const saveShortcuts = extractMethod('saveShortcuts() {');
const loadShortcutScheme = extractMethod('loadShortcutScheme() {');
const applyShortcutScheme = extractMethod('applyShortcutScheme(name) {');
const previewShortcutScheme = extractMethod('previewShortcutScheme(name) {');
const resetShortcuts = extractMethod('resetShortcuts() {');
const handleShortcutRecording = extractMethod('handleShortcutRecording(e) {');
const findDuplicateShortcut = extractMethod('findDuplicateShortcut(key, excludeAction) {');
const _validConfigObject = extractMethod('_validConfigObject(raw) {');

function makeSchemeStub(scheme = 'vscode') {
  return {
    shortcuts: null,
    shortcutScheme: scheme,
    _schemePreviewing: false,
    getDefaultShortcuts,
    getShortcutPresets,
    buildSchemeShortcuts,
    _loadShortcutOverrides,
    _migrateLegacyShortcuts,
    loadSchemeShortcuts,
    loadShortcuts,
    saveShortcuts,
    loadShortcutScheme,
    saveShortcutScheme(name) { try { localStorage.setItem('tizumark-shortcut-scheme', name); } catch {} },
    applyShortcutScheme,
    previewShortcutScheme,
    resetShortcuts,
    handleShortcutRecording,
    findDuplicateShortcut,
    _validConfigObject,
    renderShortcutsList() {},
    applyShortcuts() {},
    setStatus() {},
    showToast() {},
    t: (k) => k,
  };
}

function overrides() {
  return JSON.parse(localStorage.getItem('tizumark-shortcut-overrides') || 'null');
}

// ============================================================
// B. 预置方案数据完整性
// ============================================================

test('B1 预置表含 3 个方案（vscode/typora/sublime），无 default/custom', async () => {
  const presets = getShortcutPresets();
  assert.strictEqual(Object.keys(presets).length, 3);
  assert.ok(['vscode', 'typora', 'sublime'].every(k => k in presets));
  assert.ok(!('default' in presets) && !('custom' in presets));
});

test('B2 每方案内部键位互不重复', async () => {
  for (const [name, map] of Object.entries(getShortcutPresets())) {
    const keys = Object.values(map).filter(Boolean);
    assert.strictEqual(new Set(keys).size, keys.length, name + ' 内部键位应互不重复');
  }
});

test('B3 预置方案仅引用合法 actionId', async () => {
  const ids = new Set(Object.keys(getDefaultShortcuts()));
  for (const map of Object.values(getShortcutPresets())) {
    for (const aid of Object.keys(map)) {
      assert.ok(ids.has(aid), '预置方案引用了非法 actionId: ' + aid);
    }
  }
});

// ============================================================
// C. 方案出厂键位 + per-scheme 差异表
// ============================================================

test('C1 vscode 出厂键位覆盖全部 action，globalSearch 默认 Ctrl+Shift+F', async () => {
  const s = makeSchemeStub('vscode');
  const table = s.buildSchemeShortcuts('vscode');
  assert.strictEqual(Object.keys(table).length, Object.keys(s.getDefaultShortcuts()).length);
  assert.strictEqual(table.globalSearch.key, 'Ctrl+Shift+F', 'VSCode 方案默认全局搜索快捷键');
  assert.strictEqual(table.strikethrough.key, '', '预置未列出的 action 回落空串');
});

test('C2 buildSchemeShortcuts 未知方案名回落 vscode', async () => {
  const s = makeSchemeStub();
  const table = s.buildSchemeShortcuts('hacked');
  assert.strictEqual(table.globalSearch.key, 'Ctrl+Shift+F');
  assert.strictEqual(table.saveAs.key, 'Ctrl+Shift+S');
});

test('C3 saveShortcuts 只把与出厂键位的差异写入当前方案覆盖表', async () => {
  localStorage.clear();
  const s = makeSchemeStub('typora');
  s.shortcuts = s.buildSchemeShortcuts('typora');
  s.shortcuts.insertH1 = { ...s.shortcuts.insertH1, key: 'Ctrl+7' }; // 出厂 Ctrl+1 → 修改
  s.shortcuts.bold = { ...s.shortcuts.bold, key: '' };               // 出厂 Ctrl+B → 清空
  s.shortcuts.italic = { ...s.shortcuts.italic, key: 'Ctrl+I' };     // 与出厂一致 → 不写
  s.saveShortcuts();
  assert.deepStrictEqual(overrides().typora, { insertH1: 'Ctrl+7', bold: '' },
    '覆盖表只含差异项（修改 + 清空），不含与出厂一致的项');
  assert.strictEqual(localStorage.getItem('tizumark-shortcut-scheme'), null,
    'saveShortcuts 不改写方案名');
});

test('C4 loadSchemeShortcuts = 出厂键位 + 该方案覆盖表合并', async () => {
  localStorage.clear();
  localStorage.setItem('tizumark-shortcut-overrides', JSON.stringify({ vscode: { bold: 'Ctrl+J' } }));
  const s = makeSchemeStub('vscode');
  const table = s.loadSchemeShortcuts('vscode');
  assert.strictEqual(table.bold.key, 'Ctrl+J', '差异项生效');
  assert.strictEqual(table.italic.key, 'Ctrl+I', '出厂项不受影响');
  // 其他方案的差异不串味
  localStorage.setItem('tizumark-shortcut-overrides', JSON.stringify({ typora: { bold: 'Ctrl+P' } }));
  assert.strictEqual(s.loadSchemeShortcuts('vscode').bold.key, '', 'vscode 出厂 bold 为空，不读 typora 差异');
});

// ============================================================
// D. 录制写入当前方案 + 整方案重置
// ============================================================

test('D1 手动录制直接写入当前方案，方案名不再切换 custom', async () => {
  localStorage.clear();
  const s = makeSchemeStub('vscode');
  s.shortcuts = s.loadSchemeShortcuts('vscode');
  s.recordingAction = 'bold'; // vscode 出厂 bold=''
  const handled = s.handleShortcutRecording({ key: 'J', ctrlKey: true, preventDefault() {}, stopPropagation() {} });
  assert.strictEqual(handled, true);
  assert.strictEqual(s.shortcuts.bold.key, 'Ctrl+J');
  assert.strictEqual(s.shortcutScheme, 'vscode', '录制后方案名应保持不变');
  assert.strictEqual(overrides().vscode.bold, 'Ctrl+J', '差异已持久化');
});

test('D2 resetShortcuts 恢复【当前方案】出厂键位并清空该方案差异', async () => {
  localStorage.clear();
  const s = makeSchemeStub('typora');
  s.shortcuts = s.loadSchemeShortcuts('typora');
  s.shortcuts.insertH1 = { ...s.shortcuts.insertH1, key: 'Ctrl+7' };
  s.saveShortcuts();
  s.resetShortcuts();
  assert.strictEqual(s.shortcuts.insertH1.key, 'Ctrl+1', '恢复 typora 出厂 Ctrl+1');
  assert.strictEqual(s.shortcutScheme, 'typora', '方案名不变');
  assert.deepStrictEqual(overrides().typora, {}, '该方案差异表应清空');
});

test('D3 清除键位作为差异持久化（方案名不变）', async () => {
  localStorage.clear();
  const s = makeSchemeStub('typora');
  s.shortcuts = s.loadSchemeShortcuts('typora');
  s.shortcuts.bold = { ...s.shortcuts.bold, key: '' }; // typora 出厂 bold='Ctrl+B'
  s.saveShortcuts();
  assert.strictEqual(overrides().typora.bold, '');
  assert.strictEqual(s.shortcutScheme, 'typora');
});

// ============================================================
// E. 旧体系一次性迁移 + scheme 白名单
// ============================================================

test('E1 旧体系 default/custom 用户迁移归入 vscode，旧自定义保留', async () => {
  localStorage.clear();
  localStorage.setItem('tizumark-shortcuts', JSON.stringify({ bold: { key: 'Ctrl+Z', label: '加粗' } }));
  localStorage.setItem('tizumark-shortcut-scheme', 'custom');
  const s = makeSchemeStub('vscode');
  const table = s.loadShortcuts();
  assert.strictEqual(s.shortcutScheme, 'vscode', 'custom 应迁移为 vscode');
  assert.strictEqual(table.bold.key, 'Ctrl+Z', '旧自定义键位保留');
  assert.strictEqual(localStorage.getItem('tizumark-shortcuts'), null, '旧全量表应删除');
  assert.strictEqual(overrides().vscode.bold, 'Ctrl+Z', '差异应写入覆盖表');
});

test('E2 旧体系命名方案用户迁移回原方案', async () => {
  localStorage.clear();
  localStorage.setItem('tizumark-shortcuts', JSON.stringify({ bold: { key: 'Ctrl+Z', label: '加粗' } }));
  localStorage.setItem('tizumark-shortcut-scheme', 'typora');
  const s = makeSchemeStub('typora');
  const table = s.loadShortcuts();
  assert.strictEqual(s.shortcutScheme, 'typora');
  assert.strictEqual(table.bold.key, 'Ctrl+Z', 'typora 出厂 Ctrl+B → 旧值 Ctrl+Z 为差异');
  assert.strictEqual(overrides().typora.bold, 'Ctrl+Z');
});

test('E3 迁移丢弃已废弃 actionId（findReplace/previewFind）', async () => {
  localStorage.clear();
  localStorage.setItem('tizumark-shortcuts', JSON.stringify({
    bold: { key: 'Ctrl+Z', label: '加粗' },
    findReplace: { key: 'Ctrl+R', label: '查找和替换' },
  }));
  localStorage.setItem('tizumark-shortcut-scheme', 'custom');
  const s = makeSchemeStub('vscode');
  s.loadShortcuts();
  assert.deepStrictEqual(overrides().vscode, { bold: 'Ctrl+Z' }, '废弃项不应进入覆盖表');
});

test('E4 scheme 白名单外/缺失一律回落 vscode', async () => {
  localStorage.clear();
  const s = makeSchemeStub();
  localStorage.setItem('tizumark-shortcut-scheme', 'hacked');
  assert.strictEqual(s.loadShortcutScheme(), 'vscode');
  localStorage.removeItem('tizumark-shortcut-scheme');
  assert.strictEqual(s.loadShortcutScheme(), 'vscode');
  localStorage.setItem('tizumark-shortcut-scheme', 'sublime');
  assert.strictEqual(s.loadShortcutScheme(), 'sublime');
});

test('E5 全新安装（无任何存储）：loadShortcuts 返回 vscode 出厂键位', async () => {
  localStorage.clear();
  const s = makeSchemeStub();
  const table = s.loadShortcuts();
  assert.strictEqual(s.shortcutScheme, 'vscode');
  assert.strictEqual(table.globalSearch.key, 'Ctrl+Shift+F');
});

// ============================================================
// G. previewShortcutScheme：预览不落盘、不应用 CM
// ============================================================

test('G1 previewShortcutScheme 加载键位+渲染，但不持久化、不应用 CM', async () => {
  localStorage.clear();
  const s = makeSchemeStub('vscode');
  s.shortcuts = s.loadSchemeShortcuts('vscode');
  let rendered = 0, applied = 0;
  s.renderShortcutsList = () => rendered++;
  s.applyShortcuts = () => applied++;
  s.previewShortcutScheme('typora');
  assert.strictEqual(s.shortcuts.insertH1.key, 'Ctrl+1', '应加载方案出厂键位');
  assert.strictEqual(s.shortcutScheme, 'typora');
  assert.strictEqual(s._schemePreviewing, true, '应打预览标记（供取消回滚）');
  assert.strictEqual(rendered, 1, '应渲染列表供预览');
  assert.strictEqual(applied, 0, '预览不得应用 CM');
  assert.strictEqual(localStorage.getItem('tizumark-shortcut-scheme'), null, '预览不得持久化方案名');
  assert.strictEqual(overrides(), null, '预览不得写覆盖表');
});

test('G2 previewShortcutScheme 应包含该方案已保存的差异', async () => {
  localStorage.clear();
  localStorage.setItem('tizumark-shortcut-overrides', JSON.stringify({ typora: { insertH1: 'Ctrl+9' } }));
  const s = makeSchemeStub('vscode');
  s.previewShortcutScheme('typora');
  assert.strictEqual(s.shortcuts.insertH1.key, 'Ctrl+9', '预览含该方案已有差异');
});

test('G3 applyShortcutScheme 正式生效并持久化（清除预览标记）', async () => {
  localStorage.clear();
  const s = makeSchemeStub('vscode');
  s.shortcuts = s.loadSchemeShortcuts('vscode');
  let applied = 0;
  s.applyShortcuts = () => applied++;
  s.previewShortcutScheme('typora');
  s.applyShortcutScheme('typora');
  assert.strictEqual(s._schemePreviewing, false, '生效后应清除预览标记');
  assert.strictEqual(applied, 1, '生效后应应用 CM');
  assert.strictEqual(localStorage.getItem('tizumark-shortcut-scheme'), 'typora');
});

test('G4 previewShortcutScheme 非法方案名直接忽略', async () => {
  localStorage.clear();
  const s = makeSchemeStub('vscode');
  const before = s.shortcutScheme;
  s.previewShortcutScheme('hacked');
  assert.strictEqual(s.shortcutScheme, before);
  assert.strictEqual(s._schemePreviewing, false);
});

// ============================================================
// F. 源码语法
// ============================================================

test('F1 src/app.js 通过 node --check 语法检查', async () => {
  let ok = true, msg = '';
  try {
    execSync('node --check ' + path.join(ROOT, 'src', 'app.js'));
  } catch (e) {
    ok = false; msg = e.message;
  }
  assert.ok(ok, 'app.js 语法检查失败: ' + msg);
});
