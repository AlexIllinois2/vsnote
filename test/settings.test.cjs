// 设置模块测试：默认值 / 校验 / 合并 / 主题色 / 字体 / 重置
// 仅依赖共享 harness（jsdom + 真实 CodeMirror），不触及 Tauri 集成。
const test = require('node:test');
const assert = require('node:assert');
const { buildEnv, cleanup, delay } = require('./helpers/app-env.cjs');

async function makeEditor() {
  const { w, getInitErr } = await buildEnv({ captureInitErr: true });
  await delay(300); // 等异步初始化（DOMContentLoaded -> new MarkdownEditor）完成
  return { w, ed: w.editor, getInitErr };
}

test('settings: defaultSettings 返回完整默认配置', async () => {
  const { w, ed, getInitErr } = await makeEditor();
  try {
    assert.strictEqual(getInitErr(), null, '初始化不应报错');
    const d = ed.defaultSettings();
    assert.strictEqual(d.fontSize, 14);
    assert.strictEqual(d.tabSize, 4);
    assert.strictEqual(d.lineWrap, true);
    assert.strictEqual(d.lineNumbers, true);
    assert.strictEqual(d.previewFontSize, 16);
    assert.strictEqual(d.lineHeight, 1.7);
    assert.strictEqual(d.maxWidth, 0);
    assert.strictEqual(d.themeMode, 'light');
    assert.strictEqual(d.colorScheme, 'default');
    assert.strictEqual(d.fontScheme, 'system-sans');
    assert.strictEqual(d.defaultView, 'edit');
    assert.strictEqual(d.scrollSync, true);
    assert.strictEqual(d.language, 'zh');
    assert.strictEqual(d.imageInsertMode, 'assets');
    assert.strictEqual(d.imageAssetPath, 'assets');
    assert.strictEqual(d.imageAssetPathMode, 'relative');
    assert.strictEqual(d.outlineWidth, 240);
    assert.strictEqual(d.codeLineNumbers, false);
    assert.strictEqual(d.codeWrap, false);
    assert.strictEqual(d.softBreaks, true);
    assert.strictEqual(d.closeAction, 'ask');
    assert.strictEqual(d.toolbarCollapsed, false);
    assert.strictEqual(d.sidebarHidden, false);
    assert.ok(Array.isArray(d.customFonts), 'customFonts 应为数组');
    assert.strictEqual(d.customFonts.length, 0, 'customFonts 应为空');
    assert.strictEqual(d.editorFont, '');
    assert.strictEqual(d.previewFont, '');
  } finally { cleanup(w); }
});

test('settings: _validConfigObject 类型校验', async () => {
  const { w, ed } = await makeEditor();
  try {
    assert.strictEqual(ed._validConfigObject(null), null);
    assert.strictEqual(ed._validConfigObject(undefined), null);
    assert.strictEqual(ed._validConfigObject([1, 2, 3]), null, '数组应被拒绝');
    assert.strictEqual(ed._validConfigObject('nope'), null, '字符串应被拒绝');
    assert.deepStrictEqual(ed._validConfigObject({ a: 1 }), { a: 1 });
  } finally { cleanup(w); }
});

test('settings: loadSettings 合并已知字段并保留默认', async () => {
  const { w, ed } = await makeEditor();
  try {
    w.localStorage.setItem('tizumark-settings', JSON.stringify({ tabSize: 2, themeMode: 'dark' }));
    const loaded = ed.loadSettings();
    assert.strictEqual(loaded.tabSize, 2);
    assert.strictEqual(loaded.themeMode, 'dark');
    assert.strictEqual(loaded.fontSize, 14, '未提供字段应保留默认');
    assert.strictEqual(loaded.maxWidth, 0);
  } finally { cleanup(w); }
});

test('settings: loadSettings 对类型不符字段回退默认', async () => {
  const { w, ed } = await makeEditor();
  try {
    w.localStorage.setItem('tizumark-settings', JSON.stringify({ tabSize: '4', lineWrap: 'yes' }));
    const loaded = ed.loadSettings();
    assert.strictEqual(loaded.tabSize, 4, '字符串应回退为 number 默认');
    assert.strictEqual(loaded.lineWrap, true, '字符串应回退为 boolean 默认');
  } finally { cleanup(w); }
});

