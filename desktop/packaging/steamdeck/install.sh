#!/usr/bin/env bash
# Instala «Open control for Sphero Droids» en una Steam Deck (SteamOS, modo escritorio).
# No necesita sudo ni tocar el sistema de solo lectura: todo va a tu carpeta personal.
#
# Uso:
#   ./install.sh                         descarga e instala la última release
#   ./install.sh ruta/al/archivo.tar.gz  instala desde un .tar.gz ya descargado
#   ./install.sh --uninstall             lo quita todo
set -euo pipefail

REPO="Sergi-Morant/open-control-for-sphero-droids"
ASSET="open-control-for-sphero-droids-linux-x86_64.tar.gz"
BIN="open-control-for-sphero-droids"
APP_DIR="$HOME/.local/share/open-control-for-sphero-droids"
LAUNCHER="$HOME/.local/bin/$BIN"
DESKTOP_FILE="$HOME/.local/share/applications/$BIN.desktop"

if [[ "${1:-}" == "--uninstall" ]]; then
  rm -rf "$APP_DIR" "$LAUNCHER" "$DESKTOP_FILE"
  echo "Desinstalado."
  exit 0
fi

tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT

if [[ -n "${1:-}" ]]; then
  [[ -f "$1" ]] || { echo "No existe el archivo: $1" >&2; exit 1; }
  cp "$1" "$tmp/$ASSET"
else
  echo "Descargando la última release…"
  curl -fSL -o "$tmp/$ASSET" "https://github.com/$REPO/releases/latest/download/$ASSET"
fi

echo "Instalando en $APP_DIR…"
rm -rf "$APP_DIR"
mkdir -p "$APP_DIR" "$(dirname "$LAUNCHER")" "$(dirname "$DESKTOP_FILE")"
tar -xzf "$tmp/$ASSET" -C "$APP_DIR"
chmod +x "$APP_DIR/$BIN"
ln -sf "$APP_DIR/$BIN" "$LAUNCHER"

cat > "$DESKTOP_FILE" <<DESK
[Desktop Entry]
Type=Application
Name=Open control for Sphero Droids
Comment=Control no oficial de droides Sphero por Bluetooth
Exec=$APP_DIR/$BIN
Icon=input-gaming
Terminal=false
Categories=Game;Utility;
DESK
update-desktop-database "$HOME/.local/share/applications" 2>/dev/null || true

if command -v systemctl >/dev/null && ! systemctl is-active --quiet bluetooth; then
  echo "AVISO: el servicio bluetooth no está activo. Actívalo en Ajustes > Bluetooth."
fi

cat <<MSG

Listo. Ábrela desde el menú (Juegos/Utilidades) o con: $BIN

Para usar los controles de la Deck como mando:
  Steam (modo escritorio) > Juegos > Añadir un juego que no es de Steam >
  elige «Open control for Sphero Droids». Así aparece en el modo juego y
  Steam Input le entrega los botones y sticks como un mando.
MSG
