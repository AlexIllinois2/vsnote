// 文件搜索（Ctrl+P）hover 选中回归测试：
//  1. 列表重绘（输入筛选/重开对话框）时，Chromium 会对静止光标下的新元素派发合成
//     mouseenter —— 若无过滤会"自动选中"光标碰巧停留的文件，干扰键盘操作。
//     修复方案：全局 mousemove 记录真实光标坐标，mouseenter 坐标未变化即判定为合成事件并忽略。
//  2. 用户真实移动鼠标（坐标变化）时 hover 选中行为保留，Enter 打开 hover 中的文件。
const test = require('node:test');
const assert = require('node:assert');
const { withEditor, delay } = require('./helpers/app-env.cjs');

test('文件搜索：静止光标下的合成 mouseenter 不应自动选中', async () => {
  await withEditor({
    captureInitErr: true,
    invokeImpl: (cmd) => {
      if (cmd === 'list_dir') {
        return [
          { name: 'alpha.md', is_dir: false, path: '/ws/alpha.md' },
          { name: 'beta.md', is_dir: false, path: '/ws/beta.md' },
        ];
      }
      return undefined;
    },
  }, async (w, ed) => {
    ed.workspaceFolder = '/ws';
    let opened = null;
    ed.openFilePath = (p) => { opened = p; };

    // 场景 1：无任何鼠标交互时，Enter 打开第一个结果（原行为保留）
    ed.openFileSearchDialog();
    await delay(50); // 等待 fsScanWorkspace 异步渲染
    const input = w.document.getElementById('file-search-input');
    input.dispatchEvent(new w.KeyboardEvent('keydown', { key: 'Enter', bubbles: true, cancelable: true }));
    assert.strictEqual(opened, '/ws/alpha.md', '未选中时 Enter 应打开第一个结果');
    assert.ok(w.document.getElementById('file-search-dialog').classList.contains('hidden'),
      '打开文件后对话框应关闭');

    // 重新打开对话框（列表重绘，光标静止在结果区）
    ed.openFileSearchDialog();
    await delay(50);
    const list = w.document.getElementById('file-search-list');
    assert.strictEqual(list.querySelectorAll('.file-search-item').length, 2, '前置：列表应渲染 2 个文件');

    // 模拟鼠标此前真实移动过，静止停在第一个结果上 (100, 100)
    w.document.dispatchEvent(new w.MouseEvent('mousemove', { clientX: 100, clientY: 100, bubbles: true }));

    // 模拟列表重绘触发的合成 mouseenter：坐标与真实位置完全一致 → 不应选中
    const first = list.querySelectorAll('.file-search-item')[0];
    first.dispatchEvent(new w.MouseEvent('mouseenter', { clientX: 100, clientY: 100 }));
    assert.strictEqual(list.querySelector('.selected'), null, '坐标未变化的 mouseenter（重绘合成事件）不应选中任何项');

    // 模拟用户真实移动鼠标到第二个结果（坐标变化）→ hover 选中恢复
    const second = list.querySelectorAll('.file-search-item')[1];
    second.dispatchEvent(new w.MouseEvent('mouseenter', { clientX: 100, clientY: 140 }));
    const sel = list.querySelector('.selected');
    assert.ok(sel, '真实鼠标移动应产生 hover 选中');
    assert.strictEqual(sel.dataset.index, '1', '应选中鼠标真实移动到的第二项');

    // hover 选中后 Enter 打开选中的文件
    input.dispatchEvent(new w.KeyboardEvent('keydown', { key: 'Enter', bubbles: true, cancelable: true }));
    assert.strictEqual(opened, '/ws/beta.md', 'hover 选中后 Enter 应打开选中的文件');
  });
});
