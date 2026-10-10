#!/usr/bin/env python
# -*- coding: utf-8 -*-
"""
检查本机 Microsoft Edge WebView2 Runtime 是否完好。

用途：vsnote 若"双击后闪退"并伴随系统弹窗
      "msedgewebview2.exe - 损坏的映像 / xxx.dll 没有被指定在 Windows 上运行"，
      用本脚本确认是不是 WebView2 Runtime 的文件损坏。

用法：
    python scripts/check-webview2.py

退出码：0 = 完好；1 = 发现损坏文件（需要修复 WebView2）
"""

import glob
import os
import struct
import sys

CANDIDATES = [
    r"C:\Program Files (x86)\Microsoft\EdgeWebView\Application",
    r"C:\Program Files\Microsoft\EdgeWebView\Application",
    os.path.expandvars(r"%LOCALAPPDATA%\Microsoft\EdgeWebView\Application"),
]


def is_valid_pe(path):
    """读 PE 头，判断是否是可加载的 Windows 映像。"""
    try:
        with open(path, "rb") as f:
            head = f.read(4096)
        if len(head) < 64 or head[:2] != b"MZ":
            return False
        e_lfanew = struct.unpack("<I", head[0x3C:0x40])[0]
        if e_lfanew + 24 > len(head):
            # 头不在前 4K 内，再读一次完整文件头
            with open(path, "rb") as f:
                head = f.read(e_lfanew + 24)
            if len(head) < e_lfanew + 24:
                return False
        return head[e_lfanew:e_lfanew + 4] == b"PE\0\0"
    except OSError:
        return False


def main():
    found_root = None
    for root in CANDIDATES:
        if os.path.isdir(root):
            found_root = root
            break

    if not found_root:
        print("[错误] 本机没有找到 WebView2 Runtime 安装目录。")
        print("       请从 https://developer.microsoft.com/microsoft-edge/webview2/ 下载安装。")
        return 1

    print("WebView2 Runtime 目录: %s" % found_root)

    versions = sorted(
        d for d in glob.glob(os.path.join(found_root, "*")) if os.path.isdir(d)
    )
    if not versions:
        print("[错误] 目录下没有找到任何版本。")
        return 1

    total_bad = 0
    for ver in versions:
        files = (
            glob.glob(os.path.join(ver, "*.dll"))
            + glob.glob(os.path.join(ver, "*.exe"))
        )
        bad = [f for f in files if not is_valid_pe(f)]
        print("\n版本 %s：共 %d 个 PE 文件，损坏 %d 个" % (os.path.basename(ver), len(files), len(bad)))
        for f in bad[:20]:
            print("    [损坏] %s (%d 字节)" % (os.path.basename(f), os.path.getsize(f)))
        if len(bad) > 20:
            print("    ... 另有 %d 个" % (len(bad) - 20))
        total_bad += len(bad)

    print("\n" + "=" * 60)
    if total_bad == 0:
        print("结论：WebView2 Runtime 文件完整。")
        print("      若 vsnote 仍闪退，问题不在运行时，请查看")
        print("      %%LOCALAPPDATA%%\\com.tizu.vsnote\\logs\\ 下的日志。")
        return 0

    print("结论：WebView2 Runtime 已损坏（%d 个文件不是有效的 Windows 映像）。" % total_bad)
    print("      所有基于 WebView2 的程序（vsnote、Edge 小组件、Teams 等）都会受影响。")
    print("\n修复方法（任选其一）：")
    print("  1) 设置 → 应用 → 已安装的应用 → 搜 \"Microsoft Edge WebView2 Runtime\"")
    print("     → 高级选项/修改 → 修复（需要管理员权限）")
    print("  2) 卸载后重新安装：")
    print("     https://go.microsoft.com/fwlink/p/?LinkId=2124703")
    print("  3) 管理员命令行：")
    print("     DISM /Online /Cleanup-Image /RestoreHealth")
    print("     sfc /scannow")
    return 1


if __name__ == "__main__":
    sys.exit(main())
