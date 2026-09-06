#!/bin/bash
set -e

APP_NAME="Esotereel"
APP_DIR="build/macOS/${APP_NAME}.app"
DIST_DIR="dist"

# 0. クリーンアップ & 出力ディレクトリ作成
rm -rf "build/macOS"
mkdir -p "$DIST_DIR"

# 1. AppBundleディレクトリ構造を作成
mkdir -p "$APP_DIR/Contents/MacOS"
mkdir -p "$APP_DIR/Contents/Resources"

# 2. 実行ファイル・ライブラリのコピー
cp build/cmake/esotereel_gui "$APP_DIR/Contents/MacOS/esotereel_gui"
chmod +x "$APP_DIR/Contents/MacOS/esotereel_gui"

# 3. Info.plistを作成
cat > "$APP_DIR/Contents/Info.plist" << 'PLIST'
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.plist">
<plist version="1.0">
<dict>
    <key>CFBundleDevelopmentRegion</key>
    <string>en</string>
    <key>CFBundleExecutable</key>
    <string>esotereel_gui</string>
    <key>CFBundleIdentifier</key>
    <string>com.esotereel.app</string>
    <key>CFBundleInfoDictionaryVersion</key>
    <string>6.0</string>
    <key>CFBundleName</key>
    <string>Esotereel</string>
    <key>CFBundlePackageType</key>
    <string>APPL</string>
    <key>CFBundleShortVersionString</key>
    <string>1.0.0</string>
    <key>CFBundleVersion</key>
    <string>1</string>
    <key>LSMinimumSystemVersion</key>
    <string>10.12</string>
</dict>
</plist>
PLIST

# 4. DMGを作成 (対象を .app に限定し、dist/ 以下に出力)
hdiutil create -volname "$APP_NAME" -srcfolder "$APP_DIR" -ov -format UDZO "$DIST_DIR/esotereel.dmg"

# 5. 作成確認
if [ -f "$DIST_DIR/esotereel.dmg" ]; then
  echo "DMG created successfully: $DIST_DIR/esotereel.dmg"
  ls -lh "$DIST_DIR/esotereel.dmg"
else
  echo "Failed to create DMG"
  exit 1
fi