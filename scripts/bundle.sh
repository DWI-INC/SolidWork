#!/bin/sh
# Builds SolidWork.app (release) in the project root.
#   BIN_DIR=<dir>    take the built programs from there (default: build and use target/release)
#   ICON_TOOL=<file> make_icon program that runs on this machine (default: $BIN_DIR/make_icon)
#   APP=<name.app>   name of the bundle (default: SolidWork.app)
set -e
cd "$(dirname "$0")/.."
VERSION=$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)
if [ -z "$BIN_DIR" ]; then cargo build --release; BIN_DIR=target/release; fi
ICON_TOOL=${ICON_TOOL:-$BIN_DIR/make_icon}
APP=${APP:-SolidWork.app}
rm -rf "$APP"
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources"
cp "$BIN_DIR/solidwork" "$APP/Contents/MacOS/solidwork"
ICONSET=target/SolidWork.iconset
rm -rf "$ICONSET"; mkdir -p "$ICONSET"
for s in 16 32 64 128 256 512 1024; do "$ICON_TOOL" $s "$ICONSET/tmp_$s.png"; done
cp "$ICONSET/tmp_16.png" "$ICONSET/icon_16x16.png"
cp "$ICONSET/tmp_32.png" "$ICONSET/icon_16x16@2x.png"
cp "$ICONSET/tmp_32.png" "$ICONSET/icon_32x32.png"
cp "$ICONSET/tmp_64.png" "$ICONSET/icon_32x32@2x.png"
cp "$ICONSET/tmp_128.png" "$ICONSET/icon_128x128.png"
cp "$ICONSET/tmp_256.png" "$ICONSET/icon_128x128@2x.png"
cp "$ICONSET/tmp_256.png" "$ICONSET/icon_256x256.png"
cp "$ICONSET/tmp_512.png" "$ICONSET/icon_256x256@2x.png"
cp "$ICONSET/tmp_512.png" "$ICONSET/icon_512x512.png"
cp "$ICONSET/tmp_1024.png" "$ICONSET/icon_512x512@2x.png"
rm "$ICONSET"/tmp_*.png
iconutil -c icns "$ICONSET" -o "$APP/Contents/Resources/SolidWork.icns"
IDEASET=target/SolidWorkIdee.iconset
mkdir -p "$IDEASET"
for size in 16 32 128 256 512; do
  sips -s format png -z "$size" "$size" assets/solidwork-idee.svg --out "$IDEASET/icon_${size}x${size}.png" >/dev/null
  retina=$((size * 2))
  sips -s format png -z "$retina" "$retina" assets/solidwork-idee.svg --out "$IDEASET/icon_${size}x${size}@2x.png" >/dev/null
done
iconutil -c icns "$IDEASET" -o "$APP/Contents/Resources/SolidWorkIdee.icns"
cat > "$APP/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
  <key>CFBundleName</key><string>SolidWork</string>
  <key>CFBundleDisplayName</key><string>SolidWork</string>
  <key>CFBundleIdentifier</key><string>io.github.dwi-inc.solidwork</string>
  <key>CFBundleVersion</key><string>$VERSION</string>
  <key>CFBundleShortVersionString</key><string>$VERSION</string>
  <key>CFBundleExecutable</key><string>solidwork</string>
  <key>CFBundleIconFile</key><string>SolidWork</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>LSMinimumSystemVersion</key><string>11.0</string>
  <key>NSHighResolutionCapable</key><true/>
  <key>CFBundleDocumentTypes</key><array><dict>
    <key>CFBundleTypeName</key><string>SolidWork Idee</string>
    <key>CFBundleTypeIconFile</key><string>SolidWorkIdee.icns</string>
    <key>CFBundleTypeExtensions</key><array><string>prti</string></array>
    <key>CFBundleTypeRole</key><string>Editor</string>
  </dict><dict>
    <key>CFBundleTypeName</key><string>SolidWork Part</string>
    <key>CFBundleTypeExtensions</key><array><string>pfprt</string></array>
    <key>CFBundleTypeRole</key><string>Editor</string>
  </dict><dict>
    <key>CFBundleTypeName</key><string>SolidWork Assembly</string>
    <key>CFBundleTypeExtensions</key><array><string>pfasm</string></array>
    <key>CFBundleTypeRole</key><string>Editor</string>
  </dict><dict>
    <key>CFBundleTypeName</key><string>SolidWork Drawing</string>
    <key>CFBundleTypeExtensions</key><array><string>pfdrw</string></array>
    <key>CFBundleTypeRole</key><string>Editor</string>
  </dict><dict>
    <key>CFBundleTypeName</key><string>STEP Model</string>
    <key>CFBundleTypeExtensions</key><array><string>stp</string><string>step</string><string>p21</string></array>
    <key>CFBundleTypeRole</key><string>Viewer</string>
    <key>LSHandlerRank</key><string>Alternate</string>
  </dict><dict>
    <key>CFBundleTypeName</key><string>Mesh Model</string>
    <key>CFBundleTypeExtensions</key><array><string>stl</string><string>obj</string></array>
    <key>CFBundleTypeRole</key><string>Viewer</string>
    <key>LSHandlerRank</key><string>Alternate</string>
  </dict><dict>
    <key>CFBundleTypeName</key><string>Creo Model</string>
    <key>CFBundleTypeExtensions</key><array><string>prt</string><string>asm</string><string>drw</string></array>
    <key>CFBundleTypeRole</key><string>Viewer</string>
    <key>LSHandlerRank</key><string>Alternate</string>
  </dict></array>
</dict></plist>
PLIST
echo "Built $APP"
