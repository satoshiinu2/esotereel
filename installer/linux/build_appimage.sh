#!/bin/bash
set -e

APP_DIR="build/AppDir"
DIST_DIR="dist"

# 0. 過去の作業用フォルダと成果物フォルダをクリーンアップ＆作成
rm -rf "$APP_DIR"
mkdir -p "$DIST_DIR"

# 1. 作業フォルダの作成
mkdir -p "$APP_DIR/usr/bin"
mkdir -p "$APP_DIR/usr/lib"
mkdir -p "$APP_DIR/usr/share/applications"
mkdir -p "$APP_DIR/usr/share/icons/hicolor/256x256/apps"

# 2. 実行ファイル・共有ライブラリのコピー
cp build/cmake/esotereel_gui "$APP_DIR/usr/bin/esotereel"
chmod +x "$APP_DIR/usr/bin/esotereel"

# 3. Desktop ファイルの作成
cat > "$APP_DIR/usr/share/applications/esotereel.desktop" << 'DESKTOP'
[Desktop Entry]
Version=1.0
Type=Application
Name=Esotereel
Exec=esotereel
Icon=esotereel
Categories=Utility;
DESKTOP

# 4. AppRun スクリプトの作成
cat > "$APP_DIR/AppRun" << 'APPRUN'
#!/bin/bash
SELF=$(readlink -f "$0")
HERE="${SELF%/*}"
export LD_LIBRARY_PATH="${HERE}/usr/lib:${LD_LIBRARY_PATH}"
exec "${HERE}/usr/bin/esotereel" "$@"
APPRUN
chmod +x "$APP_DIR/AppRun"

# 5. アイコンの配置（アイコン画像がある場合はそれをルート直下に配置）
cp "$APP_DIR/usr/share/applications/esotereel.desktop" "$APP_DIR/esotereel.desktop"

if [ -f installer/icon.png ]; then
  cp installer/icon.png "$APP_DIR/esotereel.png"
  cp installer/icon.png "$APP_DIR/usr/share/icons/hicolor/256x256/apps/esotereel.png"
fi

# 6. AppImage のビルドを実行
appimagetool -n "$APP_DIR" "$DIST_DIR/esotereel.AppImage"