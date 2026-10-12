// 视图模式测试：初始值 / toggleViewMode 循环 / applyViewMode / togglePreviewCollapse 折叠
// 与 app-core 的 setViewMode 互补，避免重复。
const test = require('node:test');
const assert = require('node:assert');
const { buildEnv, cleanup, delay } = require('./helpers/app-env.cjs');

async function makeEditor() {
  const { w } = await buildEnv({ captureInitErr: true });
  await delay(300);
  const ed = w.editor;
  // 隔离异步 DOM 副作用，聚焦视图模式状态切换
  ed.setStatus = () => {};
  ed._resumeScroll = () => {};
  ed.cm.refresh = () => {};
  return { w, ed };
}

test('viewmode: 初始化 viewMode 继承 settings.defaultView', async () => {
  const { w, ed } = await makeEditor();
  try {
    assert.strictEqual(ed.viewMode, ed.settings.defaultView, 'viewMode 应等于默认视图设置');
  } finally { cleanup(w); }
});

test('viewmode: toggleViewMode 在 edit/preview 间循环', async () => {
  const { w, ed } = await makeEditor();
  try {
    const container = w.document.querySelector('.editor-container');
    if (ed.viewMode === 'preview') ed.setViewMode('edit');
    assert.strictEqual(ed.viewMode, 'edit');
    assert.ok(!container.classList.contains('preview-mode'));
    ed.toggleViewMode();
    assert.strictEqual(ed.viewMode, 'preview');
    assert.ok(container.classList.contains('preview-mode'), '切到 preview 应有 preview-mode 类');
    ed.toggleViewMode();
    assert.strictEqual(ed.viewMode, 'edit');
  } finally { cleanup(w); }
});

test('viewmode: applyViewMode(preview) 加 preview-mode 类并隐藏侧栏', async () => {
  const { w, ed } = await makeEditor();
  try {
    ed.setViewMode('edit');
    ed.viewMode = 'preview';
    ed.applyViewMode();
    const container = w.document.querySelector('.editor-container');
    assert.ok(container.classList.contains('preview-mode'));
    const sideRight = w.document.getElementById('btn-side-right');
    if (sideRight) assert.ok(sideRight.classList.contains('side-hidden'), 'preview 模式应隐藏侧栏按钮');
  } finally { cleanup(w); }
});

test('viewmode: togglePreviewCollapse 折叠/展开预览（edit 模式）', async () => {
  const { w, ed } = await makeEditor();
  try {
    ed.setViewMode('edit');
    const container = w.document.querySelector('.editor-container');
    ed.togglePreviewCollapse();
    assert.ok(container.classList.contains('preview-collapsed'), '应折叠预览');
    ed.togglePreviewCollapse();
    assert.ok(!container.classList.contains('preview-collapsed'), '再次切换应展开预览');
  } finally { cleanup(w); }
});

test('viewmode: 纯预览模式下 togglePreviewCollapse 不生效', async () => {
  const { w, ed } = await makeEditor();
  try {
    ed.setViewMode('preview');
    const container = w.document.querySelector('.editor-container');
    ed.togglePreviewCollapse();
    assert.ok(!container.classList.contains('preview-collapsed'), 'preview 模式下折叠预览应无效');
  } finally { cleanup(w); }
});
