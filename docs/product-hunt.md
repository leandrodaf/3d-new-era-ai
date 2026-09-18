# Product Hunt — Documentação consolidada (launch + API)

> Compilado em 2026-09-18 a partir da documentação oficial e de guias da comunidade.
> Fontes completas no final.

---

## Contexto deste projeto (3D New Era AI)

- **Produto:** editor open-source de plantas baixas e interiores em Rust, com servidor MCP embutido para agentes de IA; 100% local (sem conta, sem nuvem); Windows/macOS/Linux; UI em PT/EN/ES/FR
- **Site:** https://3dneweraai.com · **GitHub:** https://github.com/leandrodaf/3d-new-era-ai
- **Ângulo forte pro PH** (audiência = early adopters técnicos, makers, devs): open-source + Rust + MCP embutido + IA local. O pitch deve liderar com isso, não com "editor de interiores".
- **Topics sugeridos:** `Open Source`, `Artificial Intelligence`, `Developer Tools`, `Design Tools`, `Productivity` (3–5 no total)
- **Slug provável do post:** `3d-new-era-ai` (definido na submissão; os exemplos de query abaixo usam esse slug como placeholder — ajuste se o PH gerar outro)
- **Comunidades pra mobilizar no dia:** r/Rust, r/SideProject, Indie Hackers, comunidades de Sweet Home 3D e MCP
- **Onde colocar o badge depois do launch:** footer do site (`site/`), READMEs e no `site/llms.txt`/blog

---

## 1. Visão geral

Product Hunt (PH) é um site onde makers lançam produtos e a comunidade vota. Para o seu app, ele serve como:

- **Evento de lançamento** — pico de tráfego concentrado no dia do launch (dura 1–3 dias)
- **Credibilidade** — selo "top product" + backlink dofollow de domínio DR91
- **Sinal pra imprensa/investidores** — audiência de early adopters

**Não** é um canal de aquisição contínuo. A conversão típica é ~3% de visitante → signup (guias otimistas citam 5–15%).

### O que vale a pena integrar (resumo)

| Integração | Esforço | Retorno |
|---|---|---|
| Badge/embed no seu site | Copiar snippet, zero API | Alto — prova social permanente |
| API v2 (GraphQL) read-only | ~1h com developer token | Médio — painel de métricas no dia do launch |
| Write access (postar via API) | Requer aprovação | Baixo — fluxo manual é melhor |

---

## 2. Badges & Embeds

Disponíveis na página do seu produto: `producthunt.com/posts/<seu-slug>/embed`.

### Tipos

1. **Drive support (website badges)** — badge pro rodapé/home pra direcionar a comunidade pro seu launch
2. **Social proof** — valida o produto pra potenciais clientes
3. **Post embed** — embed completo do launch pra blogs, press kit e parceiros
4. **Reviews / review wall** — mosaico de reviews da comunidade pra landing page; reviews individuais também são embeddáveis via link "share" embaixo de cada review

### Temas

Website badges têm 3 temas: **Light, Neutral, Dark**.

### Elegibilidade

Badges são **desbloqueados** ao ficar entre os top produtos do **dia, semana ou mês**. O snippet do embed só aparece depois disso (é renderizado no client-side, não há código estático na página).

### Ação disponível

- **Generate review wall** → `/products/<slug>/embed/testimonial`

---

## 3. API v2 (GraphQL)

### Endpoints

| O quê | URL |
|---|---|
| GraphQL API | `https://api.producthunt.com/v2/api/graphql` |
| OAuth authorize | `https://api.producthunt.com/v2/oauth/authorize` |
| OAuth token | `https://api.producthunt.com/v2/oauth/token` |
| Dashboard (apps/tokens) | `https://www.producthunt.com/v2/oauth/applications` |
| Referência da API | `https://api-v2-docs.producthunt.com/operation/query/` |
| API Explorer (GraphiQL) | `https://ph-graph-api-explorer.herokuapp.com/` |
| Schema oficial | `https://github.com/producthunt/producthunt-api/blob/master/schema.graphql` |