test('settings: loadSettings 丢弃未知键', async () => {
  const { w, ed } = await makeEditor();
  try {
    w.localStorage.setItem('tizumark-settings', JSON.stringify({ bogusKey: 'hello' }));
    const loaded = ed.loadSettings();
    assert.strictEqual(loaded.bogusKey, undefined, '未知键应被类型校验清为 undefined');
  } finally { cleanup(w); }
});

test('settings: loadSettings 旧版 colorScheme 推断 fontScheme', async () => {
  const { w, ed } = await makeEditor();
  try {
    w.localStorage.setItem('tizumark-settings', JSON.stringify({ colorScheme: 'forest' }));
    const loaded = ed.loadSettings();
    assert.strictEqual(loaded.colorScheme, 'forest');
    assert.strictEqual(loaded.fontScheme, 'system-sans', '旧版无 fontScheme 时应从 colorScheme 推断');
  } finally { cleanup(w); }
});

test('settings: loadSettings 坏 JSON 回退默认', async () => {
  const { w, ed } = await makeEditor();
  try {
    w.localStorage.setItem('tizumark-settings', 'not-json{');
    const loaded = ed.loadSettings();
    assert.strictEqual(loaded.fontSize, 14, '坏 JSON 应回退默认');
    assert.strictEqual(loaded.colorScheme, 'default');
  } finally { cleanup(w); }
});

test('settings: saveSettings 回写 localStorage', async () => {
  const { w, ed } = await makeEditor();
  try {
    ed.settings = ed.defaultSettings();
    ed.settings.tabSize = 2;
    ed.saveSettings();
    const stored = JSON.parse(w.localStorage.getItem('tizumark-settings'));
    assert.strictEqual(stored.tabSize, 2);
  } finally { cleanup(w); }
});

test('settings: applyThemeMode dark 设置 data-theme 与 cm 主题', async () => {
  const { w, ed } = await makeEditor();
  try {
    ed.settings.themeMode = 'dark';
    ed.settings.colorScheme = 'nord';
    await ed.applyThemeMode();
    assert.strictEqual(w.document.documentElement.getAttribute('data-theme'), 'dark');
    assert.strictEqual(w.document.documentElement.getAttribute('data-color-scheme'), 'nord');
    assert.strictEqual(ed.cm.getOption('theme'), 'material-darker');
  } finally { cleanup(w); }
});

test('settings: applyThemeMode system 跟随 matchMedia(返回 light)', async () => {
  const { w, ed } = await makeEditor();
  try {
    ed.settings.themeMode = 'system';
    await ed.applyThemeMode();
    assert.strictEqual(w.document.documentElement.getAttribute('data-theme'), 'light', 'matchMedia 默认 false 应为 light');
  } finally { cleanup(w); }
});

test('settings: applyFontScheme 设置 data-font-scheme', async () => {
  const { w, ed } = await makeEditor();
  try {
    ed.settings.fontScheme = 'classic-serif';
    ed.applyFontScheme();
    assert.strictEqual(w.document.documentElement.getAttribute('data-font-scheme'), 'classic-serif');
  } finally { cleanup(w); }
});

test('settings: applyCustomFonts 设置编辑器/预览字体族', async () => {
  const { w, ed } = await makeEditor();
  try {
    ed.settings.editorFont = 'cf123';
    ed.settings.previewFont = 'cf456';
    ed.applyCustomFonts();
    assert.ok(ed.cm.getWrapperElement().style.fontFamily.includes('tizumark-custom-cf123'), 'CM 应应用自定义字体 cf123，实际: ' + ed.cm.getWrapperElement().style.fontFamily);
    assert.ok(ed.preview.style.fontFamily.includes('tizumark-custom-cf456'), '预览应应用自定义字体 cf456，实际: ' + ed.preview.style.fontFamily);
  } finally { cleanup(w); }
});

test('settings: applySettings 应用 maxWidth 与代码块选项', async () => {
  const { w, ed } = await makeEditor();
  try {
    ed.settings.maxWidth = 800;
    ed.settings.codeLineNumbers = true;
    ed.settings.codeWrap = true;
    await ed.applySettings();
    assert.ok(ed.preview.classList.contains('max-width-active'), 'maxWidth>0 应加 max-width-active');
    assert.strictEqual(ed.preview.style.maxWidth, '800px');
    assert.ok(ed.preview.classList.contains('code-line-numbers'));
    assert.ok(ed.preview.classList.contains('code-wrap'));
  } finally { cleanup(w); }
});

