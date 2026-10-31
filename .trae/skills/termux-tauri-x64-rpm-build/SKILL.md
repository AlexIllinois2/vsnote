---
name: termux-tauri-x64-rpm-build
description: Cross-compile a Tauri app on Android Termux (aarch64) into an x86_64 GNU Linux binary and RPM. Use when asked to build an x64 RPM or Linux x86_64 package on a phone via SSH. Do not use for native aarch64 Android builds.
---

# Termux Tauri x86_64 RPM Build

Cross-compile a Tauri v2 app on a rooted/plain Termux phone (aarch64) against a Debian amd64 sysroot, then package the result as an x86_64 RPM with `rpmbuild`. All commands below run on the phone over SSH unless marked otherwise.

## Prerequisites and layout

Phone (`PREFIX=/data/data/com.termux/files/usr`, `HOME=/data/data/com.termux/files/home`):

```sh
pkg install -y clang lld python rpm
```

Rust comes from the Termux `rust` package (includes `rustc`, `cargo`, and `rust-src` at `$PREFIX/lib/rustlib/src/rust`). No gcc package exists in Termux. Ensure ~2 GB free on `/data`.

Directories used:

| Path | Purpose |
|------|---------|
| `~/vsnote` | synced project source |
| `~/xsys` | Debian amd64 sysroot (~1 GB) |
| `~/xbuild` | scripts, deb cache, rpmbuild topdir |
| `~/xbuild/x86gcc` | clang wrapper for target `x86_64-unknown-linux-gnu` |

## Step 1 — Sync source (host → phone)

Exclude heavy/arch-specific dirs; `target` and `node_modules` must not come from the host:

```sh
# on host
tar czf /tmp/src.tgz --exclude=node_modules --exclude=src-tauri/target --exclude=.git --exclude=.workbuddy .
scp -P 8022 /tmp/src.tgz 192.168.1.115:~/src.tgz
# on phone
mkdir -p ~/vsnote && tar xzf ~/src.tgz -C ~/vsnote
```

If the repo's frontend must be bundled (e.g. esbuild step), build it on the host with a portable Node (do not install Node on the phone), then sync only the generated output:

```sh
curl -sL https://npmmirror.com/mirrors/node/v22.11.0/node-v22.11.0-linux-x64.tar.xz -o /tmp/node.tar.xz
tar xJf /tmp/node.tar.xz -C /tmp/n --strip-components=1
PATH=/tmp/n/bin:$PATH node scripts/build-renderer.mjs
```

## Step 2 — Build the amd64 sysroot

Seeds: `libc6-dev`, `libgcc-s1`, `libgcc-14-dev`, `libgtk-3-dev`, `libwebkit2gtk-4.1-dev`, `libsoup-3.0-dev`, `librsvg2-dev`, `libssl-dev`. `libssl-dev` is mandatory because `reqwest` default-tls pulls `openssl-sys` even when rustls too is enabled.

```sh
mkdir -p ~/xbuild ~/xsys
scp -P 8022 debsys.py 192.168.1.115:~/xbuild/debsys.py   # from this skill's scripts/
ssh -p 8022 192.168.1.115 'python3 ~/xbuild/debsys.py --list'   # sanity check closure
ssh -p 8022 192.168.1.115 'nohup setsid python3 ~/xbuild/debsys.py > ~/xbuild/sysroot.log 2>&1 < /dev/null &'
tail -f ~/xbuild/sysroot.log
```

Expect ~460–470 packages, ~240 MiB of debs, ~1 GB extracted. Export paths later via `.cargo/config.toml`. If `deb.debian.org` stalls, pass `--mirror https://mirrors.tuna.tsinghua.edu.cn/debian`.

## Step 3 — Toolchain wrapper and cargo config

