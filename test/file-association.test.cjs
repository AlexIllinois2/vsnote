// 系统文件关联配置测试：txt 必须与 md 一起注册，否则「用 vsnote 打开 .txt」在
// Windows/Linux/macOS 上根本不会出现候选项，双击绑定也无从谈起。
// 关联只是入口；真正打开走的是 argv（冷启动 get_cli_args / 热启动 file-open 事件），
// 因此这里同时锁定这两条入口的载荷语义。
const fs = require('fs');
const path = require('path');
const { test } = require('node:test');
const assert = require('node:assert/strict');

const ROOT = path.join(__dirname, '..');
const conf = JSON.parse(fs.readFileSync(path.join(ROOT, 'src-tauri', 'tauri.conf.json'), 'utf8'));

function assocOf(ext) {
  return (conf.bundle.fileAssociations || []).find((a) => (a.ext || []).includes(ext));
}

test('bundle: txt 已注册为可编辑的系统文件类型', () => {
  const md = assocOf('md');
  assert.ok(md, 'md 关联必须保留');
  const txt = assocOf('txt');
  assert.ok(txt, '必须注册 txt 文件关联');
  assert.deepEqual(txt.ext, ['txt']);
  assert.equal(txt.mimeType, 'text/plain', 'Explorer / .desktop 需要正确的 MIME 类型');
  assert.equal(txt.role, 'Editor', '本应用是编辑器，应声明 Editor 角色');
  assert.ok(txt.description && txt.description.length > 0, 'Windows 资源管理器类型列显示描述');
  assert.ok(txt.name && /^[A-Za-z0-9_-]+$/.test(txt.name), 'name 作为 ProgId/CFBundleTypeName，不能含空格');
  // 不能把系统已有的 .txt 默认程序抢注成 Owner（会让用户以为记事本被换掉）
  assert.notEqual(txt.rank, 'Owner', 'rank 不应是 Owner，避免抢占 txt 默认打开方式');
});

test('bundle: 关联扩展名不得重复声明', () => {
  const seen = new Map();
  for (const a of conf.bundle.fileAssociations || []) {
    for (const ext of a.ext || []) {
      const key = String(ext).toLowerCase();
      assert.ok(!seen.has(key), `扩展名 .${ext} 在多个关联项里重复声明`);
      seen.set(key, a.name);
    }
  }
});

test('rust: 关联入口的 argv 语义（跳过 exe 自身）', () => {
  const lib = fs.readFileSync(path.join(ROOT, 'src-tauri', 'src', 'lib.rs'), 'utf8');
  // 冷启动：前端拉取 get_cli_args；热启动：单实例回调 emit file-open
  assert.match(lib, /fn get_cli_args/, '冷启动 argv 命令必须在位');
  assert.match(lib, /args\(\)\.skip\(1\)/, 'get_cli_args 应跳过 argv[0]');
  assert.match(lib, /fn files_from_args/, '单实例回调应有 argv→文件列表的提取函数');
  assert.match(lib, /\.skip\(1\)/, 'files_from_args 应跳过 argv[0]');
  assert.match(lib, /emit\("file-open"/, '二次实例应把待打开文件转发给前端');
  assert.match(lib, /tauri_plugin_single_instance::init/, '必须注册单实例插件，否则双击第二个文件只会新起一个进程');
});
