# 3D New Era AI

[English](README.md) · **Português (Brasil)** · **[Site](https://3dneweraai.com/)**

[![CI](https://github.com/leandrodaf/3d-new-era-ai/actions/workflows/ci.yml/badge.svg)](https://github.com/leandrodaf/3d-new-era-ai/actions/workflows/ci.yml)
[![Licença: MIT OU Apache-2.0](https://img.shields.io/badge/licen%C3%A7a-MIT%20OU%20Apache--2.0-blue.svg)](LICENSE-MIT)
[![Última versão](https://img.shields.io/github/v/release/leandrodaf/3d-new-era-ai?label=vers%C3%A3o)](https://github.com/leandrodaf/3d-new-era-ai/releases/latest)

Um editor de **projetos de casa, planta baixa e interiores** de código aberto, feito em
Rust e inspirado no Sweet Home 3D — para Windows, macOS e Linux — que já nasceu **nativo
para IA**: o editor vem com um servidor
[Model Context Protocol](https://modelcontextprotocol.io) embutido, então um agente de IA
desenha paredes, mobilia cômodos, projeta a marcenaria, confere iluminação e ergonomia
pelas normas e renderiza as fotos junto com você, com cada alteração aparecendo na tela na
hora e a um Ctrl+Z de distância. Tudo roda na sua máquina: sem conta, sem assinatura, sem
nuvem. A interface fala português, inglês, espanhol e francês.

![Editor com um apartamento de 105 m² mobiliado: catálogo, planta humanizada e vista 3D ao vivo](docs/images/editor.png)

## Baixar

**Windows** — abra o PowerShell e cole:

```powershell
irm https://raw.githubusercontent.com/leandrodaf/3d-new-era-ai/main/scripts/install-windows.ps1 | iex
```

**macOS** (Apple Silicon e Intel) — abra o Terminal e cole:

```sh
curl -fsSL https://raw.githubusercontent.com/leandrodaf/3d-new-era-ai/main/scripts/install-macos.sh | bash
```

Pronto: o app aparece no Menu Iniciar / Launchpad, sem senha de administrador, e o mesmo
comando atualiza. Prefere baixar o arquivo?

| [Windows (64 bits)](https://github.com/leandrodaf/3d-new-era-ai/releases/latest/download/newera-windows-x64.zip) | [Mac Apple Silicon](https://github.com/leandrodaf/3d-new-era-ai/releases/latest/download/newera-macos-apple-silicon.zip) | [Mac Intel](https://github.com/leandrodaf/3d-new-era-ai/releases/latest/download/newera-macos-intel.zip) | [Linux (x64)](https://github.com/leandrodaf/3d-new-era-ai/releases/latest/download/newera-linux-x64.tar.gz) |
|:---:|:---:|:---:|:---:|

<details>
<summary>Primeira abertura de um arquivo baixado</summary>

Os programas não são assinados com certificado pago da Apple ou da Microsoft, então o
sistema pergunta uma vez:

- **Windows:** descompacte e abra o `newera-gui.exe`; se aparecer o SmartScreen, clique em
  *Mais informações* → *Executar assim mesmo*.
- **macOS:** descompacte e mova o *3D New Era AI* para Aplicativos. Clique com o botão
  direito → *Abrir* → *Abrir*. No macOS 15 ou mais novo, tente abrir uma vez e depois vá em
  *Ajustes do Sistema* → *Privacidade e Segurança* → *Abrir Mesmo Assim*. Ou rode
  `xattr -dr com.apple.quarantine "/Applications/3D New Era AI.app"`.
- **Linux:** `tar xzf newera-linux-x64.tar.gz && ./newera/newera`. Rode
  `./newera/install-desktop.sh` para pôr o app no menu com ícone e dar aos
  arquivos `.newera` ícone e duplo clique (`--uninstall` desfaz).

Os instaladores de uma linha acima já evitam esses avisos.
</details>

Depois: **[conecte sua IA](#conecte-sua-ia)** — Claude, Codex, Gemini, Cursor, VS Code,
DeepSeek e outras.

## Vitrine

Um apartamento real de 105 m², redesenhado a partir de uma planta com licença aberta,
mobiliado, iluminado e fotografado por um agente de IA só pelo MCP — paredes tiradas da
imagem, portas abrindo para o lado certo, painel ripado da TV, cozinha em L e armário feitos
com `cabinet_run`, spots dimensionados por fotometria pela NBR ISO/CIE 8995-1 e fotos com
path tracing. Todas as imagens abaixo saíram do próprio editor.
**[Veja como foi feito →](docs/SHOWCASE.md)**

| Planta humanizada com os acabamentos reais | Vista aérea em corte |
|---|---|
| ![Planta humanizada com acabamentos reais](docs/images/showcase/13-planta-humanizada.jpg) | ![Vista aérea em corte do apartamento](docs/images/showcase/12-aerea.jpg) |

| De dia | Ao anoitecer, luzes acesas |
|---|---|
| ![Sala com painel ripado da TV](docs/images/showcase/01-estar-janela.jpg) | ![Mesa de jantar sob o pendente ao anoitecer](docs/images/showcase/09-jantar-noite.jpg) |
| ![Jantar e estar olhando para a janela](docs/images/showcase/02-jantar-estar.jpg) | ![Cozinha em L à noite](docs/images/showcase/10-cozinha-noite.jpg) |
| ![Cozinha em L com piso de granilite](docs/images/showcase/04-cozinha.jpg) | ![Suíte do casal à noite](docs/images/showcase/11-suite-noite.jpg) |

## Conecte sua IA

Abra o editor (ou rode `newera serve` para ficar sem janela). Ele serve o MCP em
**`http://127.0.0.1:7878/mcp`** — aponte sua IA para esse endereço e ela edita a planta que
você está vendo, ao vivo.

**Claude Code**

```sh
claude mcp add --transport http newera http://127.0.0.1:7878/mcp
```

Dentro de um clone deste repositório já vem configurado pelo `.mcp.json` (aprove o `newera` uma vez).

**Codex CLI**

```sh
codex mcp add newera --url http://127.0.0.1:7878/mcp
```

**Gemini CLI**

```sh
gemini mcp add --transport http newera http://127.0.0.1:7878/mcp
```

**VS Code (modo agente do Copilot)**

```sh
code --add-mcp '{"name":"newera","type":"http","url":"http://127.0.0.1:7878/mcp"}'
```

**Cursor** — `~/.cursor/mcp.json`

```json
{ "mcpServers": { "newera": { "url": "http://127.0.0.1:7878/mcp" } } }
```

**Windsurf** — `~/.codeium/windsurf/mcp_config.json`

```json
{ "mcpServers": { "newera": { "serverUrl": "http://127.0.0.1:7878/mcp" } } }
```

**Claude Desktop** — *Configurações → Desenvolvedor → Editar configuração* (precisa do
[Node.js](https://nodejs.org) para a ponte `mcp-remote`)

```json
{ "mcpServers": { "newera": { "command": "npx", "args": ["-y", "mcp-remote", "http://127.0.0.1:7878/mcp"] } } }
```

**DeepSeek, Qwen, Llama e outros modelos** — o que importa não é o modelo, e sim o app em que
você conversa. Use um que suporte MCP e adicione o endereço acima: [Cline](https://cline.bot)
ou [Roo Code](https://roocode.com) no VS Code, [Cherry Studio](https://cherry-ai.com),
[LM Studio](https://lmstudio.ai) ou [opencode](https://opencode.ai) (`opencode.json`):

```json
{ "mcp": { "newera": { "type": "remote", "url": "http://127.0.0.1:7878/mcp" } } }
```

**Qualquer outro cliente** — HTTP (streamable) em `http://127.0.0.1:7878/mcp` ou, para
clientes que só iniciam um processo, stdio com `newera mcp`:

```json
{ "mcpServers": { "newera": { "command": "newera", "args": ["mcp"] } } }
```

Pelo stdio o agente trabalha num projeto próprio em segundo plano, não na janela aberta;
use o endereço HTTP para projetar junto, ao vivo. Depois é só pedir, por exemplo: *"Desenhe
um quarto de 4 × 5 m com porta e janela, mobilie e gere uma foto."*

---

**Telemetria.** As versões publicadas enviam relatórios de erro aos desenvolvedores pelo
Sentry, junto com as notas que os agentes deixam pela ferramenta `feedback` do MCP. Vem
**ligada por padrão** e desliga com um clique em **Ajuda → Enviar relatórios de erro** ou com
`newera telemetry off`. Nada do projeto é enviado, nem o IP nem o nome da máquina. As
versões publicadas também contam no Google Analytics que o app foi aberto, com versão,
sistema e modo — mesma chave de liga/desliga, um id sorteado para a instalação e nada do
projeto.

De onde vêm os números quando o editor diz que uma cozinha está errada — as normas, a
doutrina e a pesquisa por trás de cada regra, com edição e link: [docs/NORMAS.md](docs/NORMAS.md).

Documentação técnica completa (ferramentas MCP, modos, compilação e arquitetura) no
[README em inglês](README.md). Licença MIT ou Apache 2.0.
