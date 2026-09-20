# Auditoria do fluxo de projeto por IA

Registro vivo iniciado em 20/09/2026 durante a revisão da planta **Clássico do Upper West Side**. Pedido do usuário: registrar todo trabalho manual, cálculo, diagnóstico ou checagem que o New Era poderia realizar automaticamente. Atualizar este arquivo conforme a revisão avança; não tratar propostas como funcionalidades já implementadas.

## Contexto e evidências

- Planta inicial conectada: revisão 1, 13 paredes, 10 ambientes e 135 peças, sem cotas nem etiquetas.
- `check_layout({})` inicialmente retornou somente uma sobreposição aceita entre escrivaninha e cadeira (`f90` / `f91`). Isso não significava um projeto funcional: `ergonomics` encontrou móveis virados para paredes, circulação inadequada e ausência de janela na suíte.
- A planta inicial coincide em nome, contagens e IDs com `web/demo.newera`. Uma cópia local foi aberta em uma instância separada do New Era para inspeção visual enquanto a renderização da aba online estava quebrada.
- Evidências locais desta execução: `target/plan-review/` (imagens, respostas de ferramentas e auditorias), `/tmp/newera-fix-check.log` e `/tmp/newera-wasm-clippy.log`. Esses caminhos são temporários e não substituem os testes e casos reproduzíveis descritos abaixo.
- Backup da revisão de layout na cópia local: `target/plan-review/upper-west-side-layout-revisado.newera`. Na aba, checkpoint **Layout corrigido — quartos, cozinha e orientação**, revisão 5. O arquivo preserva as alterações geométricas; não representa todo o histórico/checkpoints da aba.

### Validação da correção do crash

- **Antes:** o teste real no Chrome reproduziu `no filesystem on this platform` e `Uncaught RuntimeError: unreachable` ao executar `render_plan`.
- **Depois:** `make web-editor` passou; `scripts/web-mcp-e2e.mjs` passou com WebGPU e novamente com `NO_WEBGPU=1` (WebGL), incluindo as três renderizações, edição posterior, reconexão e fechamento efetivo da aba.
- `cargo clippy -p newera-mcp --target wasm32-unknown-unknown --locked -- -D warnings` passou.
- `make check` passou: formatação, Clippy de todo o workspace, 462 testes aprovados e smoke test MCP. A suíte marcou 29 testes opcionais como ignorados; não foram considerados provas de validação visual.
- Deploy ainda não confirmado; atualizar após a publicação.

## Falhas do software e oportunidades de automação

