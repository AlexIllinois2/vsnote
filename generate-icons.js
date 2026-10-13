const sharp = require('sharp');
const fs = require('fs');
const path = require('path');
const { Icns, IcnsImage } = require('@fiahfy/icns');

// 图标源：vsnote-logo.png（600x600，深色底 N 字 logo）
const svgBuffer = fs.readFileSync(path.join(__dirname, 'vsnote-logo.png'));

/**
 * 自行组装 ICO（不再用 to-ico）。
 * to-ico 遇到不透明 PNG 时会把位深声明成 24，却仍写入 32bpp BGRA 数据且不带 AND 掩码，
 * Windows 按 24bpp 解析 4 字节流会逐行错位 → 图标花屏。
 * 这里写标准 32bpp BGRA + 全 0 AND 掩码，位深与数据严格一致。
 */
async function renderBGRA(size) {
  const { data, info } = await sharp(svgBuffer)
    .resize(size, size, { fit: 'cover', kernel: 'lanczos3' })
    .ensureAlpha()
    .raw()
    .toBuffer({ resolveWithObject: true });

  const w = info.width;
  const h = info.height;
  const stride = w * 4;
  const flipped = Buffer.alloc(stride * h); // DIB 行序自下而上
  for (let y = 0; y < h; y++) {
    const src = y * stride;
    const dst = (h - 1 - y) * stride;
    for (let x = 0; x < w; x++) {
      flipped[dst + x * 4 + 0] = data[src + x * 4 + 2]; // B
      flipped[dst + x * 4 + 1] = data[src + x * 4 + 1]; // G
      flipped[dst + x * 4 + 2] = data[src + x * 4 + 0]; // R
      flipped[dst + x * 4 + 3] = data[src + x * 4 + 3]; // A
    }
  }
  return { w, h, pixels: flipped };
}

function buildIco(images) {
  const count = images.length;
  const header = Buffer.alloc(6);
  header.writeUInt16LE(0, 0); // reserved
  header.writeUInt16LE(1, 2); // type: icon
  header.writeUInt16LE(count, 4);

  const entries = [];
  const blobs = [];
  let offset = 6 + 16 * count;

  for (const { w, h, pixels } of images) {
    // BITMAPINFOHEADER
    const dib = Buffer.alloc(40);
    dib.writeUInt32LE(40, 0); // biSize
    dib.writeInt32LE(w, 4); // biWidth
    dib.writeInt32LE(h * 2, 8); // biHeight = XOR + AND mask
    dib.writeUInt16LE(1, 12); // biPlanes
    dib.writeUInt16LE(32, 14); // biBitCount —— 必须与 4 字节/像素的实际数据一致
    dib.writeUInt32LE(0, 16); // biCompression = BI_RGB
    dib.writeUInt32LE(pixels.length, 20); // biSizeImage
    dib.writeInt32LE(0, 24); // biXPelsPerMeter
    dib.writeInt32LE(0, 28); // biYPelsPerMeter
    dib.writeUInt32LE(0, 32); // biClrUsed
    dib.writeUInt32LE(0, 36); // biClrImportant

    // AND 掩码：每像素 1 bit，行按 4 字节对齐；全 0 = 完全不透明
    const maskRow = Math.ceil(w / 32) * 4;
    const mask = Buffer.alloc(maskRow * h);

    const blob = Buffer.concat([dib, pixels, mask]);

    const e = Buffer.alloc(16);
    e.writeUInt8(w >= 256 ? 0 : w, 0);
    e.writeUInt8(h >= 256 ? 0 : h, 1);
    e.writeUInt8(0, 2); // color count
    e.writeUInt8(0, 3); // reserved
    e.writeUInt16LE(1, 4); // planes
    e.writeUInt16LE(32, 6); // bit count
    e.writeUInt32LE(blob.length, 8);
    e.writeUInt32LE(offset, 12);
    entries.push(e);
    blobs.push(blob);
    offset += blob.length;
  }

  return Buffer.concat([header, ...entries, ...blobs]);
}

async function generateIcons() {
  // Generate PNG icons at all required sizes
  const sizes = [
    { size: 32,  dest: 'src-tauri/icons/32x32.png' },
    { size: 64,  dest: 'src/app-icon.png' },
    { size: 128, dest: 'src-tauri/icons/128x128.png' },
    { size: 128, dest: 'src-tauri/icons/128x128@2x.png' },
    { size: 256, dest: 'src-tauri/icons/icon.png' },
    { size: 256, dest: 'icon.png' },
    { size: 256, dest: 'assets/icon.png' },
  ];

  // ⚠️ 必须 .ensureAlpha()：图标源 vsnote-logo.png 是不透明的 RGB 图，
  //    sharp 默认会输出 3 通道 PNG，而 tauri 的 generate_context! 在 Linux 上
  //    校验图标必须是 RGBA，否则直接 panic：
  //      "icon .../icons/32x32.png is not RGBA"
  //    （Windows 用 .ico，不校验 PNG，所以这个问题只在 Linux/macOS 打包时暴露）
  for (const { size, dest } of sizes) {
    await sharp(svgBuffer)
      .resize(size, size)
      .ensureAlpha()
      .png()
      .toFile(path.join(__dirname, dest));
    console.log(`Generated ${dest} (${size}x${size})`);
  }

  // Generate ICO (Windows) — 自组装，位深/掩码严格正确
  const icoSizes = [16, 20, 24, 32, 40, 48, 64, 96, 128, 256];
  const images = [];
  for (const size of icoSizes) {
    images.push(await renderBGRA(size));
  }
  const icoBuffer = buildIco(images);
  fs.writeFileSync(path.join(__dirname, 'src-tauri/icons/icon.ico'), icoBuffer);
  console.log('Generated src-tauri/icons/icon.ico (' + icoSizes.join('+') + ')');

  // Generate ICNS (macOS)
  const icns = new Icns();
  const icnsSizeMap = [
    { size: 64,   osType: 'icp6' },
    { size: 128,  osType: 'ic07' },
    { size: 256,  osType: 'ic08' },
    { size: 512,  osType: 'ic09' },
    { size: 1024, osType: 'ic10' },
  ];

  for (const { size, osType } of icnsSizeMap) {
    const buf = await sharp(svgBuffer).resize(size, size).ensureAlpha().png().toBuffer();
    const img = IcnsImage.fromPNG(buf, osType);
    icns.append(img);
  }

  fs.writeFileSync(path.join(__dirname, 'src-tauri/icons/icon.icns'), Buffer.from(icns.data));
  console.log('Generated src-tauri/icons/icon.icns');

  console.log('\nAll icons generated successfully!');
}

generateIcons().catch(err => {
  console.error('Error generating icons:', err);
  process.exit(1);
});
