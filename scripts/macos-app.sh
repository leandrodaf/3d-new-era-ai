#!/usr/bin/env bash
# Wraps a built `newera` binary as "3D New Era AI.app", signed ad-hoc (Apple
# Silicon only runs signed code; no Developer account needed).
#
#   scripts/macos-app.sh <newera binary> <output folder> [commit]
#
# Used by the release workflow and by scripts/install-macos.sh.
set -euo pipefail

BINARY="$1"
OUT_DIR="$2"
COMMIT="${3:-}"
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
VERSION=$(sed -n 's/^version = "\(.*\)"/\1/p' "$ROOT/Cargo.toml" | head -1)
APP="$OUT_DIR/3D New Era AI.app"

rm -rf "$APP"
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources"
cp "$BINARY" "$APP/Contents/MacOS/newera"
chmod +x "$APP/Contents/MacOS/newera"
cp "$ROOT/assets/icon.icns" "$APP/Contents/Resources/AppIcon.icns"
cp "$ROOT/assets/document.icns" "$APP/Contents/Resources/Document.icns"
[ -n "$COMMIT" ] && echo "$COMMIT" > "$APP/Contents/Resources/commit"
cat > "$APP/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleName</key><string>3D New Era AI</string>
    <key>CFBundleDisplayName</key><string>3D New Era AI</string>
    <key>CFBundleIdentifier</key><string>io.github.leandrodaf.newera</string>
    <key>CFBundleExecutable</key><string>newera</string>
    <key>CFBundleIconFile</key><string>AppIcon</string>
    <key>CFBundlePackageType</key><string>APPL</string>
    <key>CFBundleShortVersionString</key><string>${VERSION}</string>
    <key>CFBundleVersion</key><string>${VERSION}</string>
    <key>LSMinimumSystemVersion</key><string>11.0</string>
    <key>NSHighResolutionCapable</key><true/>
    <key>LSApplicationCategoryType</key><string>public.app-category.graphics-design</string>
    <key>NSHumanReadableCopyright</key><string>Leandro Ferreira — MIT or Apache-2.0</string>
    <key>CFBundleDocumentTypes</key>
    <array>
        <dict>
            <key>CFBundleTypeName</key><string>3D New Era AI project</string>
            <key>CFBundleTypeExtensions</key><array><string>newera</string></array>
            <key>CFBundleTypeIconFile</key><string>Document</string>
            <key>LSItemContentTypes</key><array><string>io.github.leandrodaf.newera.project</string></array>
            <key>CFBundleTypeRole</key><string>Editor</string>
            <key>LSHandlerRank</key><string>Owner</string>
        </dict>
        <dict>
            <key>CFBundleTypeName</key><string>Sweet Home 3D project</string>
            <key>CFBundleTypeExtensions</key><array><string>sh3d</string></array>
            <key>CFBundleTypeIconFile</key><string>Document</string>
            <key>CFBundleTypeRole</key><string>Editor</string>
            <key>LSHandlerRank</key><string>Alternate</string>
        </dict>
    </array>
    <!-- The .newera type itself, so Finder shows the icon and the kind even
         where the app is not the default opener. -->
    <key>UTExportedTypeDeclarations</key>
    <array>
        <dict>
            <key>UTTypeIdentifier</key><string>io.github.leandrodaf.newera.project</string>
            <key>UTTypeDescription</key><string>3D New Era AI project</string>
            <key>UTTypeIconFile</key><string>Document</string>
            <key>UTTypeConformsTo</key><array><string>public.data</string><string>public.content</string></array>
            <key>UTTypeTagSpecification</key>
            <dict>
                <key>public.filename-extension</key><array><string>newera</string></array>
                <key>public.mime-type</key><array><string>application/x-newera</string></array>
            </dict>
        </dict>
    </array>
</dict>
</plist>
PLIST
codesign --force --deep --sign - "$APP"
echo "$APP"