| Caso | Evidência / trabalho manual | Comportamento desejado | Verificação de aceite |
| --- | --- | --- | --- |
| **P0 — Renderizar a planta aborta o editor web** | `render_plan` chama `scene_options_for`, que cria `TopViews` usando `cache_dir()`. Sem variáveis de ambiente no WASM, chega a `std::env::temp_dir()`, que aborta com `no filesystem on this platform`; no navegador aparece `Uncaught RuntimeError: unreachable`. Foi necessário reproduzir via Chrome/CDP e ler o panic no console. | O caminho web deve usar os mesmos símbolos da planta na tela, sem acessar cache de disco. Auditar dependências de filesystem, threads e outras APIs nativas em todas as ferramentas expostas no navegador. | Imagem 2D, imagem 3D, outra imagem 2D e uma edição devem funcionar em sequência pelo relay, sem panic nem tela fatal. Teste em `scripts/web-mcp-e2e.mjs`; correção em `crates/newera-mcp/src/tools/render.rs`. |
| **P0 — Publicação não testava renderização MCP** | O teste anterior desenhava uma parede, mas não solicitava imagens. O fluxo Publish testava interação visual, enquanto CI rodava separadamente. Uma publicação podia passar sem exercitar a operação que derrubava a aba. | Testar o fluxo real da IA antes de publicar: ler, editar, renderizar, continuar editando, reconectar e encerrar. | Publish deve depender do sucesso do teste MCP; falhas de imagem devem impedir o passo de deploy. Alteração preparada nesta revisão. |
| **P1 — Diagnóstico fatal sem contexto** | O HTML informa “não conseguiu iniciar” mesmo quando o editor já iniciou e falhou durante uma ferramenta. A resposta MCP ficou pendente e depois informou que a aba saiu, sem explicar o panic original. | Diferenciar falha de inicialização de falha durante uso. Registrar ferramenta, fase, revisão, build, backend gráfico e causa original, sem incluir segredos. Permitir que a IA obtenha diagnóstico estruturado. | Uma falha injetada durante renderização deve identificar a operação e a causa; não deve culpar genericamente o navegador. **Ainda não implementado.** |
| **P1 — Persistência e recuperação da sessão** | A fonte autoritativa é a aba; após o crash a conexão se perdeu. Foi necessário usar a cópia local para inspeção e aguardar o retorno da conexão. Foi criado checkpoint antes das edições, mas checkpoint sozinho não prova recuperação após perda da aba. | Autosave durável no navegador, recuperação após panic/reload e ferramenta para verificar status e revisão salvos. Exportação/download pelo MCP deve ter semântica própria no navegador. | Após editar e interromper a aba, reabrir deve recuperar a última revisão confirmada, com backup exportável. **Ainda não verificado/implementado nesta revisão.** |
| **P1 — “Sem colisões” não significa “pronto”** | A checagem de layout inicial não trouxe os móveis inutilizáveis que apareceram em ergonomia. A IA precisou combinar `get_home`, `check_layout`, `ergonomics`, imagens, orientação e medidas. | Auditoria unificada, com módulos e escopo explícitos: geometria, uso, aberturas, instalações, iluminação e apresentação. Resposta deve declarar o que não foi verificado. | A planta inicial deve destacar armários/louças voltados para paredes e quarto sem janela, mesmo sem sobreposição geométrica. |
| **P1 — Correção isolada pode piorar outra restrição** | Girar `f92` para dentro do quarto resolve a orientação, mas revela somente 46 cm diante das portas. Girar `f155` revela 40 cm diante da ilha. Girar `f79` deixa 8,5 cm diante de parte da cama. Foi preciso testar conjuntos de alterações. | Resolver relações entre móveis em conjunto, com propostas reversíveis e comparação antes/depois. Não apresentar uma rotação como solução completa quando a peça continua inutilizável. | Uma proposta deve avaliar circulação, abertura e colisões de todas as peças afetadas; recusar ou explicar regressões antes de aplicar. O dry-run atual ajuda, mas a composição ainda foi manual. |
| **P1 — Semântica do ambiente incoerente** | `r18` se chamava “Copa”, mas continha vaso e box. O sistema recomendava fogão, geladeira e zonas de cozinha. Além disso, `f96` era uma pia de cozinha nesse banheiro. | Detectar conflito entre nome, tipo do ambiente e equipamentos. Sugerir correção sem renomear silenciosamente nem trocar o programa do usuário. Permitir tipo explícito independente do nome. | O conjunto vaso + chuveiro deve disparar revisão da classificação “copa”; o lavatório precisa ser realmente do tipo correto, não apenas receber novo nome. |
| **P1 — Aberturas e móveis altos** | A suíte não tinha janela. No closet, um armário alto estava diante da janela. O armário aéreo da cozinha também precisava ser retirado da faixa de janela. A checagem inicial não destacou esses casos. | Checar o volume real das aberturas contra marcenaria, incluindo folha, ventilação e acesso; distinguir móvel baixo de bloqueio alto. | Testes com armário cruzando janela devem produzir achado com IDs, altura e trecho bloqueado. Uma janela parcialmente bloqueada não deve contar integralmente como abertura útil. |
| **P2 — Medidas ainda precisam ser calculadas pela IA** | Foi necessário calcular caixas orientadas, folgas entre ilha e bancada, posição da cabeceira, área ocupada pelas portas e comprimento de armários. Exemplos: bancada lateral com face em x=72,5; ilha começando em x=167,5; corredor resultante de 95 cm. | Oferecer operações relacionais: encostar costas à parede, manter corredor mínimo, alinhar criados à cabeceira, posicionar assentos voltados para mesa/TV. Recalcular dependências ao redimensionar. | Proposta deve fornecer restrições satisfeitas e medidas verificáveis, sem exigir que a IA calcule centros a partir de faces. |
| **P2 — Cabeceira e mobiliário de apoio** | As camas estavam a 180°, mas os criados estavam próximos ao pé. A IA precisou interpretar o modelo e mover cama, criados e abajures juntos. | Relações semânticas entre cama, cabeceira, criados, luminárias e tapete; detectar criados no extremo errado. | Mover/reorientar a cama deve oferecer acompanhamento das peças associadas, sem separar abajur de apoio nem bloquear a circulação lateral. |
| **P2 — Resultados numéricos confusos nas aberturas** | O dry-run de janelas mostrou folgas negativas enormes contra a própria parede hospedeira, por exemplo `f25` com -342,5 e -1482,5 cm em direções ao longo de `w1`. | Medidas de abertura devem considerar vão, ombreiras e anfitrião, sem apresentar a parede onde ela está embutida como obstáculo comum. | Janela válida hospedada deve retornar largura de vão e distâncias aos cantos/aberturas vizinhas; sem falsa interpenetração com o anfitrião. |
| **P2 — Renomear agrupa achados como novos/resolvidos** | Ao renomear Copa para Banho social, o dry-run listou diversos pontos hidráulicos/elétricos ausentes como resolvidos e novos, embora continuassem ausentes. Isso dilui o efeito real da mudança. | Identidade estável por elemento/regra, com alteração de descrição separada de resolução. | Renomear um ambiente não deve contar a mesma falta de água/esgoto como defeito resolvido. |
| **P2 — Pontuação mistura estágios de projeto** | A pontuação inicial de ergonomia foi 29. Grande parte dos achados eram falta de circuitos, tomadas, hidráulica e telecom em uma planta de arquitetura/interiores. Foi necessário separar manualmente problemas de uso e disciplinas ainda não projetadas. | Declarar escopo e maturidade do projeto. Separar “ausente no desenho desta disciplina” de “defeito do layout”. Manter visível o trabalho pendente sem apresentar score único como certificado de qualidade. | Auditoria arquitetônica deve ser completa no seu escopo e declarar instalações não verificadas; auditoria executiva deve exigir as disciplinas solicitadas. |
| **P2 — Aceitação ficou órfã depois da correção** | Após reposicionar escrivaninha e cadeira, a aceitação `overlap:f90+f91` permaneceu órfã. `check_layout` a informou, mas foi necessário interpretar e limpar separadamente. | Propor limpeza de aceitações que deixaram de corresponder a um achado, junto da edição que resolveu o problema. Preservar histórico para auditoria. | Aceitação antiga não deve esconder um conflito novo nem poluir o relatório atual; remoção deve ser reversível. |
| **P2 — Teste confundia navegação com fechamento** | Mesmo após as renderizações passarem, a checagem final do E2E demorava: navegar para `about:blank` não garante destruição do documento/socket, por cache de navegação. | No teste de fechamento, fechar realmente o target do Chrome; no teste de navegação, especificar separadamente o comportamento esperado. | `Target.closeTarget` seguido de chamada MCP deve informar desconexão. Correção incluída no teste. |
| **P2 — Critério visual ausente** | Um relatório de colisões limpo não avalia composição, detalhe da marcenaria, coerência de materiais, iluminação, legibilidade de nomes/cotas ou enquadramento. A IA precisou renderizar e revisar por conta própria. | Oferecer vistas de revisão predefinidas por ambiente, cortes úteis e checklist de apresentação separado das regras funcionais. | Entrega de interiores deve incluir revisão visual do conjunto e dos ambientes, além de auditoria geométrica; sem confundir imagem atraente com projeto construtivo validado. |

