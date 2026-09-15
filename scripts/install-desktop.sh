#!/bin/sh
# Puts 3D New Era AI in the Linux desktop: menu entry, icons at every size,
# the .newera file type and its document icon. User-only, no root.
#
#   ./install-desktop.sh              install (or update)
#   ./install-desktop.sh --uninstall  remove it again
#
# Shipped inside newera-linux-x64.tar.gz, next to the binary.
set -eu

here=$(cd "$(dirname "$0")" && pwd)
data="${XDG_DATA_HOME:-$HOME/.local/share}"
bin="$HOME/.local/bin"
desktop="$data/applications/newera.desktop"
mime="$data/mime/packages/newera.xml"
metainfo="$data/metainfo/io.github.leandrodaf.newera.metainfo.xml"

refresh() {
    command -v update-desktop-database >/dev/null 2>&1 && update-desktop-database "$data/applications" || true
    command -v update-mime-database >/dev/null 2>&1 && update-mime-database "$data/mime" || true
    command -v gtk-update-icon-cache >/dev/null 2>&1 && gtk-update-icon-cache -f -t "$data/icons/hicolor" 2>/dev/null || true
}

if [ "${1:-}" = "--uninstall" ]; then
    rm -f "$desktop" "$mime" "$metainfo" "$bin/newera"
    find "$data/icons/hicolor" -name 'newera.png' -o -name 'newera.svg' \
        -o -name 'application-x-newera.png' -o -name 'application-x-newera.svg' 2>/dev/null \
        | while read -r icon; do rm -f "$icon"; done
    refresh
    echo "removidos: atalho, ícones, tipo .newera e $bin/newera"
    exit 0
fi

put() { # put <mode> <source> <destination>  (no GNU install needed)
    mkdir -p "$(dirname "$3")"
    cp "$2" "$3"
    chmod "$1" "$3"
}

put 755 "$here/newera" "$bin/newera"

# Icons: merge the hicolor tree into the user's theme.
mkdir -p "$data/icons/hicolor"
cp -r "$here/share/icons/hicolor/." "$data/icons/hicolor/"

# Menu entry, with the absolute path so it runs whatever PATH the session has.
mkdir -p "$(dirname "$desktop")"
sed -e "s|^Exec=newera|Exec=$bin/newera|" -e "s|^TryExec=newera|TryExec=$bin/newera|" \
    "$here/share/applications/newera.desktop" > "$desktop"
chmod 644 "$desktop"

put 644 "$here/share/mime/packages/newera.xml" "$mime"
put 644 "$here/share/metainfo/io.github.leandrodaf.newera.metainfo.xml" "$metainfo"
refresh

# Make it the default opener for .newera, when the tool is around.
command -v xdg-mime >/dev/null 2>&1 && xdg-mime default newera.desktop application/x-newera || true

echo "3D New Era AI no menu; .newera com ícone e duplo clique; comando: $bin/newera"
case ":$PATH:" in
    *":$bin:"*) ;;
    *) echo "dica: adicione $bin ao seu PATH para usar 'newera' no terminal" ;;
esac
