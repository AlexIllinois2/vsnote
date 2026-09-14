// 快捷键设置项整理回归测试：
// 1. previewFind / findReplace 不再是独立设置项（与 find 同一功能 toggleFindPanel）
// 2. find 显示名为「查找替换」
// 3. crossSearch 出现在设置列表中，默认键 Ctrl+H
// 4. 设置列表按功能分组渲染（shortcut-group / shortcut-group-title）
// 5. 已保存的 previewFind / findReplace 键位在 loadShortcuts 中被迁移清理

const test = require('node:test');
const assert = require('node:assert');
const { buildEnv, cleanup, waitForEditor } = require('./helpers/app-env.cjs');

test('设置项：previewFind/findReplace 不再存在，find 改名查找替换', async () => {
  const { w } = await buildEnv({ captureInitErr: true });
  try {
    await waitForEditor(w);
    const ed = w.editor;
    assert.strictEqual(ed.shortcuts.previewFind, undefined, 'previewFind 不应存在于 shortcuts');
    assert.strictEqual(ed.shortcuts.findReplace, undefined, 'findReplace 不应存在于 shortcuts');
    const defaults = ed.getDefaultShortcuts();
    assert.strictEqual(defaults.previewFind, undefined, 'previewFind 不应存在于默认方案');
    assert.strictEqual(defaults.findReplace, undefined, 'findReplace 不应存在于默认方案');
    const zhLabels = ed.t('shortcutLabel');
    assert.strictEqual(zhLabels.find, '查找替换', 'find 显示名应为「查找替换」');
    assert.strictEqual(zhLabels.previewFind, undefined, 'shortcutLabel 不应再含 previewFind');
  } finally {
    cleanup(w);
  }
});

test('设置项：crossSearch 出现在设置列表且默认 Ctrl+H', async () => {
  const { w } = await buildEnv({ captureInitErr: true });
  try {
    await waitForEditor(w);
    const ed = w.editor;
    assert.strictEqual(ed.shortcuts.crossSearch?.key, 'Ctrl+H', 'crossSearch 默认键应为 Ctrl+H');
    ed.renderShortcutsList();
    const row = w.document.querySelector('#shortcuts-list .shortcut-row[data-action="crossSearch"]');
    assert.ok(row, '设置列表中应有 crossSearch 行');
    assert.match(row.textContent, /跨文件搜索/, 'crossSearch 行应显示「跨文件搜索」');
    // previewFind 行不应存在
    const pfRow = w.document.querySelector('#shortcuts-list .shortcut-row[data-action="previewFind"]');
    assert.strictEqual(pfRow, null, '设置列表中不应有 previewFind 行');
  } finally {
    cleanup(w);
  }
});

test('设置项：列表按功能分组渲染且默认项全覆盖', async () => {
  const { w } = await buildEnv({ captureInitErr: true });
  try {
    await waitForEditor(w);
    const ed = w.editor;
    ed.renderShortcutsList();
    const groups = w.document.querySelectorAll('#shortcuts-list .shortcut-group');
    assert.ok(groups.length >= 5, `应至少有 5 个分组，实际 ${groups.length}`);
    const titles = [...w.document.querySelectorAll('#shortcuts-list .shortcut-group-title')].map(el => el.textContent.trim());
    assert.ok(titles.includes('文件'), '应有「文件」分组');
    assert.ok(titles.includes('查找与搜索'), '应有「查找与搜索」分组');
    // 所有默认设置项都应渲染出来（分组不遗漏）
    const renderedIds = new Set([...w.document.querySelectorAll('#shortcuts-list .shortcut-row')].map(el => el.dataset.action));
    for (const id of Object.keys(ed.getDefaultShortcuts())) {
      assert.ok(renderedIds.has(id), `设置项 ${id} 应在分组列表中渲染`);
    }
  } finally {
    cleanup(w);
  }
});

test('迁移：旧全量表中废弃项丢弃，用户自定义键位按差异保留', async () => {
  const { w } = await buildEnv({ captureInitErr: true });
  try {
    await waitForEditor(w);
    const ed = w.editor;
    // 模拟旧版本保存的配置（全量表，scheme 为 custom → 归入 vscode 差异表）
    w.localStorage.setItem('tizumark-shortcuts', JSON.stringify({
      previewFind: { key: 'Ctrl+Shift+P', label: '预览查找' },
      findReplace: { key: 'Ctrl+H', label: '查找和替换' },
      crossSearch: { key: 'Ctrl+Shift+F', label: '跨文件搜索' },
    }));
    const loaded = ed.loadShortcuts();
    assert.strictEqual(loaded.previewFind, undefined, '已废弃的 previewFind 应被迁移丢弃');
    assert.strictEqual(loaded.findReplace, undefined, '已废弃的 findReplace 应被迁移丢弃');
    // 旧体系会把 Ctrl+Shift+F 强制迁回 Ctrl+H；新体系尊重用户已保存的键位（作为差异保留）
    assert.strictEqual(loaded.crossSearch.key, 'Ctrl+Shift+F', '用户自定义键位应作为差异保留');
    assert.strictEqual(w.localStorage.getItem('tizumark-shortcuts'), null, '旧全量表应删除');
  } finally {
    cleanup(w);
  }
});