## Registro das intervenções na planta

### Aplicadas e verificadas por geometria

Antes de editar, criado checkpoint **Antes da revisão de layout e acabamento**. As operações são comandos reversíveis do próprio New Era.

1. **Orientação e circulação — revisão online 3, 23 peças.**
   - Armários `f81`, `f92`, `f95`, lavatório `f84`, vasos `f85`/`f97`, sofá `f45`, poltronas `f46`/`f47` e conjunto TV `f156`/`f157` voltados para uso no interior.
   - Suíte: cama `f72` em [1100,975]; criados/abajures em y=1060; cômoda `f79` em [1230,705]; poltrona `f78` em [955,800].
   - Quarto 2: cama `f87` em [650,975]; criados em y=1060; escrivaninha `f90` em [510,785], cadeira `f91` em [605,785] e armário `f92` em [840,800].
   - O dry-run resolveu 14 apontamentos, sem novos achados; score 29 → 39. `check_layout` após aplicação não trouxe conflitos, somente a aceitação órfã da cadeira/escrivaninha. A pontuação não comprova acabamento nem projeto executivo completo.

2. **Cozinha, jantar e banheiro social — revisão online 4, 12 elementos.**
   - Bancada lateral `f155`: [42,5;260], 160 cm, frente para +x; ilha `f66`: [270,265], 205 × 100 cm. Corredor entre faces: 95 cm. Banquetas reposicionadas.
   - Geladeira `f64` em [412,125], voltada para -x, mais próxima da área de cocção; armário aéreo `f69` transferido à parede lateral.
   - Mesa de jantar `f55` deslocada 5 cm; mesa de centro `f48` deslocada 10 cm para liberar o sofá.
   - Bancada `f96` reduzida/reposicionada e vaso `f97` em [65,835], deixando 85 cm entre os volumes; box `f98` voltado para a entrada do ambiente.
   - “Copa” renomeada **Banho social**. Falta substituir o catálogo da pia de cozinha por lavatório: a troca de nome não resolve isso.
   - `check_layout` após aplicação novamente sem conflitos, somente aceitação órfã. Imagem 2D de verificação gerada na cópia local com as mesmas alterações.

### Identificadas, ainda pendentes

- Suíte sem janela; estudar abertura e verificar relação com a cabeceira.
- Closet: armário alto interfere com janela; reposicionar marcenaria e revisar vão útil.
- Banho da suíte: painel de box chega até a parede externa; revisar extensão, acesso e conjunto de chuveiro.
- Pé-direito do nível está em 250 cm, mas paredes têm 300 cm e spots estão em 297 cm: unificar a referência vertical.
- Acabamento detalhado, iluminação e apresentação ainda em revisão. Não declarar a planta concluída com base apenas nas correções acima.
- Instalações não desenhadas: esclarecer escopo e não esconder ausências por aceitações genéricas.

## Como continuar este registro

Para cada novo caso, registrar: operação que o revelou; IDs/revisão; evidência; interpretação; cálculo ou trabalho adicional feito pela IA; intervenção; verificação; automação proposta e teste de aceite. Marcar claramente **observado**, **hipótese**, **corrigido e testado** ou **pendente**. Atualizar o estado após o teste e após o deploy; código local não equivale a correção online.
