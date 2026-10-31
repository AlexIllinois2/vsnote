#!/data/data/com.termux/files/usr/bin/bash
# Stage a Tauri x86_64 binary and build an RPM with rpmbuild on aarch64 Termux.
# Overridable env: PRODUCT, VERSION, PROJECT_DIR, BIN_PATH, RESOURCES,
#                  SUMMARY, COMMENT, CATEGORIES, ICON_DIR, RPM_TOP, STAGE.
set -euo pipefail

PROJECT_DIR="${PROJECT_DIR:-$HOME/vsnote}"
PRODUCT="${PRODUCT:-}"
VERSION="${VERSION:-}"

if [ -z "$PRODUCT" ] || [ -z "$VERSION" ]; then
  read -r CONF_PRODUCT CONF_VERSION <<<"$(python3 - "$PROJECT_DIR/src-tauri/tauri.conf.json" <<'PY'
import json, sys
cfg = json.load(open(sys.argv[1]))
print(cfg.get("productName", ""), cfg.get("version", ""))
PY
)"
  PRODUCT="${PRODUCT:-$CONF_PRODUCT}"
  VERSION="${VERSION:-$CONF_VERSION}"
fi

[ -n "$PRODUCT" ] && [ -n "$VERSION" ] || { echo "PRODUCT/VERSION unresolved"; exit 1; }

ICON_DIR="${ICON_DIR:-$PROJECT_DIR/src-tauri/icons}"
BIN_PATH="${BIN_PATH:-$PROJECT_DIR/src-tauri/target/x86_64-unknown-linux-gnu/release/$PRODUCT}"
STAGE="${STAGE:-$HOME/xbuild/rpmstage}"
RPM_TOP="${RPM_TOP:-$HOME/xbuild/rpm}"
RPMRC="${RPMRC:-$HOME/xbuild/rpmrc}"
SUMMARY="${SUMMARY:-$PRODUCT}"
COMMENT="${COMMENT:-$SUMMARY}"
CATEGORIES="${CATEGORIES:-Office;TextEditor;}"
RESOURCES="${RESOURCES-demo.md assets/icon.png}"

[ -x "$BIN_PATH" ] || { echo "missing binary: $BIN_PATH"; exit 1; }

rm -rf "$STAGE" "$RPM_TOP"
mkdir -p "$STAGE/usr/bin" "$STAGE/usr/lib/$PRODUCT" \
  "$STAGE/usr/share/applications" "$STAGE/usr/share/mime/packages" \
  "$STAGE/usr/share/licenses/$PRODUCT" \
  "$STAGE/usr/share/icons/hicolor/32x32/apps" \
  "$STAGE/usr/share/icons/hicolor/128x128/apps" \
  "$STAGE/usr/share/icons/hicolor/256x256/apps"

install -m0755 "$BIN_PATH" "$STAGE/usr/bin/$PRODUCT"

# Bundled resources keep the paths used in tauri.conf.json bundle.resources.
for res in $RESOURCES; do
  src="$PROJECT_DIR/$res"
  [ -f "$src" ] || continue
  dest_dir="$STAGE/usr/lib/$PRODUCT/$(dirname "$res")"
  mkdir -p "$dest_dir"
  install -m0644 "$src" "$dest_dir/"
done

for pair in "32x32:32x32" "128x128:128x128" "128x128@2x:256x256"; do
  from="${pair%%:*}"
  to="${pair##*:}"
  if [ -f "$ICON_DIR/$from.png" ]; then
    install -m0644 "$ICON_DIR/$from.png" "$STAGE/usr/share/icons/hicolor/$to/apps/$PRODUCT.png"
  fi
done
if [ -f "$PROJECT_DIR/LICENSE" ]; then
  install -m0644 "$PROJECT_DIR/LICENSE" "$STAGE/usr/share/licenses/$PRODUCT/LICENSE"
fi

cat > "$STAGE/usr/share/applications/$PRODUCT.desktop" <<DESKTOP
[Desktop Entry]
Type=Application
Name=$PRODUCT
Comment=$COMMENT
Exec=$PRODUCT %F
Icon=$PRODUCT
Terminal=false
Categories=$CATEGORIES
MimeType=text/markdown;text/plain;
StartupWMClass=$PRODUCT
DESKTOP

cat > "$STAGE/usr/share/mime/packages/$PRODUCT.xml" <<MIME
<?xml version="1.0" encoding="UTF-8"?>
<mime-info xmlns="http://www.freedesktop.org/standards/shared-mime-info">
  <mime-type type="text/markdown">
    <comment>Markdown Document</comment>
    <glob pattern="*.md"/>
    <glob pattern="*.markdown"/>
  </mime-type>
  <mime-type type="text/plain">
    <comment>Plain Text Document</comment>
    <glob pattern="*.txt"/>
  </mime-type>
</mime-info>
MIME

chmod 0644 "$STAGE/usr/share/applications/$PRODUCT.desktop" "$STAGE/usr/share/mime/packages/$PRODUCT.xml"
chmod -R a+rX "$STAGE/usr"

printf 'buildarch_compat: aarch64: x86_64\n' > "$RPMRC"

SPEC="$HOME/xbuild/$PRODUCT.spec"
{
  echo '%define __os_install_post %nil'
  echo '%global debug_package %{nil}'
  echo 'AutoReqProv: no'
  echo "Name: $PRODUCT"
  echo "Version: $VERSION"
  echo 'Release: 1'
  echo "Summary: $SUMMARY"
  echo 'License: GPL-3.0'
  echo 'BuildArch: x86_64'
  echo 'Requires: webkit2gtk4.1'
  echo 'Requires: gtk3'
  echo ''
  echo '%description'
  echo "$COMMENT"
  echo ''
  echo '%install'
  echo 'mkdir -p %{buildroot}'
  echo "cp -a \"$STAGE\"/. %{buildroot}/"
  echo ''
  echo '%files'
  echo "/usr/bin/$PRODUCT"
  if [ -n "$RESOURCES" ]; then
    echo "/usr/lib/$PRODUCT/*"
  fi
  echo "/usr/share/applications/$PRODUCT.desktop"
  echo "/usr/share/icons/hicolor/*/apps/$PRODUCT.png"
  echo "/usr/share/mime/packages/$PRODUCT.xml"
  if [ -f "$PROJECT_DIR/LICENSE" ]; then
    echo "/usr/share/licenses/$PRODUCT/LICENSE"
  fi
} > "$SPEC"

rpmbuild -bb --rcfile "$RPMRC" --target x86_64-linux --define "_topdir $RPM_TOP" "$SPEC"
ls -l "$RPM_TOP"/RPMS/x86_64/*.rpm
rpm -qp --queryformat "arch=%{ARCH} name=%{NAME} ver=%{VERSION}\n" "$RPM_TOP"/RPMS/x86_64/*.rpm
