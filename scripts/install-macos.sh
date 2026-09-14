#!/usr/bin/env bash
# Builds and installs 3D New Era AI on a Mac from source — no Apple Developer
# account needed. A binary built on the machine itself is never quarantined,
# so Gatekeeper opens it without complaints.
#
#   curl -fsSL https://raw.githubusercontent.com/leandrodaf/3d-new-era-ai/main/scripts/install-macos.sh | bash
#
# What it does:
#   1. Xcode Command Line Tools (compiler and linker), if missing
#   2. Rust through rustup, if missing
#   3. Clones or updates the source in ~/.newera/src
#   4. cargo build --release
#   5. "3D New Era AI.app" in ~/Applications (Spotlight and Launchpad find it)
#   6. `newera` command in ~/.local/bin
#   7. Registers the MCP server in Claude Code, if installed
#
# Environment: NEWERA_HOME (default ~/.newera), NEWERA_REF (branch or tag,
# default main), NEWERA_APPS (default ~/Applications).
set -euo pipefail

REPO="https://github.com/leandrodaf/3d-new-era-ai.git"
NEWERA_HOME="${NEWERA_HOME:-$HOME/.newera}"
NEWERA_REF="${NEWERA_REF:-main}"
NEWERA_APPS="${NEWERA_APPS:-$HOME/Applications}"
SRC="$NEWERA_HOME/src"
BIN_DIR="$HOME/.local/bin"
APP="$NEWERA_APPS/3D New Era AI.app"

step() { printf '\n\033[1;34m==>\033[0m \033[1m%s\033[0m\n' "$*"; }
fail() { printf '\033[1;31merro:\033[0m %s\n' "$*" >&2; exit 1; }

[ "$(uname -s)" = "Darwin" ] || fail "este script é para macOS (no Linux/Windows veja o README)."

step "Xcode Command Line Tools"
if ! xcode-select -p >/dev/null 2>&1; then
    xcode-select --install || true
    echo "Uma janela pediu para instalar as Command Line Tools. Conclua a instalação;"
    echo "o script continua sozinho quando terminar."
    until xcode-select -p >/dev/null 2>&1; do sleep 10; done
fi
echo "ok: $(xcode-select -p)"

step "Rust"
if ! command -v cargo >/dev/null 2>&1 && [ ! -x "$HOME/.cargo/bin/cargo" ]; then
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal
fi
# shellcheck disable=SC1091
[ -f "$HOME/.cargo/env" ] && . "$HOME/.cargo/env"
command -v cargo >/dev/null 2>&1 || fail "cargo não encontrado depois de instalar o rustup."
rustup update stable >/dev/null 2>&1 || true
echo "ok: $(cargo --version)"

step "Código-fonte ($NEWERA_REF)"
command -v git >/dev/null 2>&1 || fail "git não encontrado (vem com as Command Line Tools)."
if [ -d "$SRC/.git" ]; then
    git -C "$SRC" fetch --depth 1 origin "$NEWERA_REF"
    git -C "$SRC" checkout -q --force FETCH_HEAD
else
    mkdir -p "$NEWERA_HOME"
    git clone --depth 1 --branch "$NEWERA_REF" "$REPO" "$SRC"
fi
echo "ok: $(git -C "$SRC" log -1 --format='%h %s')"

step "Compilando (a primeira vez leva alguns minutos)"
# rust-toolchain.toml in the repo picks the toolchain and components.
# Half the cores keeps the Mac usable (and cool) while it builds.
JOBS=$(( $(sysctl -n hw.ncpu) / 2 ))
[ "$JOBS" -ge 1 ] || JOBS=1
(cd "$SRC" && cargo build -p newera --release --locked -j "$JOBS")
BUILT="$SRC/target/release/newera"
[ -x "$BUILT" ] || fail "o build terminou sem gerar $BUILT."

step "Aplicativo em $APP"
VERSION=$(sed -n 's/^version = "\(.*\)"/\1/p' "$SRC/Cargo.toml" | head -1)
rm -rf "$APP"
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources"
cp "$BUILT" "$APP/Contents/MacOS/newera"
cat > "$APP/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleName</key><string>3D New Era AI</string>
    <key>CFBundleDisplayName</key><string>3D New Era AI</string>
    <key>CFBundleIdentifier</key><string>io.github.leandrodaf.newera</string>
    <key>CFBundleExecutable</key><string>newera</string>
    <key>CFBundlePackageType</key><string>APPL</string>
    <key>CFBundleShortVersionString</key><string>${VERSION}</string>
    <key>CFBundleVersion</key><string>${VERSION}</string>
    <key>LSMinimumSystemVersion</key><string>11.0</string>
    <key>NSHighResolutionCapable</key><true/>
    <key>CFBundleDocumentTypes</key>
    <array>
        <dict>
            <key>CFBundleTypeName</key><string>3D New Era AI project</string>
            <key>CFBundleTypeExtensions</key><array><string>newera</string><string>sh3d</string></array>
            <key>CFBundleTypeRole</key><string>Editor</string>
        </dict>
    </array>
</dict>
</plist>
PLIST
# Ad-hoc signature: Apple Silicon only runs signed code, and no account is needed.
codesign --force --deep --sign - "$APP" >/dev/null 2>&1 || true
xattr -dr com.apple.quarantine "$APP" 2>/dev/null || true
echo "ok"

step "Comando newera em $BIN_DIR"
mkdir -p "$BIN_DIR"
ln -sf "$APP/Contents/MacOS/newera" "$BIN_DIR/newera"
case ":$PATH:" in
    *":$BIN_DIR:"*) ;;
    *)
        PROFILE="$HOME/.zprofile"
        [ "${SHELL##*/}" = "bash" ] && PROFILE="$HOME/.bash_profile"
        grep -qs '.local/bin' "$PROFILE" || echo 'export PATH="$HOME/.local/bin:$PATH"' >> "$PROFILE"
        echo "adicionado ao PATH em $PROFILE (abra um terminal novo)"
        ;;
esac
echo "ok"

if command -v claude >/dev/null 2>&1; then
    step "MCP no Claude Code"
    claude mcp add --scope user --transport http newera http://127.0.0.1:7878/mcp >/dev/null 2>&1 \
        && echo "ok: servidor 'newera' registrado" \
        || echo "já registrado (ou falhou): claude mcp list"
fi

printf '\n\033[1;32mPronto!\033[0m\n'
cat <<DONE
  Abrir:        open "$APP"   (ou Spotlight: "3D New Era AI")
  Terminal:     newera --demo
  Sem janela:   newera serve
  MCP:          http://127.0.0.1:7878/mcp (com o editor aberto)
  Atualizar:    rode este script de novo
  Desinstalar:  rm -rf "$APP" "$NEWERA_HOME" "$BIN_DIR/newera"
DONE
