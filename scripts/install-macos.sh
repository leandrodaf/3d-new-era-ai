#!/usr/bin/env bash
# Builds and installs 3D New Era AI on a Mac from source — no Apple Developer
# account needed: a binary built on the machine itself is never quarantined,
# so Gatekeeper opens it without complaints.
#
#   curl -fsSL https://raw.githubusercontent.com/leandrodaf/3d-new-era-ai/main/scripts/install-macos.sh | bash
#   ... | bash -s -- --uninstall
#
# Runs as your user only: nothing needs sudo, nothing is written outside your
# home folder and the temporary build folder.
#
#   - Already up to date? Nothing is downloaded (the commit is compared first).
#   - Rust already installed (rustup or Homebrew, 1.95+)? It is used as it is.
#     Otherwise a private Rust goes into the temporary folder and leaves with it.
#   - Run from inside a clone of the repository? That source is used, untouched.
#   - At the end — success or failure — the temporary folder (source, build,
#     private Rust) is deleted. What stays: the app and the `newera` link.
#
# Environment: NEWERA_REF (branch, tag or commit; default main),
# NEWERA_APPS (default ~/Applications), NEWERA_FORCE=1 to rebuild anyway.
set -euo pipefail

OWNER_REPO="leandrodaf/3d-new-era-ai"
NEWERA_REF="${NEWERA_REF:-main}"
NEWERA_APPS="${NEWERA_APPS:-$HOME/Applications}"
APP="$NEWERA_APPS/3D New Era AI.app"
BIN_DIR="$HOME/.local/bin"
MIN_RUST="1.95"

step() { printf '\n\033[1;34m==>\033[0m \033[1m%s\033[0m\n' "$*"; }
info() { printf '    %s\n' "$*"; }
fail() { printf '\n\033[1;31merro:\033[0m %s\n' "$*" >&2; exit 1; }

[ "$(uname -s)" = "Darwin" ] || fail "este script é para macOS (no Linux/Windows veja o README)."
[ "$(id -u)" -ne 0 ] || fail "não rode como root/sudo: tudo é instalado só para o seu usuário."

if [ "${1:-}" = "--uninstall" ]; then
    step "Desinstalando"
    rm -rf "$APP"
    [ -L "$BIN_DIR/newera" ] && rm -f "$BIN_DIR/newera"
    if command -v claude >/dev/null 2>&1 && claude mcp get newera >/dev/null 2>&1; then
        claude mcp remove --scope user newera >/dev/null 2>&1 || true
    fi
    info "removidos: $APP, $BIN_DIR/newera e o MCP 'newera' do Claude Code"
    exit 0
fi

# Everything temporary lives here and is deleted on any exit.
WORK=$(mktemp -d "${TMPDIR:-/tmp}/newera-install.XXXXXX")
cleanup() {
    # Build files can be read-only (cargo registry): make them removable first.
    chmod -R u+w "$WORK" 2>/dev/null || true
    rm -rf "$WORK"
}
trap cleanup EXIT
trap 'exit 130' INT TERM

version_ge() { # version_ge 1.96.0 1.95 -> true
    [ "$(printf '%s\n%s\n' "$2" "$1" | sort -t. -k1,1n -k2,2n -k3,3n | head -1)" = "$2" ]
}

step "Ferramentas do sistema"
# The compiler and linker come from Apple's Command Line Tools. Installing them
# asks for an administrator password, so the script never does it on its own.
if ! xcode-select -p >/dev/null 2>&1 || ! xcrun --find cc >/dev/null 2>&1; then
    fail "faltam as Xcode Command Line Tools (compilador e linker da Apple).
       Instale uma vez com:  xcode-select --install
       e rode este script de novo."
fi
command -v curl >/dev/null 2>&1 || fail "curl não encontrado."
info "Command Line Tools: $(xcode-select -p)"

step "Código-fonte"
LOCAL_SRC=""
for dir in "$PWD" "$(cd "$(dirname "${BASH_SOURCE[0]:-$0}")/.." 2>/dev/null && pwd || true)"; do
    if [ -n "$dir" ] && [ -f "$dir/crates/newera/Cargo.toml" ]; then
        LOCAL_SRC="$dir"
        break
    fi
done

if [ -n "$LOCAL_SRC" ]; then
    SRC="$LOCAL_SRC"
    COMMIT=$(git -C "$SRC" rev-parse HEAD 2>/dev/null || echo "local")
    info "usando o clone em $SRC ($COMMIT) — nada a baixar"
else
    COMMIT=$(curl -fsSL -H "Accept: application/vnd.github.sha" \
        "https://api.github.com/repos/$OWNER_REPO/commits/$NEWERA_REF") \
        || fail "não consegui consultar $OWNER_REPO@$NEWERA_REF no GitHub."
    info "$NEWERA_REF está em ${COMMIT:0:12}"
fi

INSTALLED=$(cat "$APP/Contents/Resources/commit" 2>/dev/null || true)
if [ "${NEWERA_FORCE:-0}" != "1" ] && [ "$COMMIT" != "local" ] && [ "$INSTALLED" = "$COMMIT" ] \
    && [ -x "$APP/Contents/MacOS/newera" ]; then
    info "já instalado nesta versão: nada a baixar nem compilar (NEWERA_FORCE=1 recompila)"
    NEED_BUILD=0
else
    NEED_BUILD=1
fi

