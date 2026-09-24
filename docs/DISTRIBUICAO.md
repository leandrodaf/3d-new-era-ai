# 3D New Era AI: endgame de distribuição e monetização

Pesquisa feita em 24/09/2026. O documento começa pelo **estado final** do produto.
Depois vêm as **decisões que ficam fixas desde já** e as **etapas** para chegar lá.
Cada etapa entrega peças definitivas: nada construído numa etapa é jogado fora ou
reescrito na seguinte. A Parte II guarda a referência detalhada de cada canal,
marketplace e regra.

## Sumário

**Parte I: o plano**

1. [O endgame](#1-o-endgame)
2. [Onde estamos hoje](#2-onde-estamos-hoje)
3. [Decisões fixadas agora](#3-decisões-fixadas-agora)
4. [Etapas](#4-etapas)
5. [Canais destravados por etapa](#5-canais-destravados-por-etapa)
6. [Monetização com Polar.sh](#6-monetização-com-polarsh)
7. [Pendências e riscos](#7-pendências-e-riscos)
8. [Progresso](#8-progresso)

**Parte II: referência**

- [R1. Anthropic (Claude)](#r1-anthropic-claude)
- [R2. OpenAI (ChatGPT e Codex)](#r2-openai-chatgpt-e-codex)
- [R3. Registries e diretórios abertos](#r3-registries-e-diretórios-abertos)
- [R4. Instalação do binário](#r4-instalação-do-binário)
- [R5. Canais enterprise](#r5-canais-enterprise)
- [R6. Cobrança dentro dos diretórios](#r6-cobrança-dentro-dos-diretórios)
- [R7. Alternativas de pagamento avaliadas](#r7-alternativas-de-pagamento-avaliadas)
- [R8. Checklist de submissão](#r8-checklist-de-submissão)
- [Fontes](#fontes)

---

# Parte I: o plano

## 1. O endgame

**Um produto, uma superfície de MCP, três jeitos de usar.** A mesma lista de tools,
com as mesmas anotações e a mesma interface dentro do chat, responde:

- no app desktop;
- no editor aberto numa aba do navegador;
- na nuvem, sem nada aberto.

O usuário escolhe onde; a IA não percebe diferença.

```
 Clientes de IA: Claude (web, desktop, mobile, Code, Cowork) · ChatGPT · Codex
                 Cursor · VS Code · Gemini CLI · LM Studio · Zed · JetBrains ...
        │                                              │
        │ local: .mcpb, stdio, 127.0.0.1:7878          │ remoto: https://mcp.3dneweraai.com/mcp
        │ (grátis, sem conta, nada sai da máquina)     │ (OAuth; grátis com cota, "cafezinho" amplia)
        ▼                                              ▼
 ┌──────────────────┐        ┌────────────────────────── newera-cloud ───────────────────────────┐
 │ App desktop      │        │ contas + OAuth 2.1 (CIMD e DCR)    planos/cotas ◄── webhook Polar  │
 │ `newera`         │        │                                                                    │
 └──────────────────┘        │ roteador de sessão (por conta):                                    │
                             │   ├─ aba do usuário aberta em 3dneweraai.com/app → relay → a aba    │
                             │   └─ senão → motor headless → projeto na nuvem (.newera no R2)      │
                             │                                                                    │
                             │ link anônimo, sem conta: /r/<sala>/<token>/mcp  (o relay de hoje)  │
                             └────────────────────────────────────────────────────────────────────┘

 Todos os caminhos chamam o mesmo `newera-mcp`: mesmas tools, mesmas anotações,
 mesma interface no chat (MCP App com o viewer WASM).
```

### Como cada pessoa usa, no final

- **Quem está no ChatGPT ou no Claude:**
  1. Acha o app no diretório e clica em **Conectar**.
  2. Entra com o e-mail (OAuth).
  3. Pede "desenhe um quarto de 4 × 5 m e mobilie".
  4. A planta aparece interativa na conversa.
  5. "Abrir no editor" leva ao mesmo projeto em 3dneweraai.com/app, e "Baixar" entrega o
     `.newera` para abrir no desktop.
- **Quem usa o desktop:** instala com winget, brew, `.mcpb` ou o instalador. A IA fala
  com o app local, sem conta e sem nuvem, grátis para sempre.
- **Quem paga o cafezinho:** assina no site pelo Polar. Com o mesmo login, ganha cota
  maior, foto e vídeo renderizados na nuvem e mais projetos guardados.

### Promessas que não mudam

1. **Desktop e editor web: grátis, open source, sem conta. Sempre.**
2. **A nuvem é opcional.** Na nuvem o projeto é o mesmo arquivo `.newera` do desktop e
   pode ser baixado a qualquer momento, sem aprisionar ninguém.
3. **Uma só superfície de tools** (`newera-mcp`). Nenhum canal ganha um fork.
4. **Só se cobra o que custa servidor.** Recurso que roda na máquina do usuário nunca
   vira pago.

## 2. Onde estamos hoje

| Peça | Estado | Onde está |
|---|---|---|
| Motor, tools do MCP | Pronto | `crates/newera-mcp` (`tools()`, `call()`) |
| MCP HTTP local | Pronto | `crates/newera-server`, `127.0.0.1:7878/mcp` |
| MCP stdio | Pronto, mas trabalha num projeto próprio, não no da janela | `newera mcp` → `newera_mcp::serve_stdio` |
| Headless | Pronto | `newera serve` |
| Editor web (WASM) | Pronto | `crates/newera-editor-web`, 3dneweraai.com/app |
| Viewer WASM | Pronto | `crates/newera-web` |
| Relay público | Pronto, uma URL por aba | `crates/newera-relay` (biblioteca + binário), `mcp.3dneweraai.com` |
| Anotações das tools (`title`, `readOnlyHint`...) | **Falta** | exigidas por todos os diretórios |
| UI no chat (MCP Apps, `ui://`) | **Falta** | |
| URL fixa + OAuth + contas | **Falta** | |
| Projetos na nuvem | **Falta** | |
| `.mcpb`, `server.json`, plugin, manifestos de loja | **Falta** | |
| Privacidade e termos publicados | **Falta** | pedidos em todo formulário |

## 3. Decisões fixadas agora

Estas decisões são caras ou impossíveis de mudar depois, porque viram nome público,
URL em cadastro de terceiros ou fronteira entre módulos. Por isso ficam fixas antes da
primeira linha de código.

| # | Decisão | Escolha | Por que fixar agora |
|---|---|---|---|
| D1 | Nome no Official MCP Registry | **`com.3dneweraai/newera`** (namespace do domínio, não `io.github.leandrodaf/...`) | O nome é permanente. O mesmo registro recebe o `.mcpb` na Etapa 2 e a URL remota (`remotes`) na Etapa 4. Com `io.github` seria preciso publicar outro servidor e perder o histórico. |
| D2 | URL remota canônica | **`https://mcp.3dneweraai.com/mcp`** | Vai ficar gravada em diretórios, plugins e configurações de usuários. O subdomínio já é do relay. |
| D3 | Serviço hospedado | **Um binário só, `newera-cloud`**, que monta o `newera-relay` como biblioteca (`router_with` já existe) e soma MCP hospedado, OAuth, contas e webhooks | O relay não é reescrito, vira uma rota. Deploy, domínio e TLS continuam os mesmos. |
| D4 | Superfície de tools | **Anotações, `title` e o recurso de UI vivem em `newera-mcp`** | Desktop, relay e nuvem herdam de graça. Nada de "anotação só na nuvem". |
| D5 | Interface no chat | **Padrão aberto MCP Apps** (`_meta.ui`, `ui://`), não o Apps SDK proprietário | Um widget serve Claude, ChatGPT, VS Code, Cursor, Goose... |
| D6 | Conteúdo do widget | **SVG da planta + PNG 3D gerados no servidor**, numa página HTML sem nada a buscar (`ui://newera/plan-viewer.html`). O viewer WASM fica para quando os hosts aceitarem `wasm-unsafe-eval` (ext-apps #605) | Resolvido pelo E0-a: o sandbox recusa WebAssembly. A página funciona em qualquer host e é a mesma em todos os transportes. |
| D7 | Conta | **A conta é nossa**: tabela `accounts` com ID próprio no Postgres, identidade = e-mail verificado. O jeito de entrar (link por e-mail, Google, GitHub) pode mudar; o ID, não | Site, conector do Claude, ChatGPT e Codex usam a mesma conta. O pagamento se liga ao ID. |
| D8 | OAuth | **Servidor de autorização dentro do `newera-cloud`**, OAuth 2.1 + PKCE S256, registro de cliente por **CIMD e DCR**, `/.well-known/oauth-protected-resource` | O Claude aceita os dois registros e a OpenAI prefere CIMD. Os callbacks são específicos de cada cliente, então é preciso controlar o servidor. |
| D9 | Planos | **Cotas desde o primeiro dia**: toda conta tem um plano (`free` no começo), e as tools checam "tem cota?", nunca "é pagante?" | Ligar o Polar na Etapa 5 só muda qual plano a conta tem. Nenhuma tool é tocada. |
| D10 | Pagamento | **Polar.sh**, uma organização só, da doação à assinatura. O cliente no Polar se liga à conta pelo ID externo (`account.id`); o e-mail serve de reserva | Não cria dois caixas para migrar. A doação da Etapa 2 já cai onde a assinatura vai cair. |
| D11 | Armazenamento na nuvem | **Arquivo `.newera` inteiro no Postgres** (`bytea`), atrás de uma camada de armazenamento; R2 quando o volume pedir | O mesmo formato do desktop: baixar e abrir funciona sem conversão. O backup diário do Postgres já vai para o R2. |
| D12 | Plugin | **Uma pasta `plugin/` no formato Agent Plugins**, com skills compartilhadas por Claude Code, Codex e Cursor | Na Etapa 2 o MCP aponta para o local. Na Etapa 4 ganha o remoto: muda uma configuração, não código. |
| D13 | Artefato local | **Um `.mcpb` por plataforma**, gerado no `release.yml`, com o nome `newera-mcp-<plataforma>.mcpb` | O mesmo arquivo serve Claude Desktop, Registry, Smithery e Windows. O Registry exige "mcp" no nome. |
| D14 | Comportamento do stdio | **`newera mcp` se liga à janela aberta** (proxy para `127.0.0.1:7878`) **e, sem janela, roda headless** | Tem de valer antes de publicar o `.mcpb`. Mudar depois de listado mudaria o que os usuários já instalaram. |
| D16 | Superfície hospedada | **Mesmas tools, menos as que só fazem sentido na máquina do usuário**: `feedback` (manda dados aos desenvolvedores e é pedido por instrução — os diretórios recusam as duas coisas), `run_plugin`/`plugins` (rodam programas locais) e caminhos de arquivo livres (`open_home`, `save_home`, `export_*`, `set_background`, `edit_video render`), que na nuvem viram o projeto e os arquivos da própria conta | Um perfil de exposição no `newera-cloud`, não um fork: as tools continuam as do `newera-mcp` (D4). |
| D15 | Privacidade e termos | **Escritos já cobrindo o endgame**: contas, nuvem opcional, Polar como merchant of record, telemetria. Publicados em URLs fixas (`3dneweraai.com/privacy`, `/terms`) | Todo formulário pede essas URLs. Ter o texto final desde o início evita reenviar cadastros. |

## 4. Etapas

Regras das etapas:

- Cada etapa termina com algo que usuários reais usam.
- Tudo que uma etapa entrega é peça do endgame.
- **A única exceção é a Etapa 0.** Os spikes são experimentos num branch descartável.
  Nada deles é mergeado; o que fica é a resposta.

### Etapa 0: spikes que decidem o desenho (dias)

| Spike | Pergunta | Decide |
|---|---|---|
| **E0-a** | O `newera-web` (WASM) roda como MCP App dentro do Claude e do ChatGPT? Verificar CSP (`wasm-unsafe-eval`), tamanho do bundle, tempo de carga e tema claro/escuro. | D6: viewer WASM ou plano B |
| **E0-b** | OAuth com CIMD funciona ponta a ponta no Claude (conector custom) e no Developer Mode do ChatGPT, com um servidor mínimo? | Detalhes de D8 |
| **E0-c** | Quanto custa um `render_photo` na VPS (CPU e tempo)? Quantos cabem por hora? | Cotas do plano `free` (D9) e preço do cafezinho |

### Etapa 1: superfície única do MCP (em `newera-mcp`)

**Entregas, todas definitivas:**
- `title` e anotações corretas (`readOnlyHint`, `destructiveHint`, `openWorldHint`) em
  toda tool.
- Separar leitura de escrita onde uma tool mistura as duas.
- Erros que dizem o que fazer.
- Descrições sem ordens ao modelo e sem propaganda.
- Recurso `ui://` com a interface escolhida em E0-a, servido em qualquer transporte
  (stdio, HTTP local, relay, nuvem).
- `newera mcp` se liga à janela aberta (D14).
- Testes que travam o checklist R8 no CI. Assim uma tool nova sem anotação quebra o build.

**Destrava:** nada ainda, mas todas as etapas seguintes herdam isso.

### Etapa 2: distribuição local (sem servidor novo)

**Entregas:**
- Privacidade e termos publicados (D15).
- `.mcpb` por plataforma no `release.yml` (D13).
- `server.json` com o nome `com.3dneweraai/newera` (D1) e `packages` do tipo mcpb.
  - Login no Registry por DNS: `mcp-publisher login dns`, com a chave num secret.
  - Publicação num job do `release.yml`.
- Pasta `plugin/` (D12) com skills. Distribuída por:
  - marketplace próprio no repositório (Claude Code e Codex);
  - Console da Anthropic;
  - Cursor Marketplace.
- `gemini-extension.json` e o tópico `gemini-cli-extension`.
- `glama.json`, claim no Glama e PR no awesome-mcp-servers.
- Envio do `.mcpb` ao diretório de extensões do Claude Desktop.
- `smithery mcp publish`.
- Botões de instalar com um clique (Cursor, LM Studio, VS Code) no README e no site.
- WinGet Releaser e um repositório Homebrew próprio.
- Organização no Polar com o produto de apoio (D10); link no README e no site.

**Não fazer:**
- GitHub Sponsors ou Apoia.se: seria um segundo caixa.
- Namespace `io.github.*`: fere D1.
- Publicar o `.mcpb` antes da Etapa 1: fere D14.

### Etapa 3: `newera-cloud` com contas e a aba do usuário

**Entregas:**
- Crate e binário `newera-cloud` (D3), montando as rotas do `newera-relay` sem mudar o
  comportamento do link anônimo.
- Postgres com contas (D7) e planos/cotas (D9), todos no `free`.
- Servidor OAuth (D8) e `https://mcp.3dneweraai.com/mcp` (D2).
- O editor web ganha "entrar", e a aba logada se registra na conta.
- O roteador de sessão manda a chamada autenticada para a aba do usuário.
- Deploy trocando o binário do Dockerfile atual.

**Destrava:**
- Conector custom com URL fixa no Claude (link pré-preenchido) e no Developer Mode do
  ChatGPT. Sem colar uma URL nova a cada aba.

**Ainda não dá para ir aos diretórios:** os revisores precisam que o app funcione sem
nenhuma aba aberta. Isso vem na Etapa 4.

### Etapa 4: motor na nuvem e submissão aos diretórios

**Entregas:**
- Motor headless no `newera-cloud`: sem aba aberta, a chamada roda `newera_mcp::call`
  sobre um projeto carregado do R2 (D11).
- Fila para os trabalhos pesados (`render_photo`, vídeo) num worker separado, com cota
  (D9) e limite de CPU.
- Projetos na nuvem: criar, listar, abrir e salvar. "Abrir no editor" no site e baixar o
  `.newera`.
- Arquivos gerados (PNG, PDF, GLB) em URLs HTTPS temporárias, declaradas na CSP.
- O `server.json` ganha `remotes`, no mesmo registro de D1.
- O plugin ganha o MCP remoto com OAuth (D12).
- Checklist R8 completo; conta Claude **Team**; identidade verificada na OpenAI.
- Submissões:
  - Claude Connectors Directory, que também leva ao Claude Marketplace, seção "Add";
  - OpenAI Plugins Directory (ChatGPT e Codex).

### Etapa 5: cafezinho

**Entregas:**
- Produtos de assinatura no Polar (mensal e anual).
- Checkout no site.
- Webhook do Polar atualizando o plano da conta (D9, D10). **Nenhuma tool muda.**
- Conta de teste dos revisores com o plano pago.
- Quem doou na Etapa 2 com o mesmo e-mail ganha o benefício combinado.

### Etapa 6: enterprise e lojas (opcional)

- Microsoft Store com MSIX, que também leva ao Windows ODR.
- Docker MCP Catalog: o headless da Etapa 4 roda em container.
- Certificação no Copilot Studio.
- Gemini Enterprise.
- Claude Marketplace "Buy".
- Flathub, se a política de IA permitir.

## 5. Canais destravados por etapa

| Canal | Etapa | Como |
|---|---|---|
| Claude Desktop (diretório de extensões) | 2 | `.mcpb` |
| Official MCP Registry | 2 (local), 4 (remoto) | `server.json` D1 |
| VS Code / GitHub MCP Registry, Glama, PulseMCP, mcp.so, JetBrains, Zed | 2 | automático via Registry |
| Smithery | 2 | `smithery mcp publish` |
| Gemini CLI | 2 | `gemini-extension.json` |
| Claude Code (marketplace próprio e oficial) | 2 | plugin |
| Codex (marketplace próprio) | 2 | plugin |
| Cursor (deeplink e marketplace) | 2 | botão + plugin |
| LM Studio | 2 | deeplink |
| awesome-mcp-servers | 2 | PR |
| winget, Homebrew (tap próprio) | 2 | releaser/tap |
| Claude e ChatGPT como conector custom com URL fixa | 3 | `newera-cloud` + OAuth |
| **Claude Connectors Directory + Claude Marketplace** | 4 | submissão |
| **OpenAI Plugins Directory (ChatGPT + Codex)** | 4 | submissão |
| Microsoft Store / Windows ODR, Docker, Copilot Studio, Gemini Enterprise | 6 | ver R4 e R5 |

## 6. Monetização com Polar.sh

### 6.1 Modelo

**Open core com nuvem opcional.**

- **Grátis e open source para sempre:** o app desktop, o editor web, o MCP local, o link
  anônimo do relay e o uso da nuvem dentro da cota `free`.
- **Pago ("cafezinho"):** só o que custa servidor:
  - cota maior;
  - `render_photo` e vídeo na nuvem;
  - mais projetos e mais espaço guardados.
- **Sequência:** doação na Etapa 2 e assinatura na Etapa 5, as duas pelo Polar (D10).

### 6.2 Por que o Polar

- Feito para projetos open source, e o próprio Polar tem código aberto.
- É **merchant of record**: vende em nome do projeto e recolhe o imposto do mundo todo
  (VAT, sales tax).
- Faz o payout no Brasil, via Stripe Connect Express.
- Uma plataforma só para doação, assinatura, license keys e benefícios automáticos
  (acesso a repositório no GitHub, cargo no Discord).

| Plano Polar | Taxa por transação |
|---|---|
| Starter (grátis) | 5% + US$ 0,50 (+1,5% em cartão internacional) |
| Pro (US$ 20/mês) | 3,8% + US$ 0,40 |

Começar no Starter. O Pro só compensa quando a economia em taxas passar de US$ 20 por mês.

**Limitação:** o pagador **não tem Pix**, só cartão. Se isso afastar muitos
brasileiros, dá para somar Pix Automático por um PSP nacional só para o Brasil (exige
MEI/CNPJ e nota fiscal). Esse PSP alimentaria os mesmos planos de D9, sem tocar nas tools.

### 6.3 Integração

1. **Etapa 2 (doação):**
   - produto de apoio com preço livre, único ou mensal;
   - link no README, no site e no "Sobre" do app;
   - **nunca** dentro das tools do MCP (R6).
2. **Etapa 5 (assinatura):**
   - produtos mensal e anual;
   - checkout no site com a página do Polar;
   - o webhook do Polar (início, renovação, cancelamento) atualiza o plano da conta;
   - as tools só conferem a cota (D9).

**Preço:** a taxa fixa (US$ 0,50) come 25–50% de um plano de US$ 1–2. Mínimo de
~US$ 3–5 por mês, ou plano anual (uma taxa fixa por ano em vez de doze).

**Dentro dos diretórios:**
- nada de preço, botão de upgrade ou link de checkout no chat;
- nenhuma tool faz pagamento;
- a versão grátis precisa ser útil por si só.

Detalhes em R6.

## 7. Pendências e riscos

| Item | Situação | Onde se resolve |
|---|---|---|
| WASM dentro do iframe da MCP App | **Resolvido: não roda hoje** (ext-apps #605). Plano B em produção | E0-a |
| OAuth com CIMD nos dois clientes | Não testado | E0-b |
| Custo de CPU do render na nuvem | Não medido; define a cota grátis e o preço | E0-c |
| ID externo do cliente no Polar | Conferir na API ao integrar; o e-mail é o reserva (D10) | Etapa 2 |
| Preço livre no Polar para a doação | Conferir ao configurar | Etapa 2 |
| Pagador sem Pix no Polar | Medir desistências no checkout brasileiro | Etapa 5 |
| Plano Claude Team para submeter o conector | Custo recorrente | Etapa 4 |
| Developer Mode do ChatGPT para Plus/Pro | Fontes se contradizem | Etapa 3 (testar com conta real) |
| Países disponíveis no Plugins Directory | A doc não lista | Etapa 4 |
| Política de IA do Flathub (e do Snap) | Risco de rejeição | Etapa 6 |

## 8. Progresso

Marcado conforme cada item é implementado, testado e commitado na branch
`feat/distribution-endgame`. Itens que dependem de algo fora do código (pagamento,
documento de identidade, merge na `main`) dizem o que falta.

### Etapa 0
- [x] E0-a: viewer WASM como MCP App — **não roda hoje**: o sandbox das MCP Apps não
      permite `wasm-unsafe-eval` (spec `2026-01-26`; proposta aberta em ext-apps #605 /
      PR #667). Vale o plano B de D6: SVG da planta + PNG 3D do servidor. Testado no
      claude.ai como conector custom (relay local + túnel): planta com pan/zoom e 3D
      pedido pelo próprio widget.
- [ ] E0-b: OAuth CIMD/DCR no Claude e no ChatGPT
- [x] E0-c: custo do `render_photo` medido (Ryzen 5 3600, 640×480, CPU-segundos): draft
      3,7 · good 19 · best > 75; `render_3d` 0,14. Na nuvem: draft na cota grátis, good/best
      no pago, um render pesado por conta por vez e a fila limitada aos núcleos livres. A
      VPS não respondeu ao SSH (Cloudflare Access pede login), então as cotas ficam em
      configuração, não no código.

Achado no teste do E0-a: o formulário de conector custom do Claude já oferece "Entrar
agora", "Fazer login quando necessário" (grátis sem login, conta quando uma tool pedir) e
"Sem login", além de cabeçalhos fixos. O "quando necessário" é o freemium de D9 pronto no
cliente.

### Etapa 1
- [x] `title` e anotações em todas as tools (`crates/newera-mcp/src/hints.rs`)
- [x] Leitura separada de escrita: 59 tools; toda leitura tem nome próprio (`cameras`,
      `levels`, `check_layout`, `ergonomics`, `electrical`, `lighting`…) e as mudanças
      ficaram em `edit_*`, `accept`, `fill_lighting`, `trace_walls`, `export_cut_list`,
      `run_plugin`, `checkpoint`; as pessoas da revisão vão em `set_home(people=…)`
- [x] Erros acionáveis e descrições sem ordens ao modelo (a instrução de `feedback` sai
      só na superfície hospedada, D16)
- [x] Recurso `ui://` (MCP App) em todos os transportes: `show_plan` +
      `ui://newera/plan-viewer.html`, servido pelo handshake e pelo relay
- [x] `newera mcp` ligado à janela aberta (D14)
- [x] Testes do checklist R8 no CI: `every_tool_has_hints`, `reads_change_nothing`,
      `a_read_refuses_a_write_argument`, `a_write_tool_points_reads_elsewhere` e o smoke

### Etapa 2
- [x] Privacidade e termos (D15): `site/privacy/`, `site/terms/`, no rodapé e no sitemap
- [x] `.mcpb` universal no `release.yml` (`scripts/mcpb.sh`, job `mcpb`): testado
      desempacotando e rodando como o Claude Desktop roda
- [x] `server.json` (`com.3dneweraai/newera`, validado no registry) e job `registry`
      (`scripts/registry-publish.sh`, testado em simulação)
- [x] Pasta `plugin/` + marketplaces: instalado num perfil vazio do Claude Code (skills
      carregam, MCP conecta) e do Codex (plugin e MCP instalados); manifest do Cursor
- [x] Extensão do Gemini CLI (`gemini-extension.json`)
- [x] `glama.json`
- [x] Botões de instalar com um clique (README e site)
- [x] winget e Homebrew: manifests validados (schema winget 1.10, `brew style`) e jobs de
      release

**Falta, e depende de você** (conta, pagamento ou publicação em nome do projeto):
- [ ] Mesclar a branch e lançar uma versão (o job `mcpb` só roda num build de release).
- [ ] `scripts/registry-key.sh`: cria a chave do registry e o secret; commitar o
      `site/.well-known/mcp-registry-auth` que ele escreve.
- [ ] Enviar o `.mcpb` no formulário do Claude Desktop (https://clau.de/desktop-extention-submission).
- [ ] Enviar o plugin no Console da Anthropic (https://platform.claude.com/plugins/submit) e
      no Cursor Marketplace (https://cursor.com/marketplace/publish).
- [ ] Tópicos do repositório: `mcp`, `mcp-server`, `gemini-cli-extension`.
- [ ] Glama: "Claim" em https://glama.ai/mcp/servers; depois o PR no awesome-mcp-servers.
- [ ] Smithery: `npx @smithery/cli mcp publish newera-mcp.mcpb -n leandrodaf/3d-new-era-ai`.
- [ ] mcp.so: formulário em https://mcp.so/submit.
- [ ] winget: fork de microsoft/winget-pkgs, PR com `packaging/winget/manifests/…`, e o
      secret `WINGET_TOKEN`.
- [ ] Homebrew: criar `leandrodaf/homebrew-tap` com `packaging/homebrew/Casks/…` e o
      secret `HOMEBREW_TAP_TOKEN`.
- [ ] Polar: criar a organização e o produto de apoio com preço livre; pôr o link no
      README e no site.
- [ ] E-mails `privacidade@` e `contato@3dneweraai.com` (Cloudflare Email Routing), citados
      nas páginas legais.

### Etapa 3
- [x] `newera-cloud` montando o relay (`crates/newera-cloud`; o relay ganhou salas com dono)
- [x] Postgres: contas, sessões, planos com cotas (`free`, `supporter`), assinaturas e
      uso — migração `0001`, segredos guardados só como hash
- [x] Servidor OAuth 2.1 (metadados RFC 9728/8414, registro dinâmico, client metadata
      documents com proteção contra SSRF, PKCE S256, rotação de refresh com detecção de
      reuso) e `/mcp` fixo com Bearer
- [x] Entrar por link no e-mail (Resend) ou Google; página da conta, sair e apagar
      (tudo some em 30 dias)
- [x] Editor web: "Entrar para usar no Claude e no ChatGPT" no painel de IA, a aba
      reivindica a sala para a conta e mostra o endereço fixo
- [x] Layout do painel de IA corrigido: o bloco da conta estava espremido na linha do
      título ("No ar…" / "Desligar") e agora tem a sua própria caixa, com o endereço fixo
      no mesmo estilo do endereço da aba
- [x] A ligação da aba volta sozinha quando o serviço reinicia (a cada deploy), no mesmo
      endereço, e não fica mais presa em "Ligando…"
- [x] Testes: fluxo inteiro contra Postgres (`crates/newera-cloud/tests/flow.rs`, job
      novo no CI) e no navegador (entrar, "Pronto", reivindicar, reiniciar o serviço)
- [x] Roda local por inteiro: `docker compose -f crates/newera-cloud/local/docker-compose.yml up --build`
      (serviço + Postgres próprio; links de entrada no log). A imagem foi testada saudável.
- [x] **VPS preparada** (com o `hospedar-app`, sem edição à mão): o `newera-cloud`
      assume o lugar do relay em `/opt/newera-relay`, no mesmo Postgres (`newera_relay`),
      com backup, túnel e domínio que já existiam; 1 GB e 1 CPU (novo `APP_CPUS` no
      template do agendo-certo-internal), saúde em `/cloud/health` (503 sem banco),
      imagem com `rust:1.95-slim-bookworm`. Conferido na VPS: compose válido, a senha do
      banco continua a mesma, o relay no ar.
- [ ] **Deploy**: mesclar na `main` e criar a tag — o CI testa (relay + cloud contra
      Postgres), publica a imagem e troca o container. Depois, `backup-agora --app
      newera-relay` para confirmar o primeiro dump no R2 (até hoje o banco estava vazio
      e o backup recusava o dump vazio, como deve).

### Etapa 4
- [x] Motor headless: sem aba aberta, as chamadas rodam no projeto ativo da conta
      (`crates/newera-cloud/src/engine.rs`). **D11 mudou**: o `.newera` fica no Postgres
      (`bytea`), que o backup diário já leva ao R2 — projetos pesam de 4 a 420 KB; trocar
      por objetos no R2 é trocar a implementação de armazenamento, não refazer.
- [x] Jaula de leitura de disco no `newera-core` (`vfs::jail`): um projeto não lê
      arquivos de outro nem do servidor
- [x] Fila de trabalhos pesados com cota: uma renderização pesada por conta, no máximo
      núcleos−1 ao mesmo tempo; fotos `draft` contam por dia, `good`/`best` por mês
- [x] Projetos na nuvem: `projects`, `new_home`/`open_home`/`save_home` por nome, lista
      e download na página da conta, "Abrir no editor" abrindo o projeto da nuvem no
      editor web (com o cookie)
- [x] Exportações viram links de 24 h (`/files/…`), nunca gravadas no caminho pedido
- [x] `remotes` no `server.json` (o script só publica a URL remota quando ela estiver no ar)
- [x] Plugin com o MCP remoto: `plugin/` (`3d-new-era-ai`) aponta para
      `https://mcp.3dneweraai.com/mcp` com OAuth; `plugin-desktop/` (`3d-new-era-ai-desktop`)
      fica com o app local, as mesmas skills por link simbólico (a instalação copia)
- [ ] Submissões ao Claude Connectors Directory e ao OpenAI Plugins Directory: dependem
      da produção no ar, do plano Claude Team e da identidade verificada na OpenAI

### Etapa 5
- [x] Webhook do Polar (`/billing/polar`, `crates/newera-cloud/src/billing.rs`): assinatura
      Standard Webhooks verificada (segredos `whsec_` e os antigos), plano da conta segue a
      assinatura (ativa → `supporter`, revogada → `free`), conta criada pelo e-mail se a
      pessoa pagou antes de entrar
- [x] Botão "Apoiar com um cafezinho (plano pago)" só na página da conta, com e-mail e id
      da conta no link de checkout — nada é vendido dentro do chat
- [x] Nenhuma tool mudou: elas leem as cotas do plano (D9)
- [x] No Polar: organização `3d-new-era-ai`, "Supporter (monthly)" US$ 5/mês e
      "Supporter (yearly)" US$ 48/ano, um link de checkout com os dois e o webhook
      `newera-cloud` (eventos `subscription.*`, API 2026-04) para
      `https://mcp.3dneweraai.com/billing/polar`
- [x] `POLAR_WEBHOOK_SECRET`, `NEWERA_POLAR_CHECKOUT_URL` e `NEWERA_POLAR_PLANS` no
      ambiente do serviço

### Etapa 6
- [ ] Microsoft Store/MSIX, Windows ODR, Docker MCP Catalog, Copilot Studio, Gemini
      Enterprise, Flathub: opcionais; todos são cadastros ou lojas em nome do projeto
      (e alguns exigem a produção no ar). Ficam para depois das etapas 2 a 5 publicadas.

### O que falta

Feito em 24/09/2026: a branch `feat/distribution-endgame` foi mesclada na `main` e a
versão **1.8.0** saiu (tag `v1.8.0`): release no GitHub com o `newera-mcp.mcpb`, deploy
do `newera-cloud` na VPS pelo CI e o site com as páginas legais.

Quem faz cada passo: **você** (conta, pagamento, identidade, decisão) ou **eu, com a
sua confirmação a cada envio** (formulário, publicação, configuração em painel).

**1. Produção**
- [x] Conferir o deploy da 1.8.0 (`/cloud/health` 200 na VPS) — eu
- [x] Primeiro backup do banco no R2: `backup-agora --app newera-relay` (verificado,
      `daily/newera_relay-2026-09-24T17-56-40Z.sql.gz`) — eu
- [x] Resend: domínio `3dneweraai.com` (São Paulo) com DKIM e os CNAME `rsend`/`send` no
      Cloudflare; chave "Sending access" no 1Password ("3D New Era AI - Resend") e no
      `app.env` da VPS (24/09/2026)
- [x] Resend verificou o domínio; a entrada por e-mail foi entregue e usada (24/09/2026)
- [ ] Google (opcional): cliente OAuth "Web application" com redirect
      `https://mcp.3dneweraai.com/login/google/callback` — **você**
- [x] E-mails `privacidade@` e `contato@3dneweraai.com` (Cloudflare Email Routing ligado,
      MX e SPF do Cloudflare, as duas regras encaminham para leandro.daf4@gmail.com) — eu

**2. Pagamento (Polar)**
- [ ] Conta e dados de recebimento — **você**
- [x] Produtos no painel — eu:
      - "Supporter (monthly)", US$ 5/mês: `afb196b2-844f-46c3-899a-b2bf9df7a68b`
      - "Supporter (yearly)", US$ 48/ano: `593fc418-b65a-410d-861b-20c59da0d419`
- [x] Link de checkout "Supporter (account page)" com os dois produtos, volta para
      `https://mcp.3dneweraai.com/account`:
      `https://buy.polar.sh/polar_cl_fXCqiyLmYzA1cWpJbo6wRKH6hZtqwmshPeNAD2TfpP1` — eu
- [x] Webhook `newera-cloud` para `https://mcp.3dneweraai.com/billing/polar`, formato
      Raw, API 2026-04, os 11 eventos `subscription.*` — eu
- [x] `POLAR_WEBHOOK_SECRET` (o *Signing secret* do webhook), `NEWERA_POLAR_CHECKOUT_URL`
      e `NEWERA_POLAR_PLANS` no `app.env` da VPS e o container recriado (24/09/2026):
      `POST /billing/polar` sem assinatura responde 401 — você, com o script sem eco
- [x] Link de apoio no README ("Support the project") e no rodapé do site ("Apoiar ☕") — eu

**3. Registries e diretórios (Etapa 2)**
- [x] `scripts/registry-key.sh` e commit do `site/.well-known/mcp-registry-auth`;
      `com.3dneweraai/newera` 1.8.0 publicado no MCP Registry oficial — eu
- [x] Formulário do Claude Desktop com o `.mcpb` 1.8.0 enviado em 24/09/2026
      (https://clau.de/desktop-extention-submission); a Anthropic só responde se selecionar
- [x] Cursor Marketplace: pedido de publisher enviado em 24/09/2026 (`@3d-new-era-ai`,
      `.cursor-plugin/marketplace.json` na raiz apontando para `./plugin`,
      logo em `assets/logo-plate.png`) — eu
- [x] Plugin no Console da Anthropic enviado em 24/09/2026 ("3d-new-era-ai", caminho
      `plugin`, aguardando revisão em https://platform.claude.com/plugins/submissions)
- [x] Tópicos do repositório: `mcp`, `mcp-server`, `gemini-cli-extension` (no lugar de
      `desktop-app`: o GitHub aceita 20) — eu
- [x] mcp.so: https://github.com/chatmcp/mcpso/issues/4361 — eu
- [x] PR no awesome-mcp-servers: https://github.com/punkpeye/awesome-mcp-servers/pull/15045 — eu
- [x] Smithery: `leandro-daf4/new-era-3d` publicado (https://smithery.ai/servers/leandro-daf4/new-era-3d),
      apontando para `https://mcp.3dneweraai.com/mcp`; o OAuth com CIMD passou de ponta a
      ponta e o Smithery leu 57 tools e 1 resource — eu
- [x] Glama: enviados para revisão em 24/09/2026 o servidor pelo código-fonte (GitHub) e o
      conector hospedado (`https://mcp.3dneweraai.com/mcp`); o Glama manda por e-mail as
      instruções dos checks (Dockerfile e health check) quando aprovar
- [x] Secret `WINGET_TOKEN` (classic, `public_repo`, vence em 24/09/2027), também no
      1Password
- [x] winget: primeiro PR aberto em 24/09/2026, https://github.com/microsoft/winget-pkgs/pull/440827;
      falta **você** assinar o CLA da Microsoft (o bot pede no PR) e os revisores aprovarem;
      as versões seguintes o release envia sozinho
- [x] Homebrew: repositório `leandrodaf/homebrew-tap` com o cask da 1.8.0
      (`brew install --cask leandrodaf/tap/3d-new-era-ai`) — eu
- [x] Secret `HOMEBREW_TAP_TOKEN` (fine-grained, só no tap, Contents: write, sem validade),
      também no 1Password: as próximas versões atualizam o cask sozinhas

**4. Diretórios de conector (depois de 1 e 2)**
- [x] Plugin com o MCP remoto (`https://mcp.3dneweraai.com/mcp`) e o plugin do app
      (`3d-new-era-ai-desktop`) no mesmo marketplace — eu
- [ ] ~~Claude Connectors Directory~~ — fora por decisão (24/09/2026): exige o plano Claude
      Team (mínimo 2 assentos, R$ 276/mês). O conector segue usável no Claude como conector
      personalizado, pelo endereço, e o plugin segue na revisão do Console
- [ ] OpenAI Plugins Directory — exige identidade verificada na OpenAI (**você**)
- [ ] Conta de teste com plano pago para os revisores — eu

---

# Parte II: referência

## R1. Anthropic (Claude)

Nenhum canal da Anthropic cobra taxa de listagem nem comissão.

### R1.1 Connectors Directory (conector MCP remoto)

- **O que é:** catálogo único de conectores do Claude.ai, Desktop, mobile, Claude Code e
  Cowork. A ordem é por uso. Conectores listados podem ser sugeridos pelo próprio Claude
  no chat ("Suggested Connectors"); conectores custom nunca são.
- **Requisitos técnicos:**
  - URL `https://` com streamable HTTP (o portal ainda aceita SSE).
  - Toda tool com `title` e `readOnlyHint` ou `destructiveHint`.
  - Nome de tool com até 64 caracteres; tools de leitura separadas das de escrita.
  - Erros dizem o que fazer; erro genérico reprova.
  - Descrições de tools não dão ordens ao Claude nem promovem produtos.
- **Tipos de login:** `oauth_dcr` e `oauth_cimd` (funcionam direto), `none` (sem login),
  `static_headers` (beta), `oauth_anthropic_creds` e `custom_connection` (via
  mcp-review@anthropic.com). Callback: `https://claude.ai/api/mcp/auth_callback`, com
  PKCE S256. O Claude Code usa callback em loopback.
- **Formas de URL:** "Universal URL" (uma para todos), "Multiple URLs" (lista fixa) ou
  "URL pattern" (regex; cada usuário cola a sua). As duas últimas têm revisão mais lenta.
  Dá, em teoria, para enviar o relay atual como "URL pattern" com `none`, mas a
  experiência seria ruim e a doc desaconselha credencial na URL.
- **Submissão:** portal
  <https://claude.ai/admin-settings/directory/submissions/new>.
  - Exige organização **Team ou Enterprise**; plano individual não consegue enviar.
  - Pede URL de documentação, política de privacidade, ícone, contato de suporte e uma
    conta de teste "fully populated".
  - Sete declarações de compliance obrigatórias.
  - Scan automático → entra como "Community" → a Anthropic pode promover a "Verified"
    (teste funcional de cada tool), sem pedido.
  - Sem prazo definido; escalonamento por mcp-review@anthropic.com.
- **Link de instalação sem estar no diretório** (serve hoje para o relay e, na Etapa 3, para a URL fixa):
  `https://claude.ai/customize/connectors?modal=add-custom-connector&connectorName=NAME&connectorUrl=ENCODED_URL`
- **Política de imagens:** conectores que "geram imagens/vídeo/áudio via modelos de IA"
  são proibidos, com exceção para ferramentas de design (diagramas, mockups, design
  assets). O `render_photo` é path tracing determinístico, sem modelo generativo, então
  está dentro. Deixar isso explícito na submissão.

### R1.2 Desktop Extensions / MCP Bundles (`.mcpb`, antigo `.dxt`)

- **O que é:** um zip com o servidor MCP local (stdio) e um `manifest.json`. Instala com
  um clique no Claude Desktop (duplo clique, arrastar, ou Settings → Extensions). O
  manifest pode gerar uma tela de configuração (`user_config`). Sem OAuth, funciona
  offline.
- **Ferramenta:** `npm i -g @anthropic-ai/mcpb`, depois `mcpb init` e `mcpb pack`. A spec
  aceita servidor binário (Node é só recomendado).
- **Plataformas:** o Claude Desktop só existe para macOS e Windows.
- **Para o diretório:**
  - `manifest_version` 0.2 ou mais nova;
  - array `privacy_policies` com URLs HTTPS;
  - seção "Privacy Policy" no README;
  - pelo menos 3 prompts de exemplo.

  As cláusulas de open source e "spec will evolve" dos Terms não podem ser dispensadas.
- **Submissão:** formulário <https://clau.de/desktop-extention-submission>. Não exige
  Team/Enterprise. Os aprovados aparecem no mesmo Connectors Directory.
- **Limite:** a Anthropic chama esse canal de "secondary distribution path". É o mais
  rápido para o binário, não o de maior alcance.
- MCP Apps também renderizam a partir de servidores stdio locais no Claude Desktop.

### R1.3 MCP Apps (UI interativa dentro do chat)

- **O que é:** primeira extensão oficial do MCP (26/01/2026). Uma tool devolve uma UI
  HTML que roda num iframe sandbox dentro da conversa.
- **Técnico:**
  - Helpers `registerAppTool` e `registerAppResource`; no cliente, `App.connect()`.
  - Negociar a capability `io.modelcontextprotocol/ui`.
  - A CSP bloqueia tudo por padrão: embutir os assets ou declarar domínios em
    `_meta.ui.csp`.
  - No Claude, a UI roda em `<sha256(url)[:32]>.claudemcpcontent.com`.
  - Há exemplos oficiais com three.js, CesiumJS e shaders.
  - **Não verificado:** se a CSP permite `wasm-unsafe-eval` para carregar o
    `newera_web.wasm`.
- **Clientes com suporte:**
  - Claude web e Desktop, Cowork, mobile;
  - ChatGPT (há guia de migração do Apps SDK);
  - VS Code Copilot, M365 Copilot, Cursor;
  - Goose, Postman, MCPJam, Archestra, PostHog Code.

  Ainda sem suporte (ago/2026): JetBrains, Kiro, Antigravity.
- **Exigências no diretório:**
  - 3 a 5 screenshots PNG com pelo menos 1000px, recortadas sem o prompt;
  - layout responsivo de 320px até tela cheia;
  - tema claro e escuro;
  - sem scroll horizontal ou aninhado.

  Há template no Figma.
- **Encaixe:** muito alto. Uma planta ou vista 3D interativa no chat é exatamente o caso
  de uso, e acaba com a necessidade da aba aberta.

### R1.4 Plugins (Claude Code e Cowork), marketplaces e Agent Skills

- **O que é:** o marketplace oficial `claude-plugins-official` aparece para todo usuário
  do Claude Code e no Cowork. Um plugin leva skills, slash commands, subagentes e
  servidores MCP (remotos, locais ou `.mcpb`). Skills sozinhas **não** são aceitas; vão
  dentro de um plugin.
- **Submissão:**
  - Repositório público no GitHub que passe em `claude plugin validate`.
  - Formulários:
    - <https://platform.claude.com/plugins/submit> (Console; autor individual, grátis);
    - <https://claude.ai/admin-settings/directory/submissions/plugins/new> (Team/Enterprise).
  - Entra como "Community" e pode virar "Anthropic Verified".
  - Atualizações chegam via push no GitHub, com checagem automática.
  - Espelho comunitário: <https://github.com/anthropics/claude-plugins-community>.
- **Marketplace próprio:** `.claude-plugin/marketplace.json` no repositório; o usuário roda
  `/plugin marketplace add leandrodaf/3d-new-era-ai`. Grátis e imediato.
- **Formato recomendado pela Anthropic:** MCP remoto com OAuth + plugin com skills.

### R1.5 Novidades de 2026

- **Claude Marketplace** (relançado em 23/09/2026):
  - **"Add":** mais de 2.000 conectores e plugins vindos do diretório. É a porta do 3D New Era.
  - **"Buy":** produtos que empresas pagam com o crédito já contratado com a Anthropic,
    sem comissão; entrada por candidatura em <https://claude.com/marketplace-partners>.
    Voltado a enterprise.
  - **"Scale":** consultorias.
- **Claude Partner Network** (mar/2026): fundo de US$ 100M, focado em serviços e
  consultorias. Pouco relevante aqui.
- **Políticas:** em abr/2026 a política de MCP virou a "Software Directory Policy"; os
  Terms foram atualizados em 16/03/2026.
- **Official MCP Registry não faz o servidor aparecer no Claude.** Vale pelos outros
  clientes (R3).

---

## R2. OpenAI (ChatGPT e Codex)

### R2.1 O que mudou

- **"Apps" agora são "Plugins".** Em 09/07/2026 o App Directory (dez/2025) e o de
  plugins viraram um **Plugins Directory único para ChatGPT e Codex**: publica uma vez,
  aparece nos dois. Um plugin empacota skills, apps (servidor MCP com ou sem UI), hooks e
  extensões de navegador.
- **O Apps SDK continua a base** (MCP + UI opcional em iframe). O ChatGPT segue a
  especificação **MCP Apps** desde 22/02/2026: um widget feito no padrão (`_meta.ui`)
  roda no ChatGPT e no Claude. A OpenAI recomenda começar pelo padrão MCP Apps.
- **Alcance:** ~900 milhões de usuários ativos por semana (ago/2026, número de terceiros).
  Não há número público de plugins no diretório.

### R2.2 Submissão ao Plugins Directory

- **Quem pode:** organização na OpenAI Platform com **identidade verificada** (pessoa ou
  empresa) e permissão "Apps Management". Nome, site, suporte, política de privacidade e
  termos precisam bater com a identidade.
- **Técnico:**
  - MCP num **domínio HTTPS público** (local ou de teste não é aceito);
  - verificação de domínio por endpoint well-known;
  - **CSP** com os domínios exatos que o widget acessa (`_meta.ui.csp`; iframes só do
    próprio domínio);
  - anotações corretas em toda tool (`readOnlyHint`, `destructiveHint`,
    `openWorldHint`). Anotação errada é causa comum de rejeição.
- **Tipos de submissão:** só skills, MCP remoto, ou os dois.
- **O que enviar:**
  - ícone, nome, descrições, screenshots, prompts iniciais;
  - **pelo menos 5 testes positivos e 3 negativos**;
  - países ou regiões de disponibilidade.
- **Revisão:** prazo variável. Depois de aprovado, o dev decide quando publicar. Mudanças
  no metadata das tools são captadas por varredura periódica. Mudanças na listagem ou nas
  skills exigem nova versão e nova revisão.
- **Política:**
  - público de 13 anos para cima;
  - dados mínimos; não devolver IDs internos, timestamps ou IDs de sessão;
  - não pedir histórico da conversa;
  - **não aceita versões de teste ou demo**;
  - se houver login, conta sem MFA para os revisores.

### R2.3 Developer Mode (o usuário adiciona o MCP à mão)

- A doc oficial lista Pro, Plus, Business, Enterprise e Education, **só na web**. MCP
  completo (escrita pede confirmação), transportes SSE e streamable HTTP, **sem stdio**.
  Para ligar: Settings → Security and login → Developer mode, depois Plugins → "+".
- **Ressalva:** fontes de terceiros (set/2026) dizem que Plus/Pro teriam só leitura ou
  nem teriam a opção; há relatos no fórum de contas Pro sem ela. **Não contar com isso
  para todo usuário pago.** No plano Free não funciona.
- É onde a URL do relay já funciona hoje.
- **Secure MCP Tunnel (novo):** cliente de túnel da OpenAI, só com conexão de saída, que
  liga um MCP em localhost ou rede privada ao ChatGPT, Codex e Responses API. É voltado
  a empresas (tenant, config de admin).

### R2.4 Codex

- Marketplace de plugins desde mar/abr 2026 (mais de 90 plugins em abril).
- **Funciona com o binário local hoje, sem aprovação:**
  - um plugin (`plugin.json`, `.mcp.json`, `skills/`) pode declarar MCP stdio:
    `{"mcpServers":{"newera":{"command":"newera","args":["mcp"]}}}`;
  - marketplace no próprio repositório (`.agents/plugins/marketplace.json`), instalado
    por `/plugins` ou `codex plugin marketplace add`;
  - o caminho mais simples é `codex mcp add newera --url http://127.0.0.1:7878/mcp`
    (já está no README).
- O diretório público é o mesmo do ChatGPT (R2.2). Para MCP local a doc diz: "deploy it
  to a public HTTPS URL. If you can't, reach out to your OpenAI contact for local MCP
  support."
- O formato plugin/skills segue o padrão "Agent Plugins", portátil entre Codex, Claude
  Code e Cursor. **O mesmo plugin pode servir aos três.**

### R2.5 GPT Store

Em modo de manutenção. Os Custom GPTs estão sendo substituídos por Workspace Agents e
plugins, com sunset em ago/2026 para Business/Enterprise. **Não investir.**

---

## R3. Registries e diretórios abertos

### R3.1 Official MCP Registry (prioridade máxima)

- **Onde:** <https://registry.modelcontextprotocol.io>; docs em
  <https://github.com/modelcontextprotocol/registry/tree/main/docs>.
- **Tipos de pacote aceitos:** npm, PyPI, NuGet, **Cargo**, OCI e **MCPB**.
- **Binário via MCPB:**
  - `"registryType": "mcpb"`;
  - `identifier` = URL do asset no GitHub Releases, **que precisa conter "mcp"**;
  - `fileSha256` obrigatório;
  - `transport: stdio`.
- **Alternativa Cargo:** `registryType: "cargo"` com a linha
  `mcp-name: com.3dneweraai/newera` visível no README do crates.io (comentário
  HTML não vale).
- **`remotes`** (streamable-http) exige URL pública e fixa. Entra na Etapa 4, no mesmo registro (D1).
- **Namespace** (escolhido: `com.3dneweraai/newera`, ver D1):
  - `io.github.leandrodaf/<nome>` via `mcp-publisher login github`, ou GitHub OIDC no
    Actions;
  - ou `com.3dneweraai/<nome>` via DNS TXT no domínio
    (`mcp-publisher login dns --domain 3dneweraai.com --private-key ...`), ou arquivo em
    `/.well-known/mcp-registry-auth`.
- **Passos:**
  1. Gerar o `.mcpb` no `release.yml`.
  2. `mcp-publisher init` e editar o `server.json` (name, version, description,
     repository, websiteUrl, packages com mcpb + sha256).
  3. `mcp-publisher login ...` e `mcp-publisher publish`.
  4. Colocar num job do `release.yml` com OIDC.
- **Quem puxa daqui automaticamente:**
  - GitHub MCP Registry e galeria do VS Code (`@mcp` na aba Extensions);
  - PulseMCP (ingestão semanal; submissão direta está pausada);
  - Glama;
  - Smithery (parcialmente);
  - mcp.so;
  - plugin "MCP Servers for AI Assistants" da JetBrains;
  - Zed, que vai trocar as extensões de MCP pelo registry oficial.

### R3.2 MCPB

Spec em <https://github.com/modelcontextprotocol/mcpb>. O mesmo artefato serve ao
Registry, ao Smithery, ao Windows ODR e ao Claude Desktop. **Fazer uma vez só.**

### R3.3 Gemini CLI

- Galeria <https://geminicli.com/extensions>, **automática**: repo público + tópico
  `gemini-cli-extension` + `gemini-extension.json` na raiz. O crawler roda diariamente.
- Aceita `mcpServers` com `command` (`newera mcp`) ou `httpUrl`
  (`http://127.0.0.1:7878/mcp`).
- Binários por plataforma nos assets: `darwin.arm64.<nome>.tar.gz`,
  `linux.x64.<nome>.tar.gz`, `win32.<nome>.zip`, com o manifest na raiz do arquivo.
- Pode ficar num repo separado (ex.: `newera-gemini`) para não mexer nos assets atuais.
- O app Gemini de consumidor não tem diretório de MCP aberto a terceiros; o canal Google
  realista é o Gemini CLI.

### R3.4 Smithery

<https://smithery.ai/docs/build/publish>. Para servidor stdio:
`smithery mcp publish ./server.mcpb -n leandrodaf/3d-new-era-ai`. Grátis.

### R3.5 Glama e awesome-mcp-servers

- **Glama** já indexa pelo GitHub e pelo registry. Para reivindicar o listing, um
  `glama.json` na raiz:
  `{"$schema":"https://glama.ai/mcp/schemas/server.json","maintainers":["leandrodaf"]}`,
  depois "Claim".
- **awesome-mcp-servers** (<https://github.com/punkpeye/awesome-mcp-servers>): PR com
  entrada em ordem alfabética na categoria Art & Culture / Design, com o badge do Glama
  (na prática, pré-requisito). Ler o CONTRIBUTING.md.
- Adicionar os tópicos `mcp-server` e `mcp` no repositório.

### R3.6 mcp.so

<https://mcp.so/submit>: formulário manual para repo público. Em geral também puxa do
registry.

### R3.7 Botões de instalar com um clique

Sem aprovação; basta colocar no README e no site.

| Cliente | Link |
|---|---|
| Cursor | `cursor://anysphere.cursor-deeplink/mcp/install?name=newera&config=<base64 JSON>` |
| LM Studio | `lmstudio://add_mcp?name=newera&config=<base64>` |
| VS Code | `vscode:mcp/install?...` (a galeria já vem via registry) |
| Claude | link de conector custom da R1.1 |

### R3.8 Cursor Marketplace

<https://cursor.com/marketplace/publish>. `.cursor-plugin/plugin.json` num repo público,
com revisão manual. Template: <https://github.com/cursor/plugin-template>. O
cursor.directory é comunitário.

### R3.9 Docker MCP Catalog

<https://github.com/docker/mcp-registry>. PR com metadados; o Docker compila, assina e
publica `mcp/<nome>`, e ele aparece no MCP Toolkit do Docker Desktop em ~24h. **Só faz
sentido quando `newera mcp` rodar headless num container.** Hoje o benefício é baixo.

### R3.10 LM Studio, Zed, JetBrains

- **LM Studio:** deeplink (R3.7).
- **Zed:** vai adotar o registry oficial.
- **JetBrains:** o AI Assistant aceita servidor local via JSON; a descoberta acontece
  pelo plugin "MCP Servers for AI Assistants", que lê o registry. Nada extra a fazer.

---

## R4. Instalação do binário

| Canal | Custo | Observações |
|---|---|---|
| **winget** | Grátis | O zip portátil atual serve (`InstallerType: zip` + `NestedInstallerType: portable`). Automatizar com a action **WinGet Releaser** (Komac). **Melhor custo/benefício.** |
| **Homebrew (tap próprio)** | Grátis | `leandrodaf/homebrew-tap` → `brew install leandrodaf/tap/newera`. Sem exigências. |
| **Homebrew (tap oficial)** | US$ 99/ano (Apple) | Desde set/2026 exige app assinado com Developer ID e **notarizado**; casks que falham no Gatekeeper estão sendo desativados. Também há critério de notoriedade. |
| **Microsoft Store** | Grátis (desde mai/2026) | Aceita Win32 empacotado ou não. Com **MSIX**, também registra o MCP no Windows ODR. |
| **Windows On-device Agent Registry (ODR)** | — | Em preview. Registro via MSIX ou `.mcpb` no instalador. Exige `_meta.com.microsoft.windows.static_responses` idêntico a `initialize`/`tools/list`. Servidores via MCPB **não** ficam acessíveis por padrão; o caminho real é MSIX + package identity. |
| **Snap Store** | Grátis | Mais simples que o Flathub; política de IA não verificada. |
| **Flathub** | Alto | Build offline (`cargo-sources.json`), MetaInfo, verificação de domínio. **Política atual:** é obrigatório declarar código gerado ou assistido por IA, e o manifesto não pode ter conteúdo gerado por IA; há risco de rejeição. Deixar para depois. |

## R5. Canais enterprise

Exigem MCP remoto público e estável. Ficam para a Etapa 6, depois do motor na nuvem da Etapa 4.

- **Microsoft 365 Copilot / Copilot Studio:** MCP é GA desde jul/2026. Para aparecer em
  todos os tenants é preciso **certificação**: um conector Power Platform enviado pelo
  Partner Center, oferta "Apps and Agents for M365 and Copilot".
- **Gemini Enterprise Agent Gallery / Google Cloud Marketplace:** voltado a parceiros e
  agentes hospedados, com fluxo de vendedor no GCP Marketplace.
- **Claude Marketplace "Buy":** candidatura como parceiro, voltado a enterprise (R1.5).

---

## R6. Cobrança dentro dos diretórios

| | Claude | ChatGPT/Codex |
|---|---|---|
| Plano pago com login OAuth | Permitido | Permitido ("sign in to an existing paid account") |
| Vender ou assinar dentro do chat | Proibido: nenhuma tool pode fazer transação financeira | Proibido: bens digitais, assinaturas, créditos |
| Mostrar planos, preço ou upgrade | Proibido nas descrições das tools (conta como prompt injection) | Proibido: "must not display subscription plans... or promote upgrades" |
| Link para checkout | Evitar; erro factual com URL é o limite razoável | Proibido linkar checkout ou página de upgrade |
| Comissão da plataforma | Nenhuma | Nenhuma (e nenhum repasse) |
| Checkout próprio | No site, via Polar.sh | No site, via Polar.sh. O Instant Checkout foi aposentado em mar/2026, e o ACP ficou para bens físicos |

**Freemium no ChatGPT:** declarar `securitySchemes` com **`noauth` e `oauth2` juntos**
por tool. As tools grátis funcionam anonimamente; as pagas pedem a conta vinculada.
A tela de login aparece quando a tool devolve erro com `_meta["mcp/www_authenticate"]`.
Refresh token com `offline_access`. Os revisores precisam de uma conta completa sem MFA.

**No Claude:** grátis com `none` (ou OAuth com cota grátis) e pago com OAuth. O card do
conector (até 2.000 caracteres) pode dizer que existe plano pago; a etapa "Use cases" do
portal pergunta isso.

**Paridade (OpenAI):** um recurso do site não pode ser pior dentro do plugin.

---

## R7. Alternativas de pagamento avaliadas

Descartadas em favor do Polar.sh (seção 6). Ficam aqui para consulta.

| Opção | Taxa | Brasil / Pix | Serve para assinatura de SaaS? |
|---|---|---|---|
| **GitHub Sponsors** | 0% para conta pessoal; até 6% para organização | Paga em conta BR; o pagador usa cartão, sem Pix | Não: é doação/patrocínio |
| **Open Collective** | 10% do host + processamento | Fundos no host dos EUA; sem Pix | Não |
| **Ko-fi** | 0% em gorjeta única; 5% em mensal (0% com Gold, US$ 12/mês) + ~2,9% + US$ 0,30 | Via PayPal/Stripe; sem Pix | Só membership simples |
| **Buy Me a Coffee** | 5% + processamento | Saque no Brasil não confirmado | Só membership simples |
| **Apoia.se / Catarse** | ~13% no recorrente | **Sim: Pix, boleto e cartão em reais** | Não: financiamento coletivo |
| **Pix Automático direto** (Efí, Asaas, Mercado Pago...) | ~0–1,45% | Nativo, recorrente com mandato (Bacen, desde 2025) | Sim, no Brasil. Exige MEI/CNPJ, nota fiscal e integração própria |
| **Stripe direto** | ~3,99%+ no BR (cartão); Pix para empresa BR só por convite | Parcial | Sim, mas o imposto internacional fica com você |
| **Stripe Managed Payments** (ex-Lemon Squeezy) | ~5% + US$ 0,50 | Pix não confirmado | Sim; o imposto fica com eles |
| **Paddle** | ~5% + US$ 0,50 (não verificado) | Pix Automático para assinaturas desde 03/09/2026 | Sim. Seria o plano B se o Pix virar essencial |

---

## R8. Checklist de submissão

Vale para todos os diretórios. O kit para copiar nos formulários (descrições, prompts,
casos de teste) está em `docs/connector-review.md`.

- [x] Toda tool com `title` e anotações corretas (`readOnlyHint`, `destructiveHint`,
      `openWorldHint`) — testes `every_tool_has_hints` e `reads_change_nothing`.
- [x] Tools de leitura separadas das de escrita; nomes com até 64 caracteres.
- [x] Erros que dizem o que fazer.
- [x] Descrições sem ordens ao modelo, sem propaganda, sem upsell.
- [x] Nada de venda, preço ou link de checkout dentro do chat.
- [x] Nenhuma tool que faça pagamento.
- [x] Versão grátis útil por si só, não uma demo (a OpenAI recusa).
- [x] Dados mínimos; nada de histórico da conversa, IDs internos ou IDs de sessão.
- [x] Política de privacidade e termos publicados em 3dneweraai.com.
- [x] Documentação pública do conector: https://3dneweraai.com/connector/
- [x] Prompts de exemplo (5) e casos de teste (7 positivos, 4 negativos) em
      `docs/connector-review.md`.
- [ ] Conta de teste completa, com plano pago liberado e sem MFA.
- [x] Screenshots da MCP App em `docs/images/connector/`: planta e 3D, claro e escuro,
      1200 px e celular (1080 px a 3x), sem o prompt.
- [x] CSP declarando todos os domínios do widget (nenhum: não carrega nada da rede).
- [x] Deixar claro que as imagens vêm de renderização determinística, não de IA
      generativa (na página do conector e na descrição longa).
- [ ] Organização Claude Team (para enviar ao Connectors Directory).
- [ ] Identidade verificada na OpenAI Platform.

---

## Fontes

### Anthropic

- https://claude.com/docs/connectors/building/submission
- https://claude.com/docs/connectors/building/review-criteria
- https://claude.com/docs/connectors/building/authentication
- https://claude.com/docs/connectors/directory
- https://claude.com/docs/connectors/building/directory-vs-custom
- https://claude.com/docs/connectors/building/what-to-build
- https://claude.com/docs/connectors/building/mcpb
- https://claude.com/docs/connectors/building/mcp-apps/getting-started
- https://claude.com/docs/connectors/building/mcp-apps/cross-compatibility
- https://claude.com/docs/plugins/submit
- https://support.claude.com/en/articles/13145358-anthropic-software-directory-policy
- https://support.claude.com/en/articles/10949351-getting-started-with-local-mcp-servers-on-claude-desktop
- https://www.anthropic.com/engineering/desktop-extensions
- https://code.claude.com/docs/en/plugin-marketplaces
- https://code.claude.com/docs/en/discover-plugins
- https://github.com/anthropics/claude-plugins-official
- https://github.com/anthropics/claude-plugins-community
- https://claude.com/blog/claude-marketplace
- https://claude.com/marketplace-partners
- https://www.anthropic.com/news/claude-partner-network
- https://siliconangle.com/2026/03/06/anthropic-launches-claude-marketplace-third-party-cloud-services/

### MCP (padrão)

- https://modelcontextprotocol.io/extensions/apps/overview
- https://blog.modelcontextprotocol.io/posts/2026-01-26-mcp-apps/
- https://github.com/modelcontextprotocol/ext-apps
- https://github.com/modelcontextprotocol/mcpb
- https://github.com/modelcontextprotocol/registry/tree/main/docs
- https://registry.modelcontextprotocol.io
- https://www.devmoment.dev/journal/mcp-apps-field-log-2026

### OpenAI

- https://developers.openai.com/plugins
- https://developers.openai.com/plugins/build/plugins
- https://developers.openai.com/plugins/build/auth.md
- https://developers.openai.com/plugins/build/monetization.md
- https://developers.openai.com/plugins/deploy/submission.md
- https://developers.openai.com/plugins/deploy/app-review.md
- https://developers.openai.com/plugins/changelog
- https://developers.openai.com/apps-sdk/mcp-apps-in-chatgpt
- https://developers.openai.com/apps-sdk/app-submission-guidelines
- https://developers.openai.com/api/docs/guides/developer-mode
- https://developers.openai.com/api/docs/guides/secure-mcp-tunnels
- https://developers.openai.com/codex/mcp
- https://github.com/openai/tunnel-client
- https://learn.chatgpt.com/docs/plugins
- https://help.openai.com/en/articles/20001256-plugins-in-chatgpt-and-codex
- https://help.openai.com/en/articles/12584461-developer-mode-and-mcp-apps-in-chatgpt
- https://community.openai.com/t/pro-account-does-not-show-developer-mode-custom-app-creation-for-remote-mcp-app-testing/1379127
- https://peliqan.io/blog/chatgpt-mcp/
- https://thenewstack.io/openais-codex-gets-plugins/
- https://zuplo.com/blog/openai-codex-mcp-plugins-api-teams
- https://codex.danielvaughan.com/2026/04/11/codex-marketplace-plugin-distribution/
- https://www.usecarly.com/blog/chatgpt-plugins/
- https://www.digitalcommerce360.com/2026/03/06/openai-shifts-checkout-plans-agentic-commerce-strategy/
- https://www.hypotenuse.ai/blog/chatgpts-instant-checkout-the-next-phase-of-agentic-commerce
- https://en.wikipedia.org/wiki/GPT_Store
- https://pickaxe.co/post/openai-custom-gpts-august-2026-shutdown-migrate-to-pickaxe
- https://www.demandsage.com/chatgpt-statistics/
- https://www.getpanto.ai/blog/chatgpt-statistics

### Registries, clientes e distribuição

- https://github.com/mcp
- https://code.visualstudio.com/docs/agent-customization/mcp-servers
- https://github.com/docker/mcp-registry
- https://smithery.ai/docs/build/publish
- https://glama.ai/mcp/servers
- https://glama.ai/blog/2025-07-08-what-is-glamajson
- https://github.com/punkpeye/awesome-mcp-servers
- https://www.pulsemcp.com
- https://mcp.so/submit
- https://cursor.com/docs/context/mcp/install-links
- https://cursor.com/docs/plugins
- https://cursor.com/marketplace/publish
- https://github.com/cursor/plugin-template
- https://geminicli.com/extensions
- https://geminicli.com/docs/extensions/releasing/
- https://lmstudio.ai/docs/app/mcp/deeplink
- https://zed.dev/docs/extensions/mcp-extensions
- https://plugins.jetbrains.com/plugin/28071
- https://learn.microsoft.com/en-us/windows/ai/mcp/servers/mcp-mcpb
- https://learn.microsoft.com/en-us/microsoft-copilot-studio/mcp-certification
- https://cloud.google.com/blog/topics/developers-practitioners/publish-agents-in-gemini-enterprise-and-google-cloud-marketplace
- https://learn.microsoft.com/windows/package-manager/package/repository
- https://github.com/microsoft/winget-pkgs
- https://github.com/marketplace/actions/winget-releaser
- https://github.com/orgs/Homebrew/discussions/7050
- https://docs.flathub.org/docs/for-app-authors/requirements
- https://snapcraft.io
- https://storedeveloper.microsoft.com

### Pagamentos

- https://docs.github.com/sponsors
- https://docs.oscollective.org
- https://polar.sh/resources/pricing
- https://polar.sh/docs/merchant-of-record/supported-countries
- https://developer.paddle.com/changelog/2026/pix-automatico/
- https://www.lemonsqueezy.com/blog/2026-update
- https://apoia.se
- https://crowdfunding.catarse.com.br/nossa-taxa

