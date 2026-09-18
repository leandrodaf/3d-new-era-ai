# Product Hunt — o que vai no ar (para revisar antes de publicar)

Tudo aqui está pronto para colar no formulário do Product Hunt. **Nada foi
submetido** — o launch é decisão sua, feita no navegador. As medidas dos
assets e as regras de dia e horário seguem o guia de lançamento.

---

## Identidade do launch

| Campo | Valor | Limite |
|---|---|---|
| **Name** | 3D New Era AI | — |
| **Tagline** | `Design homes by talking to your AI` | 34/60 ✓ |
| **Website** | https://3dneweraai.com | — |
| **Topics** | Open Source · Artificial Intelligence · Developer Tools · Design Tools | 3–5 ✓ |
| **Slug provável** | `3d-new-era-ai` | — |
| **Pricing** | Free | — |

### Taglines alternativas (se quiser trocar o ângulo)

- `The floor plan editor your AI agent can edit` (43)
- `Open-source home design with an MCP server inside` (49)
- `Your AI draws the floor plan. You approve it.` (45)

---

## Description (o texto do post)

> 3D New Era AI is an open-source home design editor written in Rust — walls,
> doors, windows, joinery, lighting, cameras — with a Model Context Protocol
> server built in.
>
> Open it and your AI agent gets 44 tools over MCP: it draws the plan in
> centimetres, furnishes the rooms from a 128-piece catalog, builds cabinets a
> workshop can cut, checks lighting and accessibility against standards, and
> renders the photos. Every call is a document command — it shows up on screen
> immediately and Ctrl+Z undoes it.
>
> It runs on your machine. No account, no subscription, nothing uploaded: the
> MCP server listens on localhost only and your project is a file you own. The
> 3D view uses the GPU when there is one, and photos render on the CPU — even
> on a headless server.
>
> Works with Claude Code, Claude Desktop, Codex, Gemini CLI, Cursor, VS Code,
> Windsurf, Cline and anything else that speaks MCP. Opens Sweet Home 3D
> projects. Windows, macOS and Linux, in four languages.
>
> Try it in the browser without installing: https://3dneweraai.com/app/

---

## Primeiro comentário (postar no minuto zero)

> Hi Product Hunt 👋
>
> I build software, and every time I needed a floor plan I ended up in a tool
> that my tools could not talk to. So I wrote the editor I wanted: it is Rust,
> it is open source, and it ships with an MCP server inside.
>
> That last part is the whole point. You open the app and tell your agent
> "draw a 4 × 5 m bedroom with a door and a window, furnish it and render a
> photo" — and it does, on the actual project, in centimetres, while you watch.
> Not an image generator: an agent with 44 tools editing the same document you
> have open, one Ctrl+Z away from undone.
>
> A few things I am proud of:
> - **It checks the work.** Lux per room by photometry against the standard,
>   wheelchair turning circles, the kitchen triangle — each finding with the
>   fix ready to apply.
> - **Joinery a workshop can build.** Cabinets and countertops with exact
>   cutouts, and the cut list as CSV or DXF.
> - **It is local.** No account, no cloud, no telemetry of your project. The
>   MCP server only accepts connections from your own machine.
>
> It also runs in the browser (WebAssembly, nothing sent anywhere) if you want
> to look before installing: https://3dneweraai.com/app/
>
> The catalog, the standards and the renderer are all in the repo —
> https://github.com/leandrodaf/3d-new-era-ai. I would love to hear what you
> would ask an agent to design first, and what is missing for it to be useful
> in your work.

---

## Assets (prontos, nas medidas do PH)

| Arquivo | Medida | Uso |
|---|---|---|
| `gallery-1-editor.png` | 1270×760 | Galeria 1 — a janela com a casa mobiliada |
| `gallery-2-render.png` | 1270×760 | Galeria 2 — foto renderizada pelo app |
| `gallery-3-plan.png` | 1270×760 | Galeria 3 — planta humanizada |
| `gallery-4-day.png` | 1270×760 | Galeria 4 — tema claro e quatro idiomas |
| `thumbnail-300.png` | 300×300 | Thumbnail |
| `banner-1920x600.png` | 1920×600 | Banner |

Falta (decisão sua): **vídeo de demo < 2 min**. O mais forte seria a tela
gravada com um agente desenhando o apartamento pelo MCP, em tempo real.

---

## Dia do launch

Da documentação, o que vale seguir:

- **Terça a quinta, 00:01 PT.** Nunca segunda nem sexta. Submeter até 5am PT.
- **Primeiro comentário no minuto zero** (o texto acima), e responder tudo em
  até 30 min — responsividade pesa no ranking.
- **Nunca pedir upvote em público.** Peça "apoio" ou comentário, no máximo 3
  pedidos por pessoa, espaçados em 2–3 horas.
- Comunidades que combinam com este produto: r/Rust, r/SideProject, Indie
  Hackers, comunidades de MCP e de Sweet Home 3D.
- Contagem de upvotes fica oculta nas primeiras 4 horas; ~10% dos posts são
  featured; 150–300 upvotes é um bom resultado.

### Acompanhar ao vivo

```sh
scripts/ph-stats.mjs --slug 3d-new-era-ai --watch
```

Precisa do developer token uma vez. O aplicativo OAuth já está criado na sua
conta — **3D New Era AI — launch dashboard** (id 299674, redirect
`https://3dneweraai.com/`) — e o token já existe, em
https://www.producthunt.com/v2/oauth/applications, ao lado de "Token:".

Eu não leio esse token. Ele vai da tela para o cofre sem passar por aqui:

1. selecione o valor ao lado de "Token:" e copie (⌘C);
2. rode `scripts/apoio/guardar-token-ph.sh`.

O script guarda em **Agendo Certo › Product Hunt - developer token**, limpa a
área de transferência e confere o token contra a própria API (`--whoami`) —
se não for aceito, ele avisa. Depois disso:

```sh
op run --env-file <(echo 'PH_TOKEN=op://Agendo Certo/Product Hunt - developer token/credential') -- \
  scripts/ph-stats.mjs --slug 3d-new-era-ai --watch
```

Sem cofre dá para `export PH_TOKEN=...` na sessão — só não commite.
O script **só lê**: não vota, não comenta e não publica nada.

### Depois do launch

O badge só é liberado se o produto ficar entre os primeiros do dia, semana ou
mês. Quando liberar, o rodapé do site já tem o lugar dele: preencha
`PRODUCT_HUNT` em `site/app.js` com o id do post e ele aparece.