if [ "$NEED_BUILD" = 1 ] && [ -z "$LOCAL_SRC" ]; then
    SRC="$WORK/src"
    mkdir -p "$SRC"
    curl -fsSL "https://codeload.github.com/$OWNER_REPO/tar.gz/$COMMIT" \
        | tar -xz -C "$SRC" --strip-components 1
    info "baixado para a pasta temporária"
fi

if [ "$NEED_BUILD" = 1 ]; then
    step "Rust"
    # Use the Rust already on the machine when it is new enough; pinning the
    # toolchain keeps rustup from fetching extra components the repo lists.
    [ -f "$HOME/.cargo/env" ] && . "$HOME/.cargo/env"
    RUST_OK=0
    if command -v rustc >/dev/null 2>&1 && command -v cargo >/dev/null 2>&1; then
        if command -v rustup >/dev/null 2>&1; then
            TOOLCHAIN=$( (cd "$HOME" && rustup show active-toolchain 2>/dev/null) | awk '{print $1}')
            [ -n "$TOOLCHAIN" ] && export RUSTUP_TOOLCHAIN="$TOOLCHAIN"
        fi
        HAVE=$(rustc --version 2>/dev/null | awk '{print $2}')
        if [ -n "$HAVE" ] && version_ge "$HAVE" "$MIN_RUST"; then
            RUST_OK=1
            info "usando o Rust já instalado: $HAVE${RUSTUP_TOOLCHAIN:+ ($RUSTUP_TOOLCHAIN)}"
        else
            info "Rust ${HAVE:-?} é anterior ao $MIN_RUST exigido; não vou mexer nele"
            unset RUSTUP_TOOLCHAIN
        fi
    fi
    if [ "$RUST_OK" = 0 ]; then
        # Private, temporary Rust: never touches ~/.cargo, ~/.rustup or the shell profile.
        export RUSTUP_HOME="$WORK/rustup" CARGO_HOME="$WORK/cargo"
        curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs \
            | sh -s -- -y --profile minimal --default-toolchain stable --no-modify-path >/dev/null
        export PATH="$CARGO_HOME/bin:$PATH" RUSTUP_TOOLCHAIN=stable
        info "Rust temporário: $(rustc --version) (apagado no final)"
    fi

    step "Compilando (a primeira vez leva alguns minutos)"
    # Half the cores keeps the Mac usable and cool while it builds.
    JOBS=$(($(sysctl -n hw.ncpu) / 2))
    [ "$JOBS" -ge 1 ] || JOBS=1
    # Build output goes to the temporary folder, even for a local clone.
    export CARGO_TARGET_DIR="$WORK/target"
    (cd "$SRC" && cargo build -p newera --release --locked -j "$JOBS")
    BUILT="$CARGO_TARGET_DIR/release/newera"
    [ -x "$BUILT" ] || fail "o build terminou sem gerar o binário."

    step "Aplicativo em $APP"
    VERSION=$(sed -n 's/^version = "\(.*\)"/\1/p' "$SRC/Cargo.toml" | head -1)
    mkdir -p "$NEWERA_APPS"
    STAGE="$WORK/3D New Era AI.app"
    mkdir -p "$STAGE/Contents/MacOS" "$STAGE/Contents/Resources"
    cp "$BUILT" "$STAGE/Contents/MacOS/newera"
    echo "$COMMIT" > "$STAGE/Contents/Resources/commit"
    cat > "$STAGE/Contents/Info.plist" <<PLIST
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
    codesign --force --deep --sign - "$STAGE" >/dev/null 2>&1 || true
    # Swap in the new app only once it is complete.
    rm -rf "$APP"
    mv "$STAGE" "$APP"
    info "ok (${COMMIT:0:12})"
fi

step "Comando newera em $BIN_DIR"
mkdir -p "$BIN_DIR"
ln -sfn "$APP/Contents/MacOS/newera" "$BIN_DIR/newera"
case ":$PATH:" in
    *":$BIN_DIR:"*) info "ok" ;;
    *)
        PROFILE="$HOME/.zprofile"
        [ "${SHELL##*/}" = "bash" ] && PROFILE="$HOME/.bash_profile"
        if ! grep -qs 'newera: ~/.local/bin' "$PROFILE"; then
            printf '\n# newera: ~/.local/bin\nexport PATH="$HOME/.local/bin:$PATH"\n' >> "$PROFILE"
        fi
        info "adicionado ao PATH em $PROFILE (abra um terminal novo)"
        ;;
esac

if command -v claude >/dev/null 2>&1; then
    step "MCP no Claude Code"
    if claude mcp get newera >/dev/null 2>&1; then
        info "já registrado"
    elif claude mcp add --scope user --transport http newera http://127.0.0.1:7878/mcp >/dev/null 2>&1; then
        info "servidor 'newera' registrado"
    else
        info "não consegui registrar; rode: claude mcp add --transport http newera http://127.0.0.1:7878/mcp"
    fi
fi

step "Limpando arquivos temporários"
info "a pasta de build é apagada ao sair"

printf '\n\033[1;32mPronto!\033[0m\n'
cat <<DONE
  Abrir:        open "$APP"   (ou Spotlight: "3D New Era AI")
  Terminal:     newera --demo
  Sem janela:   newera serve
  MCP:          http://127.0.0.1:7878/mcp (com o editor aberto)
  Atualizar:    rode o mesmo comando de novo
  Desinstalar:  rode com --uninstall
DONE
