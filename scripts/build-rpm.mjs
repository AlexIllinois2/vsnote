#!/usr/bin/env node
// 可复用的 Linux RPM 构建入口：自动生成唯一构建标识 → 只打 RPM → 校验产物与元信息。
//
// 用法：
//   npm run build:rpm
//
// 内存受限构建（建议）：
//   CARGO_BUILD_JOBS=2 systemd-run --user --scope -p MemoryMax=7G \
//     -p MemorySwapMax=8G --setenv=CARGO_BUILD_JOBS=2 npm run build:rpm
//
// 关键设计：
//   - tauri.conf.json / Cargo.toml 的 version 保持纯 X.Y.Z（不写入构建后缀）。
//     应用内展示的完整版本由 VSNOTE_BUILD_VERSION 编译期注入，见 src-tauri/build.rs；
//   - 构建标识写进 RPM Release（通过 --config 临时覆盖，不改动 tauri.conf.json）：
//       Version: 1.1.0
//       Release: YYYYMMDD.HHMMSS.<8位随机码>
//   - 日期在最前、随机码在最后，保证 dnf 看到的 EVR 单调递增，同一天重复构建也能升级。
import fs from 'node:fs';
import path from 'node:path';
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import { createBuildMeta } from './build-version.mjs';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const ROOT = path.resolve(__dirname, '..');
const pkg = JSON.parse(fs.readFileSync(path.join(ROOT, 'package.json'), 'utf8'));

const meta = createBuildMeta({
  date: process.env.VSNOTE_BUILD_DATE || undefined,
  time: process.env.VSNOTE_BUILD_TIME || undefined,
  token: process.env.VSNOTE_BUILD_TOKEN || undefined,
});

const releaseDir = path.join(ROOT, 'src-tauri', 'target', 'release');
const expectedRpm = path.join(
  releaseDir,
  'bundle',
  'rpm',
  `${pkg.name}-${meta.baseVersion}-${meta.rpmRelease}.x86_64.rpm`
);

console.log(`[build:rpm] 构建标识 : ${meta.buildVersion}`);
console.log(`[build:rpm] RPM EVR  : ${meta.baseVersion}-${meta.rpmRelease}`);
console.log(`[build:rpm] 内存参数 : CARGO_BUILD_JOBS=${process.env.CARGO_BUILD_JOBS || '(未设置)'}`);

// 强制重建 vsnote 的 build script：tauri 通过 build script 把 frontendDist 内嵌进二进制，
// 只改前端时 cargo 可能认为无需重跑，导致装出来的还是旧界面。
for (const sub of ['build', '.fingerprint']) {
  const dir = path.join(releaseDir, sub);
  if (!fs.existsSync(dir)) continue;
  for (const entry of fs.readdirSync(dir)) {
    if (entry.startsWith('vsnote-')) {
      fs.rmSync(path.join(dir, entry), { recursive: true, force: true });
    }
  }
}

// 只覆盖 RPM Release，不落盘改动 tauri.conf.json；restore 由 Tauri CLI 自己完成。
const configOverlay = JSON.stringify({
  bundle: { linux: { rpm: { release: meta.rpmRelease } } },
});

const tauriBin = path.join(ROOT, 'node_modules', '.bin', 'tauri');
const build = spawnSync(
  tauriBin,
  ['build', '--bundles', 'rpm', '--config', configOverlay],
  {
    cwd: ROOT,
    stdio: 'inherit',
    env: { ...process.env, VSNOTE_BUILD_VERSION: meta.buildVersion },
  }
);

if (build.error) {
  console.error(`[build:rpm] 无法启动 tauri CLI: ${build.error.message}`);
  process.exit(1);
}
if (build.status !== 0) {
  console.error(`[build:rpm] 构建失败，退出码 ${build.status}`);
  process.exit(build.status ?? 1);
}

// 兜底：若命名与预期不符（例如 productName/arch 变化），取目录中最新的 rpm。
let rpmPath = expectedRpm;
if (!fs.existsSync(rpmPath)) {
  const rpmDir = path.dirname(expectedRpm);
  const candidates = fs.existsSync(rpmDir)
    ? fs.readdirSync(rpmDir).filter((f) => f.endsWith('.rpm'))
        .map((f) => path.join(rpmDir, f))
        .sort((a, b) => fs.statSync(b).mtimeMs - fs.statSync(a).mtimeMs)
    : [];
  if (candidates.length === 0) {
    console.error(`[build:rpm] 未找到 RPM 产物，检查 ${rpmDir}`);
    process.exit(1);
  }
  rpmPath = candidates[0];
  console.warn(`[build:rpm] 预期文件名不存在，改用最新产物: ${rpmPath}`);
}

const size = fs.statSync(rpmPath).size;
const query = spawnSync(
  'rpm',
  ['-qp', '--qf', '%{NAME}|%{VERSION}|%{RELEASE}|%{ARCH}', rpmPath],
  { encoding: 'utf8' }
);
const evr = query.status === 0 ? query.stdout.trim() : '(rpm 查询失败)';
const fields = evr.split('|');

// 产物文件名、包元信息必须与生成的标识一致，防止退回默认 Release=1。
const ok = fields[0] === pkg.name
  && fields[1] === meta.baseVersion
  && fields[2] === meta.rpmRelease
  && fields[3] === 'x86_64';

console.log('');
console.log('[build:rpm] 产物路径 : ' + rpmPath);
console.log(`[build:rpm] 文件大小 : ${size} 字节（${(size / 1024 / 1024).toFixed(2)} MiB）`);
console.log(`[build:rpm] 包元信息 : ${evr}`);
console.log(`[build:rpm] 应用内版本: ${meta.buildVersion}（About 对话框展示）`);
console.log('[build:rpm] 升级验证 : sudo dnf install ' + path.relative(process.cwd(), rpmPath));

if (!ok) {
  console.error('[build:rpm] 校验失败：包元信息与构建标识不一致（可能被默认 Release 覆盖）');
  process.exit(1);
}
