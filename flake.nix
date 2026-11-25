{
  description = "vsnote —— 轻量跨平台 Markdown 编辑器（Tauri v2）";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-26.05";
  };

  outputs =
    { self, nixpkgs }:
    let
      systems = [
        "x86_64-linux"
        "aarch64-linux"
      ];
      forAllSystems = nixpkgs.lib.genAttrs systems;

      version = (builtins.fromJSON (builtins.readFile ./package.json)).version;
    in
    {
      packages = forAllSystems (
        system:
        let
          pkgs = import nixpkgs { inherit system; };
          inherit (pkgs) lib;

          # flake 源已按 .gitignore 过滤，这里再兜一层，避免 node_modules / 构建产物进 store。
          src = lib.cleanSourceWith {
            src = ./.;
            filter =
              path: type:
              let
                base = baseNameOf path;
              in
              !(builtins.elem base [
                "node_modules"
                "target"
                "dist"
                "out"
                "release"
                ".git"
              ]);
          };

          # Tauri v2 在 Linux 上链接 webkit2gtk-4.1 / gtk3 / libsoup3 等系统库。
          tauriBuildInputs = with pkgs; [
            webkitgtk_4_1
            gtk3
            libsoup_3
            glib
            librsvg
            openssl
            glib-networking
            gsettings-desktop-schemas
          ];

          vsnote = pkgs.rustPlatform.buildRustPackage {
            pname = "vsnote";
            inherit version src;

            cargoRoot = "src-tauri";
            buildAndTestSubdir = "src-tauri";
            cargoLock = {
              lockFile = ./src-tauri/Cargo.lock;
              # Cargo.toml 用 [patch.crates-io] 把 brotli 指向 git（crates.io 版本依赖冲突，
              # 详见 Cargo.toml 注释），git 依赖在 Nix 里必须显式给出 hash。
              outputHashes."brotli-8.0.3" = "sha256-5HRCwBCs9xcOdWd15SZ0ryEr4/Fk4IVxYdIGbMyRu98=";
            };

            # 前端依赖按 package-lock.json 离线固定，无需手工维护 hash。
            npmDeps = pkgs.importNpmLock { npmRoot = src; };

            nativeBuildInputs = with pkgs; [
              nodejs_22
              pkg-config
              wrapGAppsHook4
              gobject-introspection
              importNpmLock.npmConfigHook
            ];

            buildInputs = tauriBuildInputs;

            # tauri.conf.json 没有 beforeBuildCommand：内嵌进二进制的前端资源必须先生成。
            preBuild = ''
              node scripts/build-renderer.mjs
              node scripts/ensure-vendor.mjs
            '';

            doCheck = false;

            postInstall = ''
              install -Dm644 $src/assets/icon.png $out/share/icons/hicolor/512x512/apps/vsnote.png
              install -Dm644 $src/src-tauri/icons/128x128.png $out/share/icons/hicolor/128x128/apps/vsnote.png
              install -Dm644 $src/src-tauri/icons/32x32.png $out/share/icons/hicolor/32x32/apps/vsnote.png

              install -d $out/share/applications
              cat > $out/share/applications/vsnote.desktop <<EOF
[Desktop Entry]
Type=Application
Version=1.0
Name=vsnote
Comment=A lightweight cross-platform Markdown editor
Exec=$out/bin/vsnote %F
Icon=vsnote
Terminal=false
Categories=Office;TextEditor;Utility;
MimeType=text/markdown;text/plain;
StartupWMClass=vsnote
EOF
            '';

            meta = {
              description = "A lightweight cross-platform Markdown editor";
              homepage = "https://github.com/TizuMark/vsnote";
              license = lib.licenses.gpl3Only;
              mainProgram = "vsnote";
              platforms = lib.platforms.linux;
            };
          };
        in
        {
          default = vsnote;
          vsnote = vsnote;
        }
      );

      devShells = forAllSystems (
        system:
        let
          pkgs = import nixpkgs { inherit system; };
        in
        {
          default = pkgs.mkShell {
            packages = with pkgs; [
              nodejs_22
              rustc
              cargo
              rustfmt
              clippy
              pkg-config
              # Tauri v2 Linux 系统依赖（与 packages 保持一致）
              webkitgtk_4_1
              gtk3
              libsoup_3
              glib
              librsvg
              openssl
              glib-networking
              gsettings-desktop-schemas
            ];
          };
        }
      );

      apps = forAllSystems (system: {
        default = {
          type = "app";
          program = "${self.packages.${system}.vsnote}/bin/vsnote";
        };
      });
    };
}