Create `~/xbuild/x86gcc` (values from this skill's `scripts/x86gcc`) and `~/vsnote/src-tauri/.cargo/config.toml` (template in `scripts/cargo-config.toml`). The wrapper is:

```sh
exec clang --target=x86_64-unknown-linux-gnu --sysroot=$SYS -fuse-ld=lld -Qunused-arguments "$@"
```

The config pins `linker`, `PKG_CONFIG_SYSROOT_DIR`, `PKG_CONFIG_LIBDIR`, `PKG_CONFIG_ALLOW_CROSS`, `CC_x86_64_unknown_linux_gnu`, `AR_x86_64_unknown_linux_gnu` (`llvm-ar`). Add `[build] target = "x86_64-unknown-linux-gnu"` so plain `cargo build` works. Never commit this file on the host.

Validate before the long build — both must produce `Machine: Advanced Micro Devices X86-64`:

```sh
printf '#include <stdio.h>\nint main(){puts("hi");return 0;}\n' > ~/xbuild/hello.c
~/xbuild/x86gcc ~/xbuild/hello.c -o ~/xbuild/hello
~/xbuild/x86gcc ~/xbuild/gtkhello.c -o ~/xbuild/gtkhello $(pkg-config --cflags --libs gtk+-3.0 webkit2gtk-4.1)
```

## Step 4 — Build the binary

The Termux rust 1.99.1 source tarball has no downloadable `rust-std` for `x86_64-unknown-linux-gnu`, so compile std from `rust-src`:

```sh
cd ~/vsnote/src-tauri
RUSTC=$PREFIX/bin/rustc RUSTC_BOOTSTRAP=1 \
  nohup setsid $PREFIX/bin/cargo build --release -Z build-std=std,panic_unwind \
  > ~/xbuild/cargo.log 2>&1 < /dev/null &
```

Use absolute `$PREFIX/bin/rustc`/`cargo`: a rustup shim in `~/.cargo/bin` usually shadows them and fails with "no default toolchain". Keep `jobs = 3` to fit ~3.5 GB RAM. Expect 30–60 min; first `Updating git repository` for the lockfile's git patch can take minutes.

Verify:

```sh
readelf -h target/x86_64-unknown-linux-gnu/release/vsnote | grep Machine   # X86-64
readelf -d target/x86_64-unknown-linux-gnu/release/vsnote | grep NEEDED
```

## Step 5 — Package the RPM

`rpmbuild` refuses a foreign arch; tell rpm the host is compatible with x86_64 via a custom rpmrc (this only relaxes the check, the `%{ARCH}` still comes from `--target`):

```sh
printf 'buildarch_compat: aarch64: x86_64\n' > ~/xbuild/rpmrc
```

Run `scripts/mk-rpm.sh` (defaults to this repo, overridable by `PRODUCT`, `VERSION`, `PROJECT_DIR`, `BIN_PATH`, `RESOURCES`). Then:

```sh
rpmbuild -bb --rcfile ~/xbuild/rpmrc --target x86_64-linux --define "_topdir $HOME/xbuild/rpm" ~/xbuild/vsnote.spec
rpm -qlvp ~/xbuild/rpm/RPMS/x86_64/*.rpm
rpm -qp --queryformat '%{ARCH}\n' ~/xbuild/rpm/RPMS/x86_64/*.rpm   # x86_64
```

Use literal `/usr/...` paths in the spec and `%files`. Termux rpm macros define `_prefix` as the Termux prefix and `_libdir` as `lib64`; macros such as `%{_bindir}` produce wrong paths. Disable rpm's post-processing and dependency scanning with `%define __os_install_post %nil` and `AutoReqProv: no`.

Layout Tauri expects at runtime:

| Path | Content |
|------|---------|
| `/usr/bin/<product>` | binary, 0755 |
| `/usr/lib/<product>/<resource>` | resources, preserving config-relative paths |
| `/usr/share/applications/<product>.desktop` | launcher |
| `/usr/share/icons/hicolor/<size>/apps/<product>.png` | 32/128/256 |
| `/usr/share/mime/packages/<product>.xml` | file associations |

`app.path().resource_dir()` on Linux resolves to `/usr/lib/<product>`. Ensure generated `.desktop`/`.xml` files are 0644, not 0600.

## Step 6 — Pull to the host

```sh
# on host
scp -P 8022 192.168.1.115:'~/xbuild/rpm/RPMS/x86_64/*.rpm' ~/Downloads/
sha256sum ~/Downloads/*.rpm   # compare with the phone's sha256sum
```

## Pitfalls

| Symptom | Fix |
|---------|-----|
| `no default toolchain` from rustup | Call `$PREFIX/bin/cargo` and set `RUSTC=$PREFIX/bin/rustc` |
| `failed to run custom build command for openssl-sys` | Add `libssl-dev` to seeds and re-extract |
| `No compatible architectures found for build` | `--rcfile` with `buildarch_compat: aarch64: x86_64` |
| `AbsoluteLinkError` when extracting debs | `tarfile.extractall(..., filter="fully_trusted")` |
| `seeking backwards is not allowed` | Decompress the payload to bytes and open tar with mode `r:` |
| `error: no matching package found ... x86_64-unknown-linux-gnu` | Use `-Z build-std=std,panic_unwind` with `RUSTC_BOOTSTRAP=1` |
| Git dependency fetch crawls | Let it run, or pre-seed `~/.cargo/git/db` from a host bare clone |
| pkg-config returns Termux paths | Set `PKG_CONFIG_LIBDIR` only to sysroot dirs, keep `PKG_CONFIG_ALLOW_CROSS=1` |

Scope limits: output targets glibc distros (Fedora/RHEL/openSUSE rpm). The RPM must declare `webkit2gtk4.1` and `gtk3` runtime requires; no bundling of GTK/WebKit. This flow does not produce AppImage or musl builds.
