# 3D New Era AI — atalhos de desenvolvimento.
# `make` sozinho lista tudo.

SHELL := /usr/bin/env bash
.DEFAULT_GOAL := help

CARGO   ?= cargo
ADDR    ?= 127.0.0.1:7878
BIN     := target/debug/newera
RELEASE := target/release/newera
LOG     ?= info,wgpu_core=warn,wgpu_hal=warn,naga=warn,rmcp=warn,egui_wgpu=warn

export NEWERA_ADDR := $(ADDR)
export NEWERA_LOG  := $(LOG)

BOLD  := \033[1m
CYAN  := \033[36m
DIM   := \033[2m
RESET := \033[0m

##@ Rodar

.PHONY: run
run: ## Abre o editor com a casa demo + HTTP + MCP (o "rodar e tudo funciona")
	$(CARGO) run -p newera -- --demo

.PHONY: run-empty
run-empty: ## Abre o editor com um projeto vazio
	$(CARGO) run -p newera

.PHONY: run-release
run-release: ## Abre o editor em modo release (renderização bem mais fluida)
	$(CARGO) run -p newera --release -- --demo

.PHONY: dev
dev: ## Recompila e reabre o editor a cada alteração (usa cargo-watch)
	@command -v cargo-watch >/dev/null || { echo "cargo-watch ausente: rode 'make setup'"; exit 1; }
	cargo watch -c -w crates -x "run -p newera -- --demo"

.PHONY: serve
serve: ## Só o servidor HTTP + MCP, sem janela (headless)
	$(CARGO) run -p newera -- serve --demo

.PHONY: mcp-stdio
mcp-stdio: ## MCP via stdin/stdout (para clientes que iniciam o processo)
	$(CARGO) run -q -p newera -- mcp --demo

.PHONY: web
web: ## Compila o visualizador web (WebAssembly) em web/
	@rustup target list --installed | grep -q wasm32-unknown-unknown || rustup target add wasm32-unknown-unknown
	$(CARGO) build -p newera-web --release --target wasm32-unknown-unknown
	cp target/wasm32-unknown-unknown/release/newera_web.wasm web/

.PHONY: web-editor
web-editor: ## Compila o editor completo para o navegador em web/editor/
	@rustup target list --installed | grep -q wasm32-unknown-unknown || rustup target add wasm32-unknown-unknown
	@command -v wasm-bindgen >/dev/null || cargo install wasm-bindgen-cli --version 0.2.128 --locked
	$(CARGO) build -p newera-editor-web --release --target wasm32-unknown-unknown
	wasm-bindgen --target web --no-typescript --out-dir web/editor/pkg target/wasm32-unknown-unknown/release/newera_editor_web.wasm

.PHONY: web-serve
web-serve: web web-editor ## Serve o visualizador (/) e o editor (/editor/) em http://127.0.0.1:8790
	python3 -m http.server 8790 --bind 127.0.0.1 --directory web

##@ Qualidade

.PHONY: check
check: fmt-check lint test smoke ## Tudo que o CI roda: fmt, clippy, testes e smoke do MCP

.PHONY: fmt
fmt: ## Formata o código
	$(CARGO) fmt --all

.PHONY: fmt-check
fmt-check: ## Verifica formatação sem alterar arquivos
	$(CARGO) fmt --all -- --check

.PHONY: lint
lint: ## Clippy pedantic, warnings viram erro
	$(CARGO) clippy --workspace --all-targets --locked -- -D warnings

.PHONY: test
test: ## Testes unitários e de integração
	$(CARGO) test --workspace --locked

.PHONY: smoke
smoke: build ## Sobe o servidor e conversa com o MCP como uma IA faria
	scripts/mcp-smoke.sh $(BIN)

.PHONY: deny
deny: ## Audita licenças e vulnerabilidades das dependências (cargo-deny)
	cargo deny check

.PHONY: fix
fix: ## Aplica sugestões automáticas do clippy e formata
	$(CARGO) clippy --workspace --all-targets --fix --allow-dirty --allow-staged
	$(CARGO) fmt --all

##@ Build

.PHONY: build
build: ## Build de debug
	$(CARGO) build -p newera

.PHONY: release
release: ## Build otimizado em target/release/newera
	$(CARGO) build -p newera --release
	@ls -lh $(RELEASE)

.PHONY: doc
doc: ## Gera e abre a documentação das crates
	$(CARGO) doc --workspace --no-deps --open

.PHONY: clean
clean: ## Remove artefatos de build
	$(CARGO) clean

##@ IA / MCP

.PHONY: mcp-add-claude
mcp-add-claude: ## Registra o MCP do editor no Claude Code (com o editor aberto)
	claude mcp add --transport http newera http://$(ADDR)/mcp

.PHONY: mcp-tools
mcp-tools: ## Lista as ferramentas MCP expostas (precisa do editor/servidor rodando)
	@H=(-H "Content-Type: application/json" -H "Accept: application/json, text/event-stream"); \
	curl -s "$${H[@]}" -D /tmp/newera-mcp.h http://$(ADDR)/mcp \
	  -d '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"make","version":"1"}}}' >/dev/null; \
	S=$$(grep -i mcp-session-id /tmp/newera-mcp.h | awk '{print $$2}' | tr -d '\r'); \
	H+=(-H "Mcp-Session-Id: $$S" -H "MCP-Protocol-Version: 2025-06-18"); \
	curl -s "$${H[@]}" http://$(ADDR)/mcp -d '{"jsonrpc":"2.0","method":"notifications/initialized"}' >/dev/null; \
	curl -s "$${H[@]}" http://$(ADDR)/mcp -d '{"jsonrpc":"2.0","id":2,"method":"tools/list"}' \
	  | sed -n 's/^data: //p' \
	  | python3 -c 'import json,sys; [print("  " + t["name"] + " — " + t.get("description", "")) for t in json.load(sys.stdin)["result"]["tools"]]'

.PHONY: mcp
mcp: ## Chama uma ferramenta MCP: make mcp TOOL=get_home ARGS='{}'
	@scripts/mcp.sh $(TOOL) '$(or $(ARGS),{})'

.PHONY: home
home: ## Mostra o JSON da casa aberta (GET /api/home)
	@curl -s http://$(ADDR)/api/home | python3 -m json.tool

##@ Ambiente

.PHONY: setup
setup: ## Instala componentes e ferramentas de desenvolvimento
	rustup component add rustfmt clippy
	$(CARGO) install --locked cargo-watch cargo-deny
	@if [ "$$(uname)" = "Linux" ]; then \
	  echo -e "\n$(DIM)Dependências de sistema (Debian/Ubuntu), se faltar alguma:$(RESET)"; \
	  echo "  sudo apt install libxkbcommon-dev libwayland-dev libx11-dev libxcursor-dev libxrandr-dev libxi-dev libvulkan1"; \
	fi

.PHONY: help
help: ## Lista os comandos
	@awk 'BEGIN {FS = ":.*##"; printf "\n$(BOLD)3D New Era AI$(RESET)  $(DIM)make <alvo> [ADDR=127.0.0.1:7878]$(RESET)\n"} \
	  /^[a-zA-Z_-]+:.*?##/ { printf "  $(CYAN)%-16s$(RESET) %s\n", $$1, $$2 } \
	  /^##@/ { printf "\n$(BOLD)%s$(RESET)\n", substr($$0, 5) }' $(MAKEFILE_LIST)
	@echo