Todas as requests usam o header `Authorization: Bearer {token}` e são POST com body JSON contendo `query`.

### Autenticação

**Scopes (3):**
- `public` — lê informações públicas (default, read-only)
- `private` — age em nome do usuário autenticado (ex.: `viewer`, goals)
- `write` — mutations (requer aprovação; pedir `public private write`)

**Fluxos:**

1. **OAuth com usuário** — authorize → access grant → token exchange:
   ```bash
   curl -X POST https://api.producthunt.com/v2/oauth/token \
     -H "Content-Type: application/x-www-form-urlencoded" \
     -d "grant_type=authorization_code" \
     -d "client_id=SEU_CLIENT_ID" \
     -d "code=CODIGO" \
     -d "redirect_uri=SUA_REDIRECT_URI" \
     -d "code_verifier=SEU_CODE_VERIFIER"
   ```

2. **Client-only (sem usuário)** — token sem contexto de usuário, limitado a endpoints públicos. Campos user-level (ex.: `isVoted`) retornam default (`false`/`nil`).

3. **PKCE (public clients)** — apps native/mobile/SPA registrados como *Public client* não recebem `client_secret`:
   - `code_verifier`: 43–128 chars (A–Z, a–z, 0–9, `- . _ ~`)
   - `code_challenge = BASE64URL-ENCODE(SHA-256(ASCII(code_verifier)))`
   - Só `S256` é aceito; `plain` é rejeitado
   - Erro `invalid_request` se faltar `code_challenge`; `invalid_grant` se `code_verifier` errado
   - Codes são single-use e expiram rápido
   - Parâmetros do authorize: `client_id`, `response_type=code`, `redirect_uri` (match exato), `scope`, `state`, `code_challenge`, `code_challenge_method`

4. **Developer token (recomendado pra scripts)** — token **sem expiração**, vinculado à sua conta, disponível no dashboard `producthunt.com/v2/oauth/applications`. Perfeito pra read-only sem montar OAuth.

> ⚠️ Nunca coloque `client_secret` em código de frontend.

### Rate limits

| Endpoint | Limite | Período |
|---|---|---|
| `/v2/api/graphql` | 6.250 **pontos de complexidade** (calculado pelos campos pedidos) | 15 min |
| Demais `/v2/*` | 450 requests (exemplos mostram 900 no oauth/token) | 15 min |

**Headers em toda resposta:**

| Header | Significado |
|---|---|
| `X-Rate-Limit-Limit` | Limite da sua app no período de 15 min |
| `X-Rate-Limit-Remaining` | Quota restante |
| `X-Rate-Limit-Reset` | Segundos até resetar |

Excedeu → **HTTP 429** até o reset. Dicas: peça só os campos necessários (query enxuta ~150–300 pontos; queries pesadas queimam a cota em <30 requests), use exponential backoff em 429, e cacheie snapshots. Limites maiores podem ser pedidos (a PH se reserva o direito de ajustar).

### Queries de exemplo

**Lista de posts (curl):**
```bash
curl -X POST "https://api.producthunt.com/v2/api/graphql" \
  -H "Authorization: Bearer SEU_DEVELOPER_TOKEN" \
  -H "Content-Type: application/json" \
  -d '{"query": "query { posts(first: 5, order: RANKING) { edges { node { id name tagline votesCount url } } } }"}'
```

**Top posts do dia com makers e topics:**
```graphql
query {
  posts(order: VOTES, first: 10) {
    edges {
      node {
        id
        name
        tagline
        votesCount
        commentsCount
        url
        website
        createdAt
        makers { name username }
        topics { edges { node { name } } }
      }
    }
  }
}
```

**Post específico por slug (o que você vai usar no dia do launch):**
```graphql
query {
  post(slug: "3d-new-era-ai") {
    id
    name
    tagline
    description
    votesCount
    commentsCount
    reviewsCount
    reviewsRating
    url
    website
    createdAt
    featuredAt
    makers { name username headline }
    comments(first: 5) {
      edges { node { body votesCount createdAt user { name username } } }
    }
  }
}
```