test('settings: applySettings maxWidth=0 移除限制', async () => {
  const { w, ed } = await makeEditor();
  try {
    ed.settings.maxWidth = 0;
    await ed.applySettings();
    assert.ok(!ed.preview.classList.contains('max-width-active'), 'maxWidth=0 不应有 max-width-active');
    assert.strictEqual(ed.preview.style.maxWidth, '');
  } finally { cleanup(w); }
});

test('settings: resetSettings 恢复默认并保留自定义字体', async () => {
  const { w, ed } = await makeEditor();
  try {
    // 预置 resetSettings 操作的设置表单元素，避免 jsdom 下 null 崩溃
    const ids = ['set-font-size', 'font-size-label', 'set-tab-size', 'set-line-wrap', 'set-line-numbers',
      'set-preview-font-size', 'preview-font-size-label', 'set-line-height', 'set-max-width', 'set-theme-mode',
      'set-color-scheme', 'set-font-scheme', 'set-default-view', 'set-scroll-sync', 'set-soft-breaks',
      'set-language', 'settings-image-asset-path', 'setting-image-asset-path-hint-text'];
    for (const id of ids) {
      if (!w.document.getElementById(id)) {
        const el = w.document.createElement('input');
        el.id = id;
        w.document.body.appendChild(el);
      }
    }
    let storeMode = w.document.getElementById('settings-image-store-mode');
    if (!storeMode) { storeMode = w.document.createElement('div'); storeMode.id = 'settings-image-store-mode'; w.document.body.appendChild(storeMode); }
    storeMode.innerHTML = '<input type="radio" value="assets">';
    let pathMode = w.document.getElementById('settings-image-asset-path-mode');
    if (!pathMode) { pathMode = w.document.createElement('div'); pathMode.id = 'settings-image-asset-path-mode'; w.document.body.appendChild(pathMode); }
    pathMode.innerHTML = '<input type="radio" value="relative">';

    ed.settings = ed.defaultSettings();
    ed.settings.customFonts = [{ id: 'cf1', name: 'A', fileName: 'a.ttf', hash: 'h' }];
    ed.settings.tabSize = 8; // 偏离默认
    ed.showConfirmDialog = async () => true;
    ed.setStatus = () => {};
    ed.renderCustomFontSettings = () => {};
    await ed.resetSettings();
    assert.strictEqual(ed.settings.tabSize, 4, '应回到默认 tabSize');
    assert.strictEqual(ed.settings.customFonts.length, 1, '自定义字体应保留');
    assert.strictEqual(ed.settings.customFonts[0].id, 'cf1');
    const stored = JSON.parse(w.localStorage.getItem('tizumark-settings'));
    assert.strictEqual(stored.tabSize, 4);
  } finally { cleanup(w); }
});

test('settings: saveSettings→loadSettings 端到端一致（真往返）', async () => {
  const { w, ed } = await makeEditor();
  try {
    // 构造一份偏离默认的配置
    ed.settings = ed.defaultSettings();
    ed.settings.tabSize = 2;
    ed.settings.themeMode = 'dark';
    ed.settings.colorScheme = 'nord';
    ed.settings.maxWidth = 720;
    ed.settings.customFonts = [{ id: 'cf9', name: 'X', fileName: 'x.ttf', hash: 'h9' }];
    ed.saveSettings();

    // 模拟"重启"：清空内存对象，仅从 localStorage 重新载入
    ed.settings = null;
    const loaded = ed.loadSettings();

    assert.strictEqual(loaded.tabSize, 2, '保存的 tabSize 应原样取回');
    assert.strictEqual(loaded.themeMode, 'dark');
    assert.strictEqual(loaded.colorScheme, 'nord');
    assert.strictEqual(loaded.maxWidth, 720);
    assert.strictEqual(loaded.fontSize, 14, '未改动的字段应仍是默认');
    // customFonts 逐字段比较：JSON 往返后元素键序可能变化，deepStrictEqual 对键序敏感会误报
    assert.strictEqual(loaded.customFonts.length, 1, 'customFonts 应整组保留');
    assert.strictEqual(loaded.customFonts[0].id, 'cf9');
    assert.strictEqual(loaded.customFonts[0].name, 'X');
    assert.strictEqual(loaded.customFonts[0].fileName, 'x.ttf');
    assert.strictEqual(loaded.customFonts[0].hash, 'h9');
    // 持久层内容应与内存一致
    const stored = JSON.parse(w.localStorage.getItem('tizumark-settings'));
    assert.strictEqual(stored.tabSize, 2);
    assert.strictEqual(stored.maxWidth, 720);
  } finally { cleanup(w); }
});
