#!/bin/sh
# Crea R2Control.app a partir del binario compilado.
# macOS exige NSBluetoothAlwaysUsageDescription en el Info.plist: sin él cierra la app al usar Bluetooth.
# Uso: packaging/macos/bundle.sh <binario> <carpeta de salida> [versión]
set -eu
BIN="$1"; OUT="$2"; VERSION="${3:-0.0.0}"
APP="$OUT/R2Control.app"
rm -rf "$APP"
mkdir -p "$APP/Contents/MacOS"
cp "$BIN" "$APP/Contents/MacOS/r2control"
cat > "$APP/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
	<key>CFBundleName</key><string>R2 Control</string>
	<key>CFBundleDisplayName</key><string>R2 Control</string>
	<key>CFBundleIdentifier</key><string>com.sergio.r2control.desktop</string>
	<key>CFBundleExecutable</key><string>r2control</string>
	<key>CFBundlePackageType</key><string>APPL</string>
	<key>CFBundleVersion</key><string>$VERSION</string>
	<key>CFBundleShortVersionString</key><string>$VERSION</string>
	<key>LSMinimumSystemVersion</key><string>11.0</string>
	<key>NSHighResolutionCapable</key><true/>
	<key>NSBluetoothAlwaysUsageDescription</key><string>Para conectar con tu droide por Bluetooth.</string>
</dict>
</plist>
PLIST
# Firma ad hoc (sin cuenta de desarrollador): suficiente para ejecutarla en el propio equipo.
codesign --force --sign - "$APP"
echo "$APP"
