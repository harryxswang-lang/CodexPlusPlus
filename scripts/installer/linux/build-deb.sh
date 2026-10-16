#!/usr/bin/env bash
set -euo pipefail
umask 022

# Ubuntu 本机构建：不安装依赖、不提权、不删除历史产物。
# 用法：bash scripts/installer/linux/build-deb.sh [--check|--package-only] [VERSION]
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_DIR="$(cd "$SCRIPT_DIR/../../.." && pwd)"
PROJECT_DIR="$REPO_DIR/apps/codex-plus-manager"
MODE=build
case "${1:-}" in
    --check) MODE=check; shift ;;
    --package-only) MODE=package; shift ;;
    --help|-h)
        echo "用法: bash scripts/installer/linux/build-deb.sh [--check|--package-only] [VERSION]"
        echo "先手动准备依赖；--check 只检查依赖，--package-only 打包已有 release 二进制。"
        exit 0 ;;
esac
[ "$#" -le 1 ] || { echo "error: 参数过多" >&2; exit 1; }
cd "$REPO_DIR"

require_command() {
    command -v "$1" >/dev/null || { echo "error: 缺少命令 $1，请手动安装。" >&2; exit 1; }
}
for command in node dpkg dpkg-deb dpkg-shlibdeps; do require_command "$command"; done
[ "$(uname -s)" = Linux ] || { echo "error: 请在目标 Linux 系统上构建。" >&2; exit 1; }
VERSION="${1:-$(node -e 'process.stdout.write(JSON.parse(require("fs").readFileSync(process.argv[1], "utf8")).version)' "$PROJECT_DIR/package.json")}"
dpkg --validate-version "$VERSION"
ARCH="$(dpkg --print-architecture)"
PACKAGE_NAME=codex-plus-plus
TARGET_DIR="${CARGO_TARGET_DIR:-$REPO_DIR/target}"
[[ "$TARGET_DIR" = /* ]] || TARGET_DIR="$REPO_DIR/$TARGET_DIR"
BINARY="$TARGET_DIR/release/codex-plus-plus"
OUTPUT_DIR="$REPO_DIR/dist/linux"

if [ "$MODE" != package ]; then
    for command in npm cargo pkg-config cc; do require_command "$command"; done
    if ! pkg-config --exists gtk+-3.0 webkit2gtk-4.1 ayatana-appindicator3-0.1 openssl; then
        echo "error: 缺少系统开发包。请在终端手动执行：" >&2
        echo "sudo apt-get install --no-install-recommends build-essential pkg-config libssl-dev libgtk-3-dev libayatana-appindicator3-dev libwebkit2gtk-4.1-dev" >&2
        exit 1
    fi
    [ -x "$PROJECT_DIR/node_modules/.bin/vite" ] || {
        echo "error: 缺少前端依赖。请在 apps/codex-plus-manager 中执行 npm ci --ignore-scripts。" >&2
        exit 1
    }
    echo "系统与前端构建依赖检查通过；Rust 依赖请用 cargo fetch --locked 准备。"
    [ "$MODE" != check ] || exit 0
    (cd "$PROJECT_DIR" && npm run vite:build)
    cargo build --locked --release -p codex-plus-manager --bin codex-plus-plus
fi

[ -x "$BINARY" ] || { echo "error: 缺少 release 二进制：$BINARY" >&2; exit 1; }
mkdir -p "$OUTPUT_DIR"
# 每次使用独立目录，失败时保留便于诊断，不清理或覆盖已有安装包。
BUILD_DIR="$(mktemp -d "$OUTPUT_DIR/stage-XXXXXXXX")"
DEB_DIR="$BUILD_DIR/package"
STAGE="$DEB_DIR/usr"
DEB_PATH="$OUTPUT_DIR/${PACKAGE_NAME}_${VERSION}_${ARCH}.deb"
[ ! -e "$DEB_PATH" ] || { echo "error: 安装包已存在，请指定新版本：$DEB_PATH" >&2; exit 1; }
mkdir -p "$DEB_DIR/DEBIAN" "$STAGE/bin" "$STAGE/share/applications" \
    "$STAGE/share/icons/hicolor/128x128/apps" "$STAGE/share/doc/$PACKAGE_NAME" "$BUILD_DIR/debian"
install -m 755 "$BINARY" "$STAGE/bin/$PACKAGE_NAME"
install -m 644 "$PROJECT_DIR/src-tauri/icons/icon.png" "$STAGE/share/icons/hicolor/128x128/apps/$PACKAGE_NAME.png"
for document in README.md LICENSE; do
    install -m 644 "$REPO_DIR/$document" "$STAGE/share/doc/$PACKAGE_NAME/"
done
install -m 644 "$SCRIPT_DIR/README.md" "$STAGE/share/doc/$PACKAGE_NAME/Ubuntu.md"
install -m 644 "$SCRIPT_DIR/codex-plus-plus.desktop" "$STAGE/share/applications/$PACKAGE_NAME.desktop"

# 根据实际二进制计算依赖，避免写死 Ubuntu 版本的 t64 包名和 libc 版本。
cat > "$BUILD_DIR/debian/control" <<CONTROL
Source: $PACKAGE_NAME
Section: utils
Priority: optional
Maintainer: CodexPlusPlus contributors

Package: $PACKAGE_NAME
Architecture: any
Description: Codex++ desktop manager
CONTROL
DEPENDENCIES="$(cd "$BUILD_DIR" && dpkg-shlibdeps -O -e"$STAGE/bin/$PACKAGE_NAME")"
DEPENDENCIES="${DEPENDENCIES#shlibs:Depends=}"
[ -n "$DEPENDENCIES" ] || { echo "error: 无法计算动态库依赖。" >&2; exit 1; }
INSTALLED_SIZE="$(du -sk "$STAGE" | awk '{print $1}')"
cat > "$DEB_DIR/DEBIAN/control" <<CONTROL
Package: $PACKAGE_NAME
Version: $VERSION
Section: utils
Priority: optional
Architecture: $ARCH
Maintainer: CodexPlusPlus contributors
Installed-Size: $INSTALLED_SIZE
Depends: $DEPENDENCIES, libayatana-appindicator3-1, xdg-utils
Description: Codex++ provider and model manager for Codex desktop
 Configure third-party providers and models, manage sessions and launch
 the separately installed Codex / ChatGPT desktop app with UI enhancements.
CONTROL
cat > "$DEB_DIR/DEBIAN/postinst" <<'POSTINST'
#!/bin/sh
set -e
if command -v update-desktop-database >/dev/null 2>&1; then
    update-desktop-database -q /usr/share/applications || true
fi
if command -v gtk-update-icon-cache >/dev/null 2>&1; then
    gtk-update-icon-cache -q -t -f /usr/share/icons/hicolor || true
fi
POSTINST
chmod 755 "$DEB_DIR/DEBIAN/postinst"
dpkg-deb --root-owner-group --build "$DEB_DIR" "$DEB_PATH"
dpkg-deb --info "$DEB_PATH"
echo "安装包：$DEB_PATH"
echo "安装（请在终端手动执行）：sudo apt install \"$DEB_PATH\""
echo "本地试运行：$STAGE/bin/$PACKAGE_NAME"
