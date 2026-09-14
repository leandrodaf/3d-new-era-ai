#!/usr/bin/env bash
# Installs 3D New Era AI on a Mac, for your user only (no sudo).
#
#   curl -fsSL https://raw.githubusercontent.com/leandrodaf/3d-new-era-ai/main/scripts/install-macos.sh | bash
#   ... | bash -s -- --uninstall
#
# By default it downloads the app from the latest release (Apple Silicon or
# Intel) — seconds, no compiler. Files fetched with curl carry no quarantine
# flag, so Gatekeeper opens the app without the "unidentified developer" wall.
# When there is no build for this Mac, or with NEWERA_REF=<branch|tag|commit>,
# it builds from source instead.
#
#   - Already up to date? Nothing is downloaded (the commit is compared first).
#   - Building: Rust already installed (rustup or Homebrew, 1.95+) is used as
#     it is; otherwise a private Rust goes into the temporary folder and leaves
#     with it. Run from inside a clone, that source is used untouched.
#   - At the end — success or failure — the temporary folder is deleted. What
#     stays: the app in ~/Applications and the `newera` link in ~/.local/bin.
#
# Environment: NEWERA_REF (build this branch, tag or commit from source),
# NEWERA_APPS (default ~/Applications), NEWERA_FORCE=1 to reinstall anyway.
set -euo pipefail

OWNER_REPO="leandrodaf/3d-new-era-ai"
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
    if command -v codex >/dev/null 2>&1 && codex mcp get newera >/dev/null 2>&1; then
        codex mcp remove newera >/dev/null 2>&1 || true
    fi
    info "removidos: $APP, $BIN_DIR/newera e o MCP 'newera' do Claude Code e do Codex"
    exit 0
fi

command -v curl >/dev/null 2>&1 || fail "curl não encontrado."

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

github_sha() { # github_sha <ref> -> commit sha
    curl -fsSL -H "Accept: application/vnd.github.sha" \
        "https://api.github.com/repos/$OWNER_REPO/commits/$1"
}

installed_commit() { cat "$APP/Contents/Resources/commit" 2>/dev/null || true; }

up_to_date() { # up_to_date <commit>
    [ "${NEWERA_FORCE:-0}" != "1" ] && [ "$1" != "local" ] \
        && [ "$(installed_commit)" = "$1" ] && [ -x "$APP/Contents/MacOS/newera" ]
}

place_app() { # place_app <built .app>: swap in the new app only once it is complete
    mkdir -p "$NEWERA_APPS"
    rm -rf "$APP"
    mv "$1" "$APP"
}

local_source() {
    for dir in "$PWD" "$(cd "$(dirname "${BASH_SOURCE[0]:-$0}")/.." 2>/dev/null && pwd || true)"; do
        if [ -n "$dir" ] && [ -f "$dir/crates/newera/Cargo.toml" ]; then
            echo "$dir"
            return
        fi
    done
}

# Returns 0 when the release app was installed (or already was), 1 to build instead.
install_release() {
    case "$(uname -m)" in
        arm64) ASSET="newera-macos-apple-silicon.zip" ;;
        x86_64) ASSET="newera-macos-intel.zip" ;;
        *) return 1 ;;
    esac
    step "Última versão"
    TAG=$(curl -fsSL "https://api.github.com/repos/$OWNER_REPO/releases/latest" 2>/dev/null \
        | sed -n 's/.*"tag_name": *"\([^"]*\)".*/\1/p' | head -1)
    [ -n "$TAG" ] || { info "nenhuma release publicada ainda"; return 1; }
    COMMIT=$(github_sha "$TAG") || return 1
    info "$TAG (${COMMIT:0:12})"
    if up_to_date "$COMMIT"; then
        info "já instalado nesta versão: nada a baixar (NEWERA_FORCE=1 reinstala)"
        return 0
    fi

    step "Baixando $ASSET"
    curl -fL --progress-bar -o "$WORK/app.zip" \
        "https://github.com/$OWNER_REPO/releases/download/$TAG/$ASSET" \
        || { info "sem build pronto para este Mac nesta versão"; return 1; }
    mkdir -p "$WORK/unzip"
    ditto -x -k "$WORK/app.zip" "$WORK/unzip"
    local built="$WORK/unzip/3D New Era AI.app"
    "$built/Contents/MacOS/newera" --version >/dev/null 2>&1 \
        || { info "o app baixado não abriu neste Mac"; return 1; }
    place_app "$built"
    info "ok: $("$APP/Contents/MacOS/newera" --version)"
}