**JavaScript (fetch):**
```js
fetch('https://api.producthunt.com/v2/api/graphql', {
  method: 'POST',
  headers: {
    'Authorization': 'Bearer SEU_TOKEN',
    'Content-Type': 'application/json'
  },
  body: JSON.stringify({
    query: 'query { posts(first: 5) { edges { node { id name tagline } } } }'
  })
}).then(r => r.json()).then(data => console.log(data));
```

### Schema (tipos e campos principais)

`DateTime` é ISO-8601 UTC. Todas as conexões seguem o padrão Relay: `edges { cursor node }`, `pageInfo { endCursor hasNextPage hasPreviousPage startCursor }`, `totalCount`, com argumentos de paginação `after, before, first, last`.

**Post** (implementa `TopicableInterface` e `VotableInterface`):
`collections`, `comments(..., order: CommentsOrder = NEWEST)`, `commentsCount`, `createdAt`, `description`, `featuredAt`, `id`, `isCollected`, `isVoted`, `makers`, `media`, `name`, `reviewsRating`, `slug`, `tagline`, `thumbnail`, `topics`, `url`, `user`, `userId`, `votes`, `votesCount`, `website`

**User**: `coverImage(height, width)`, `createdAt`, `followedCollections`, `followers`, `following`, `headline`, `id`, `isFollowing`, `isMaker`, `isViewer`, `madePosts`, `name`, `profileImage(size)`, `submittedPosts`, `twitterUsername`, `url`, `username`, `votedPosts`, `websiteUrl`

**Comment** (implementa `VotableInterface`): `body`, `createdAt`, `id`, `isVoted`, `parent`, `parentId`, `replies`, `url`, `user`, `userId`, `votes`, `votesCount`

**Topic**: `createdAt`, `description`, `followersCount`, `id`, `image(height, width)`, `isFollowing`, `name`, `postsCount`, `slug`, `url`

**Collection** (implementa `TopicableInterface`): `coverImage`, `createdAt`, `description`, `featuredAt`, `followersCount`, `id`, `isFollowing`, `name`, `posts`, `tagline`, `topics`, `url`, `user`, `userId`

**Vote**: `createdAt`, `id`, `user`, `userId`

**Viewer**: `goals`, `makerGroups`, `makerProjects`, `user`

**Media**: `type`, `url(height, width)`, `videoUrl`

**Query root:**

| Campo | Argumentos | Retorna |
|---|---|---|
| `collection` | `id, slug` | `Collection` |
| `collections` | `after, before, featured, first, last, order = FOLLOWERS_COUNT, postId, userId` | `CollectionConnection!` |
| `comment` | `id!` | `Comment` |
| `post` | `id, slug` | `Post` |
| `posts` | `after, before, featured, first, last, order = RANKING, postedAfter, postedBefore, topic, twitterUrl` | `PostConnection!` |
| `topic` | `id, slug` | `Topic` |
| `topics` | `after, before, first, followedByUserId, last, order = NEWEST, query` | `TopicConnection!` |
| `user` | `id, username` | `User` |
| `viewer` | — | `Viewer` |
| `goal` / `goals` / `makerGroup(s)` | — | goals/maker groups |

**Mutations** (todas com `input` e retornando payload com `clientMutationId`, `errors: [Error!]!`, `node`):
`goalCheer`, `goalCheerUndo`, `goalCreate`, `goalMarkAsComplete`, `goalMarkAsIncomplete`, `goalUpdate`, `userFollow`, `userFollowUndo`

**Enums de ordenação:**
- `PostsOrder`: `FEATURED_AT`, `NEWEST`, `RANKING`, `VOTES`
- `CommentsOrder`: `NEWEST`, `VOTES_COUNT`
- `TopicsOrder`: `FOLLOWERS_COUNT`, `NEWEST`
- `CollectionsOrder`: `FEATURED_AT`, `FOLLOWERS_COUNT`, `NEWEST`

### Limitações conhecidas

