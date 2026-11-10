#!/usr/bin/env bash
# Instala «Open control for Sphero Droids» en una Steam Deck (SteamOS, modo escritorio).
# No necesita sudo ni tocar el sistema de solo lectura: todo va a tu carpeta personal.
# Al terminar la añade a la biblioteca de Steam para abrirla desde el modo juego.
#
# Uso:
#   ./install.sh                         descarga e instala la última release
#   ./install.sh ruta/al/archivo.tar.gz  instala desde un .tar.gz ya descargado
#   ./install.sh --uninstall             lo quita todo
#   NO_STEAM=1 ./install.sh              instala sin añadirla a la biblioteca de Steam
set -euo pipefail

REPO="Sergi-Morant/open-control-for-sphero-droids"
ASSET="open-control-for-sphero-droids-linux-x86_64.tar.gz"
BIN="open-control-for-sphero-droids"
APP_DIR="$HOME/.local/share/open-control-for-sphero-droids"
LAUNCHER="$HOME/.local/bin/$BIN"
DESKTOP_FILE="$HOME/.local/share/applications/$BIN.desktop"

# Marca de que ya se añadió a Steam, para no duplicar la entrada al actualizar. Va fuera de APP_DIR.
STEAM_MARK="$HOME/.local/share/$BIN.steam-added"

if [[ "${1:-}" == "--uninstall" ]]; then
  rm -rf "$APP_DIR" "$LAUNCHER" "$DESKTOP_FILE" "$STEAM_MARK"
  echo "Desinstalado. Si la añadiste a Steam, quítala desde la biblioteca:"
  echo "  clic derecho sobre el juego > Administrar > Eliminar de la biblioteca."
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

# Añadirla a la biblioteca de Steam, para abrirla desde el modo juego con los controles de la Deck.
added=no
if [[ -f "$STEAM_MARK" ]]; then
  added=yes
elif [[ "${NO_STEAM:-}" != "1" ]] && command -v steamos-add-to-steam >/dev/null; then
  if ! pgrep -x steam >/dev/null; then
    echo "AVISO: Steam no está abierto; ábrelo y vuelve a ejecutar este script para añadirla a la biblioteca."
  elif steamos-add-to-steam "$DESKTOP_FILE" >/dev/null 2>&1; then
    touch "$STEAM_MARK"
    added=new
  fi
fi

echo
echo "Listo. Ábrela desde el menú de aplicaciones o con: $BIN"
echo
case "$added" in
  new)
    echo "Añadida a tu biblioteca de Steam como «Open control for Sphero Droids»"
    echo "(sección «No de Steam»). Vuelve al modo juego y ábrela desde ahí: los botones"
    echo "y sticks de la Deck le llegan como un mando."
    ;;
  yes)
    echo "Ya estaba en tu biblioteca de Steam; la entrada sigue apuntando a la versión nueva."
    ;;
  *)
    echo "No se pudo añadir a Steam automáticamente. Hazlo a mano en el modo escritorio:"
    echo "  Steam > Juegos > Añadir un juego que no es de Steam > «Open control for Sphero Droids»."
    ;;
esac