build_from_source() {
    step "Ferramentas do sistema"
    # The compiler and linker come from Apple's Command Line Tools. Installing
    # them asks for an administrator password, so the script never does it.
    if ! xcode-select -p >/dev/null 2>&1 || ! xcrun --find cc >/dev/null 2>&1; then
        fail "para compilar faltam as Xcode Command Line Tools (compilador da Apple).
       Instale uma vez com:  xcode-select --install
       e rode este script de novo."
    fi
    info "Command Line Tools: $(xcode-select -p)"

    step "Código-fonte"
    local src ref="${NEWERA_REF:-main}"
    src=$(local_source)
    if [ -n "$src" ]; then
        COMMIT=$(git -C "$src" rev-parse HEAD 2>/dev/null || echo "local")
        info "usando o clone em $src ($COMMIT) — nada a baixar"
    else
        COMMIT=$(github_sha "$ref") || fail "não consegui consultar $OWNER_REPO@$ref no GitHub."
        info "$ref está em ${COMMIT:0:12}"
    fi
    if up_to_date "$COMMIT"; then
        info "já instalado nesta versão: nada a baixar nem compilar (NEWERA_FORCE=1 recompila)"
        return
    fi
    if [ -z "$src" ]; then
        src="$WORK/src"
        mkdir -p "$src"
        curl -fsSL "https://codeload.github.com/$OWNER_REPO/tar.gz/$COMMIT" \
            | tar -xz -C "$src" --strip-components 1
        info "baixado para a pasta temporária"
    fi

    step "Rust"
    # Use the Rust already on the machine when it is new enough; pinning the
    # toolchain keeps rustup from fetching extra components the repo lists.
    # shellcheck disable=SC1091
    [ -f "$HOME/.cargo/env" ] && . "$HOME/.cargo/env"
    local rust_ok=0 have toolchain
    if command -v rustc >/dev/null 2>&1 && command -v cargo >/dev/null 2>&1; then
        if command -v rustup >/dev/null 2>&1; then
            toolchain=$( (cd "$HOME" && rustup show active-toolchain 2>/dev/null) | awk '{print $1}')
            [ -n "$toolchain" ] && export RUSTUP_TOOLCHAIN="$toolchain"
        fi
        have=$(rustc --version 2>/dev/null | awk '{print $2}')
        if [ -n "$have" ] && version_ge "$have" "$MIN_RUST"; then
            rust_ok=1
            info "usando o Rust já instalado: $have${RUSTUP_TOOLCHAIN:+ ($RUSTUP_TOOLCHAIN)}"
        else
            info "Rust ${have:-?} é anterior ao $MIN_RUST exigido; não vou mexer nele"
            unset RUSTUP_TOOLCHAIN
        fi
    fi
    if [ "$rust_ok" = 0 ]; then
        # Private, temporary Rust: never touches ~/.cargo, ~/.rustup or the shell profile.
        export RUSTUP_HOME="$WORK/rustup" CARGO_HOME="$WORK/cargo"
        curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs \
            | sh -s -- -y --profile minimal --default-toolchain stable --no-modify-path >/dev/null
        export PATH="$CARGO_HOME/bin:$PATH" RUSTUP_TOOLCHAIN=stable
        info "Rust temporário: $(rustc --version) (apagado no final)"
    fi

    step "Compilando (a primeira vez leva alguns minutos)"
    # Half the cores keeps the Mac usable and cool while it builds.
    local jobs=$(($(sysctl -n hw.ncpu) / 2))
    [ "$jobs" -ge 1 ] || jobs=1
    # Build output goes to the temporary folder, even for a local clone.
    export CARGO_TARGET_DIR="$WORK/target"
    (cd "$src" && cargo build -p newera --release --locked -j "$jobs")
    [ -x "$CARGO_TARGET_DIR/release/newera" ] || fail "o build terminou sem gerar o binário."

    step "Aplicativo"
    mkdir -p "$WORK/app"
    bash "$src/scripts/macos-app.sh" "$CARGO_TARGET_DIR/release/newera" "$WORK/app" "$COMMIT" >/dev/null
    place_app "$WORK/app/3D New Era AI.app"
    info "ok (${COMMIT:0:12})"
}

if [ -n "${NEWERA_REF:-}" ] || [ -n "$(local_source)" ] || ! install_release; then
    build_from_source
fi
xattr -dr com.apple.quarantine "$APP" 2>/dev/null || true

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

MCP_URL="http://127.0.0.1:7878/mcp"
if command -v claude >/dev/null 2>&1; then
    step "MCP no Claude Code"
    if claude mcp get newera >/dev/null 2>&1; then
        info "já registrado"
    elif claude mcp add --scope user --transport http newera "$MCP_URL" >/dev/null 2>&1; then
        info "servidor 'newera' registrado"
    else
        info "não consegui registrar; rode: claude mcp add --transport http newera $MCP_URL"
    fi
fi
if command -v codex >/dev/null 2>&1; then
    step "MCP no Codex"
    if codex mcp get newera >/dev/null 2>&1; then
        info "já registrado"
    elif codex mcp add newera --url "$MCP_URL" >/dev/null 2>&1; then
        info "servidor 'newera' registrado"
    else
        info "não consegui registrar; rode: codex mcp add newera --url $MCP_URL"
    fi
fi

printf '\n\033[1;32mPronto!\033[0m (arquivos temporários apagados ao sair)\n'
cat <<DONE
  Abrir:        open "$APP"   (ou Spotlight: "3D New Era AI")
  Terminal:     newera --demo
  Sem janela:   newera serve
  MCP:          http://127.0.0.1:7878/mcp (com o editor aberto; outras IAs: veja o README)
  Atualizar:    rode o mesmo comando de novo
  Desinstalar:  rode com --uninstall
DONE