- **Sem busca por texto livre** em posts — navegue por topic/date ou busque o post por `slug`/`id`. Pra achar o slug de um produto: pesquise no site e pegue o caminho da URL (`producthunt.com/posts/<slug>`).
- `featuredAt: null` significa que o post nunca entrou no fluxo de descoberta (não foi featured).
- ⚠️ Uma fonte (Crawlora, 2026) afirma que o endpoint de comments foi descontinuado em maio/2026 e retorna `410 Gone`; exemplos recentes ainda mostram `comments`/`commentsCount` funcionando. **Verifique contra os docs atuais antes de depender disso.**

### Planos

| Plano | Preço | O que inclui |
|---|---|---|
| **Developer (Free)** | $0 | Read-only não-comercial: dados de posts, produtos, votes, comments, users, topics, collections; scopes public/private; OAuth 2.0; developer token; API Explorer |
| **Write Access** | sob consulta | Aprovação especial: postar comments e outras writes, criar/gerenciar goals, follow/unfollow de users |
| **Commercial** | sob consulta | Uso comercial explícito + rate limits potencialmente maiores |

Contato pra write/commercial: **hello@producthunt.com**. Uso comercial é proibido por padrão; attribution com link pro PH é pedida.

### Endpoints de terceiros (referência)

O Anysite (`anysite.io`) expõe 23 endpoints REST sobre PH (products/search, products, reviews, alternatives, launches, launches/comments, leaderboards, users, topics, collections, categories, forums etc.) — útil como alternativa REST se você não quiser GraphQL, mas é serviço pago de terceiro, não oficial.

---

## 4. Guia de launch

### Cronograma (30 dias antes)

- Construa uma página "notify me" / lista de espera
- Recrute **200–500 supporters** (pessoas que vão votar no dia)
- Finalize assets (especificações abaixo)
- Contate hunters 1–2 semanas antes (hunter ideal: ≥5k followers, >30 hunts upvotados)
- Envie emails personalizados pra 150–200 founders/investidores com link de upvote one-click (marcado com UTM)
- A/B teste a hero da landing page por 48h e mantenha a variante com ≥9% CTR

### Checklist do MVP (não lance sem)

- Demo one-click; carrega em <2s; Lighthouse ≥ 90
- Backend aguenta pico (≥200 RPS)
- Valor em 1 frase + 3 bullets; pitch <30 palavras
- Suporte com resposta <5 min (chat + FAQ)
- **Lançar com produto funcionando, nunca só landing page**

### Especificações de assets

| Asset | Especificação |
|---|---|
| Galeria do launch | **1270×760 px** |
| Thumbnail | **300×300 px** |
| Banner | **1920×600 px** |
| Tagline | ≤ **60 caracteres** |
| Vídeo de demo | < **2 min** |
| Hero image | 1200×630 PNG, fundo transparente, <200 KB |
| Demo GIF | 800×450, loop de 8s, <1 MB |
| Pitch deck | 8 slides, PDF ≤1 MB |
| Topics | 3–5 tags |

### Timing

- **Terça a quinta, 00:01 PT** (fuso do Pacífico) — o PH roda meia-noite a meia-noite PT
- Nunca segunda nem sexta
- Submeta até 5am PT pra dar margem ao hunter
- 62% dos hunts top-rankeados saem numa terça
- Pico de velocidade: ~0,9 upvotes/seg nos primeiros 30 min

### Dia do launch (24h)

| Janela | Ação |
|---|---|
| Imediato | Poste o **primeiro comentário** (história do produto); responda todo comentário em ≤2–30 min (responsividade afeta ranking) |
| 30–120 min | Solte um GIF de demo nos comentários (máx 1 por 30 min) |
| 2–6 h | Post "behind the scenes" (ex.: diagrama de arquitetura) |
| 6–12 h | Rode uma enquete: "qual integração vocês querem a seguir?" — use como sinal de roadmap |
| Dia todo | Reserve 16+ horas; não faça outros compromissos |

Stagger os pedidos de apoio ao longo de 2–3 horas pra não parecer spam.

### Regras da comunidade (importante)

