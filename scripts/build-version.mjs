#!/usr/bin/env node
// 构建唯一标识生成器（可复用，供 npm run build:rpm 与 build-linux-rpm.sh 共用）。
//
// 生成两类字符串：
//   1. buildVersion  应用内展示 / 对外标识：<基础版本>-<YYYYMMDD>-<8位随机码>
//                      例：1.1.0-20261108-685f2b0f
//   2. rpmRelease    RPM Release 字段：<YYYYMMDD>.<HHMMSS>.<8位随机码>
//                      例：20261108.170418.685f2b0f
//
// 为什么 RPM 不能用 buildVersion 原样：
//   - RPM 以 '-' 分隔 Version 与 Release，Version 里不允许出现 '-'；
//   - Tauri 打包器会把 semver 预发布段转成 '~'（1.1.0~20261108.xxx），
//     而 '~' 在 RPM 的比较规则里低于 '1.1.0'，会让 dnf 认为新包更旧。
// 因此：Version 始终是纯 X.Y.Z；日期/时间/随机码全部放进 Release（用 '.' 分隔）。
//
// 为什么 Release 要带 HHMMSS：
//   同一天多次构建时，仅靠 8 位随机码无法保证 rpmvercmp 严格递增；
//   把时间放在随机码之前，日期参与排序、时间保证同日内递增、随机码只做区分。
//
// 可用环境变量覆盖（便于 CI 复现同一标识）：
//   VSNOTE_BUILD_DATE=YYYYMMDD
//   VSNOTE_BUILD_TIME=HHMMSS
//   VSNOTE_BUILD_TOKEN=<8 位字母数字>
//
// CLI：
//   node scripts/build-version.mjs --json    输出 JSON
//   node scripts/build-version.mjs --shell   输出可 eval 的 shell 变量
import fs from 'node:fs';
import path from 'node:path';
import { randomBytes } from 'node:crypto';
import { fileURLToPath } from 'node:url';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const ROOT = path.resolve(__dirname, '..');
const pkg = JSON.parse(fs.readFileSync(path.join(ROOT, 'package.json'), 'utf8'));

const pad = (n, width = 2) => String(n).padStart(width, '0');

/**
 * @param {object} [options]
 * @param {string} [options.baseVersion] 纯 X.Y.Z 的基础版本，默认取 package.json
 * @param {Date}   [options.now]
 * @param {string} [options.date]  YYYYMMDD
 * @param {string} [options.time]  HHMMSS
 * @param {string} [options.token] 8 位随机码
 */
export function createBuildMeta({ baseVersion, now = new Date(), date, time, token } = {}) {
  const version = baseVersion || pkg.version;
  // 基础版本必须是纯 X.Y.Z：预发布/构建元数据会让 RPM Version 出现 '~' 或 '+'，
  // 破坏"同 Version 下靠 Release 递增"的升级模型。
  if (!/^\d+\.\d+\.\d+$/.test(version)) {
    throw new Error(`基础版本必须是纯 X.Y.Z（当前为 ${version}），构建标识只能放在 RPM Release 中`);
  }

  const buildDate = date || `${now.getFullYear()}${pad(now.getMonth() + 1)}${pad(now.getDate())}`;
  const buildTime = time || `${pad(now.getHours())}${pad(now.getMinutes())}${pad(now.getSeconds())}`;
  const buildToken = (token || randomBytes(4).toString('hex')).toLowerCase();

  if (!/^\d{8}$/.test(buildDate)) throw new Error(`构建日期格式应为 YYYYMMDD（当前为 ${buildDate}）`);
  if (!/^\d{6}$/.test(buildTime)) throw new Error(`构建时间格式应为 HHMMSS（当前为 ${buildTime}）`);
  if (!/^[0-9a-z]{8}$/.test(buildToken)) throw new Error(`构建随机码应为 8 位字母数字（当前为 ${buildToken}）`);

  const buildVersion = `${version}-${buildDate}-${buildToken}`;
  // RPM Release 不允许 '-'（会破坏 name-version-release 解析），统一用 '.'
  const rpmRelease = `${buildDate}.${buildTime}.${buildToken}`;

  return {
    baseVersion: version,
    date: buildDate,
    time: buildTime,
    token: buildToken,
    buildVersion,
    rpmRelease,
  };
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
  const meta = createBuildMeta({
    date: process.env.VSNOTE_BUILD_DATE || undefined,
    time: process.env.VSNOTE_BUILD_TIME || undefined,
    token: process.env.VSNOTE_BUILD_TOKEN || undefined,
  });
  if (process.argv.includes('--shell')) {
    console.log(`VSNOTE_BUILD_DATE='${meta.date}'`);
    console.log(`VSNOTE_BUILD_TIME='${meta.time}'`);
    console.log(`VSNOTE_BUILD_TOKEN='${meta.token}'`);
    console.log(`VSNOTE_BUILD_VERSION='${meta.buildVersion}'`);
    console.log(`VSNOTE_RPM_RELEASE='${meta.rpmRelease}'`);
  } else {
    console.log(JSON.stringify(meta, null, 2));
  }
}