- **Nunca peça upvote publicamente** — peça pra "apoiar" ou comentar
- Máx **3 pedidos de upvote por pessoa** — exceder dispara moderação e possível ban
- **Não compre upvotes** — o algoritmo (2025/26) detecta manipulação; US$1.500–2.000 gastos nisso *prejudicam* o ranking. Comentários genuínos de contas estabelecidas valem muito mais.
- Não use bots de auto-reply de forma agressiva — o que aparece é monitorado

### Como o algoritmo funciona (2025–2026)

- O time do PH **cura manualmente** a homepage
- Contagem de upvotes fica **oculta nas primeiras 4 horas**
- Só **~10% dos submissions** são featured

### Expectativas realistas

- **150–300 upvotes** = bom; **300–600** = ótimo
- **Top-6** te coloca na primeira página
- Tráfego em pico dura **1–3 dias**; conversão ~3% visitante→signup
- Restrição de **relançar por 6 meses**
- Após o launch: publique o badge no site e capitalize o backlink

---

## 5. Ferramentas / automação

- **LaunchPal (MCP server)** — automação de launch pra agentes: `login_producthunt`, `create_product`, `schedule_launch`, `process_images` (otimiza pras specs do PH), `track_launch`, `generate_launch_report`, `get_trending`, `find_hunters`, `optimize_launch_time`
- **ph-launch (CLI, openclaw)** — monitor de métricas do launch day via developer token: `ph-launch stats --slug "seu-slug"`, monitor ao vivo com polling e leaderboard
- **Skills de agentes** (SkillsMP/SkillsAuth/LobeHub) — skills prontas de "product-hunt-launch" que dirigem o wizard de submission via automação de browser (com safety stops: nunca loga por você, nunca clica Schedule/Publish, nunca resolve CAPTCHA). Gotcha técnico: o form do PH é React SPA — campos de texto precisam do setter nativo de valor via script, senão o `fill` perde caracteres.
- **Sponsored hunt** — listing patrocinado por **US$500** via mutation `createHunt` (GraphQL com `Authorization: Bearer`): nome, tagline, thumbnail, website, redirect URL e topics; retorna `huntId` e URL pra push pré-launch por email.

---

## 6. Fontes

- [Product Hunt API Docs (oficial)](https://api.producthunt.com/v2/docs)
- [Product Hunt API Reference](https://api-v2-docs.producthunt.com/operation/query/)
- [Rate Limits: Headers (oficial)](https://api.producthunt.com/v2/docs/rate_limits/headers)
- [OAuth Client Only Authentication (oficial)](https://api.producthunt.com/v2/docs/oauth_client_only_authentication/unauthorized_oauth_oauth_test_invalid_access_to_user-level_content_with_just_an_client_level_token_will_lead_to_errors)
- [Product Hunt GraphQL API (api-evangelist)](https://raw.githubusercontent.com/api-evangelist/producthunt/refs/heads/main/graphql/producthunt-graphql.md)
- [API Plans (api-evangelist)](https://raw.githubusercontent.com/api-evangelist/producthunt/refs/heads/main/plans/plans.yml)
- [Schema GraphQL oficial](https://github.com/producthunt/producthunt-api/blob/master/schema.graphql)
- [Badges & Embeds](https://www.producthunt.com/posts/api-diff/embed)
- [Product Hunt Endpoint Reference — 23 Endpoints (Anysite)](https://anysite.io/endpoints/producthunt/)
- [How to Crush Your AI Product Launch on Product Hunt and LinkedIn (DEV)](https://dev.to/howiprompt/how-to-crush-your-ai-product-launch-on-product-hunt-and-linkedin-a-no-fluff-guide-for-13mp)
- [How to Scrape Product Hunt in 2026 (Crawlora)](https://crawlora.net/blog/how-to-scrape-product-hunt)
- [LaunchPal MCP Server (LobeHub)](https://lobehub.com/tr/mcp/lekt9-launchpal-mcp)
- [ph-launch CLI skill (openclaw)](https://skillsauth.com/skills/openclaw/product-hunt-launch)
- [Product Hunt API — Complete Integration Guide (blog)](https://blog.saaspa.ge/product-hunt-api-the-complete-integration-guide) *(fora do ar em 2026-09-18)*
