# Atritos de uso

O que ainda dói quando um agente conduz uma planta real pelo MCP, do lado de fora.

Cada caso traz **o que aconteceu**, com a resposta literal que serve de prova,
**como reproduzir** em um comando, e **o que deveria acontecer**. O banco de
provas é sempre a mesma planta — um apartamento de 65 m² com marcenaria
desenhada módulo a módulo, hoje em nota 99 e zero colisões.

Os casos resolvidos saem daqui conforme caem; o histórico fica no git.

---

## 26. O `turned` parou de avisar, e a face continua errada

O mais sério da lista, porque apagou o próprio alarme.

O vassoureiro extraível abre para `-y`, o corredor da cozinha: é lá que estão a
frente, o puxador e 67,6 cm livres. Do outro lado há 0,9 cm, contra o próprio
roda-teto.

```
get_home(ids=["f1218"])   → "faces": "+y"
measure(from="f1218")     → "faces": "+y",  "-y": [67.6, "f829", …]
check_layout()            → turned: f901, f909, f1130, f1165   (f1218 não está)
```

Na versão anterior ele aparecia em `turned` com `built: -y, placed: +y` — a
face **certa** e o ângulo errado, com o aviso apontando a divergência. Agora as
duas ferramentas concordam **no valor errado**, e por concordarem o `turned`
deixou de apontar. Era o único lugar onde isso aparecia.

A peça foi montada com `arrange group` sobre sólidos desenhados: não tem painel
de porta que o motor reconheça como frente.

**Reproduzir:** `measure(from="f1218")` — compare o `faces` com onde estão a
frente e o puxador (`get_home(ids=["f1210","f1212"])`, y 415,6–419,1).

**Deveria:** derivar a face das frentes quando houver; quando não houver, dizer
que não sabe em vez de escolher um lado — e continuar sinalizando.

## 27. O `anchor` enxerga dentro dos grupos; o `stale` não

Descoberto agora, ancorando as cotas soltas da planta.

```
annotations(anchor=true)   → {"anchored": ["d104", "d105"]}
annotations(stale=true)    → ["d105", 91, 91, "f1210",
                              "f1210 is gone; this no longer marks anything"]
```

Repare nos números: **escrito 91, medido 91** — a cota está certa. E `f1210`
não sumiu:

```
get_home(ids=["f1210"])
→ "Vassoureiro extraível — frente 30 × 195 cm", bounds [[615,417.1],[645,419.1]]
```

Ela existe: é **parte do grupo** `f1218`. O `anchor` desceu no grupo e amarrou a
cota nela; o `stale` só procura no nível de cima, não acha, e declara a âncora
morta. A cota fica permanentemente stale, com o número correto, a cada revisão.

É o mesmo efeito do antigo caso 13 — cota travada em estado de erro — mas por
outra causa, e esta reproduz em um par de comandos.

**Reproduzir:** os dois comandos acima, nesta ordem.

**Deveria:** o `stale` procurar a âncora também dentro dos grupos, como o
`anchor` já faz.

## 17. Grupo só sabe escalar tudo junto

Um armário de 30 cm que precisa virar 40 cm não engorda as chapas: os montantes
continuam com 2 cm e quem cresce é o vão entre eles.

```
update(items=[{"id": "f1218", "w": 40}], dry=true)
→ f957  lateral:    2 cm  → 2,7 cm
→ f1214 corrediça:  1,5   → 2
→ f1216 fundo:      26    → 34,7
```

Fator 1,333 aplicado a tudo. Chapa de 15 mm não vira 20 mm porque o armário
ficou mais largo, e corrediça telescópica é comprada em medidas de catálogo.

O mesmo apareceu na moldura da mesa basculante: levar a face de 180 para 131 cm
**encolhia** a mesa de 110 para 80, quando o pedido era manter os montantes e
crescer a mesa até a torre. A saída foi `ungroup`, refazer as sete peças na mão
e deixar desagrupado.

**Reproduzir:** o `update` acima, em dry.

**Deveria:** haver como dizer o que estica e o que fica — ou espessuras de
chapa e ferragem serem preservadas por padrão.

## 25. `joinery` não tem `anchor`, e o armário se descola da parede

```
joinery(id="f1291", p={"d": 55})
antes:  bounds y 699–756   (encostado na parede)
depois: bounds y 700–755   (1 cm de vão atrás)
```

Encolhe pela frente **e** pelo fundo. Nenhum armário de parede tem vão atrás, e
o problema não aparece em render nenhum de frente — só medindo.

O `update` resolve isso desde sempre, e a documentação dele explica exatamente
este caso: *"anchor … holds one face still … so a run of joinery keeps its back
on the wall"*. Mas `anchor` é de `update`, e mudar parâmetro de build é
`joinery`.

**Reproduzir:** o comando acima, seguido de `get_home(ids=["f1291"])`.

**Deveria:** `anchor` em `joinery` com o mesmo significado — ou o fundo ficar
parado por padrão, já que o `cabinet_run` sabe em que parede pôs o módulo.

## 23. Não há como reusar um modelo que já está no projeto

Para repetir o arremate de madeira da cozinha em mais cinco lugares:

```
place(items=[{"model": "51/crown.obj", …}])
→ "51/crown.obj: could not read the file: No such file or directory"

place(items=[{"cat": "f832", …}])
→ "unknown catalog id `f832` (use the catalog tool)"
```

O caminho `51/crown.obj` é exatamente o que `catalog(scope="project")` devolve
para aquela peça, mas nomeia um modelo **embutido no projeto**, não um arquivo
em disco — o original pode ter vindo de um `.sh3d` importado meses atrás.

O `catalog(scope="project")` até anuncia a intenção certa — *"one id to copy
from"* — mas não existe o "copiar esta peça". O contorno foi `arrange array`
com `dy: 1000`, para jogar as cópias fora da casa, e `update` em cada uma para
trazê-la de volta. Oito peças de roda-teto nasceram assim.

**Reproduzir:** qualquer um dos dois comandos acima.

**Deveria:** `place` aceitar o id de uma peça existente como fonte, ou um
`copy` em `arrange` que não precise arremessar as cópias para longe primeiro.

## 21. `accept` no `check_layout` ainda não vale para `in_wall`

O `accept` chegou e funciona: `overlap`, `outside_rooms` e `turned` trazem
`key`, e `orphaned` avisa quando o motivo deixa de valer.

`in_wall` não tem `key`:

```
check_layout()
→ in_wall:        [[{bounds, id, level, name, z}, …]]        sem key
→ outside_rooms:  [{bounds, id, key, level, name, z}, …]     com key
```

São as duas persianas de rolo integradas, que estão dentro da espessura da
parede porque é ali que uma persiana de rolo fica. Duas linhas que voltam a
cada revisão sem como dizer "visto, está correto".

**Reproduzir:** `check_layout()` e comparar as chaves das duas listas.

**Deveria:** `key` em `in_wall` também.

## 28. Limpar a herança de uma importação é trabalho de garimpo

A planta veio de um `.sh3d` e trouxe junto: 14 properties de interface do
SweetHome3D (posição da janela, divisor de painel, escala do plano, tamanho da
tela), um `sh3d:id` em cada nível, **73 rótulos `[NN]`** numerando peças à mão,
**21 rótulos `L1`–`L21`** numerando luminárias, **13 cotas** de ambiente e
**59 nomes** de peça começando com o número do índice antigo.

Tudo isso o app já resolve nativamente: `references` numera 126 peças por
cômodo com nome e medidas, e `auto_dimensions` cota os ambientes. Os dois
estavam ligados — então a planta carregava **duas numerações simultâneas e
divergentes**, a manual e a nativa, sobrepostas no mesmo desenho. Na cama
apareciam `[20]` e `4` lado a lado.

Limpar isso levou a sequência inteira abaixo, e cada passo esbarrou em algo:

**Não há ferramenta para properties.** Nenhum tool MCP toca as properties do
projeto ou dos níveis. Foi preciso ler `crates/newera-core/src/command.rs` para
descobrir que `/api/commands` existe, que os comandos são tagueados por `op`, e
que `Element` é tagueado por `kind`. Um agente sem o código-fonte à mão não
chega lá — e o erro que guia até isso é só `missing field 'op'`.

**O REST não enxerga partes de grupos.** O mesmo `update` que renomeia uma peça
de topo responde `f935 not found` para uma parte. A operação é uma só, e
precisa de dois caminhos: REST para o topo, MCP para dentro dos grupos.

**Não há edição em massa.** Os 59 renomes seguiam uma regra de uma linha
(remover o prefixo `NN — `). Os 53 que estavam dentro de grupos tiveram que
passar **inteiros pelo contexto do agente**, em três chamadas, porque só o MCP
os alcança. E no REST o `update` exige o elemento **completo**, não um patch:
baixar tudo, alterar um campo, devolver tudo.

**Não há busca por padrão.** Para achar os 73 rótulos `[NN]` foi preciso baixar
o JSON e passar um regex. O `annotations(q=…)` acha texto literal
(`q="Vidro"` → o rótulo certo), mas `q="[0"` devolve vazio, então não serve
para varrer uma convenção de nomenclatura.

**Encurtaria:** um `update` que aceite um filtro e uma transformação — ou ao
menos que alcance partes de grupo pelo REST, onde o lote é possível.

## 29. Ligar um modo de anotação custa 6 mil tokens da mesma lista

`annotations` responde sempre a mesma coisa, independentemente do que se pediu:

```
annotations(refs=true)    → schedule de 126 itens  (~6k tokens)
annotations(legend=true)  → o mesmo schedule de 126 itens
annotations(dims=true)    → o mesmo schedule de 126 itens
```

Ligar os três modos custou 18 mil tokens da mesma lista repetida, e em nenhuma
delas apareceu o que se pediu: nem as cadeias de cota (`dims`), nem a legenda
de símbolos com contagem que a documentação promete para `legend`
(*"Legend of electrical/plumbing symbols with counts"*).

Para saber se `dims` tinha surtido efeito, o caminho foi renderizar a planta e
procurar a olho um "345" que aparecia duas vezes no dormitório — a cota manual
e a automática, sobrepostas.

**Encurtaria:** responder o que foi pedido, e confirmar a mudança de modo com
uma linha em vez do schedule inteiro.

## 30. As anotações do plano sumiram, uma vez, sem aviso

Depois da sequência de limpeza, `annotations` estava `None` — os 126 números do
desenho tinham desaparecido, e só percebi porque renderizei a planta para
conferir o resultado.

Tentei isolar o culpado e **não reproduzi**: `set_properties` (com o mesmo
conteúdo, adicionando chave e removendo chave), `update` de nível (um e os
três), `update` de móvel pelo REST — nenhum apaga. A sequência original tinha,
entre esses, dois `delete` em massa pelo MCP (73 + 21 labels, 13 cotas) e três
`update` de 19 partes pelo MCP.

Fica registrado como observado uma vez, porque é perda silenciosa de
configuração: nada no retorno de nenhum comando mencionou as anotações.

**Encurtaria:** qualquer comando que zere `annotations` dizer isso no retorno.

## 31. MCP e JSON usam nomes diferentes para o mesmo campo

Detalhe pequeno que confunde toda inspeção pelo REST:

| no MCP | no JSON |
|---|---|
| `dims` | `auto_dimensions` |
| `refs` | `references` |
| `details` | `reference_details` |
| `legend` | `legend` |

Ao conferir pelo `/api/home` se um modo ficou ligado, é preciso traduzir de
cabeça — e `dims`/`auto_dimensions` é justamente o que não se parece.

---

## Sem como reproduzir agora

**O `fix` que desmancha o móvel** (antigo caso 5). Na versão anterior, o `fix`
de um alerta mandava mover a lava-louças 17 cm — para fora do nicho de
marcenaria em que estava embutida — e isso *melhorava* a nota, sem que
`outgrew_niche` fosse consultado. Hoje a planta não tem nenhum finding com
`fix` oferecido, então não dá para dizer se mudou. Fica anotado para a próxima
planta que ofereça um.

---

## O que já caiu

Verificado em uso, não no changelog. O texto de cada um está no histórico do
git, com os números e as respostas literais.

| # | O que doía | Caiu em |
|---|---|---|
| 1 | Dry run não dizia o tipo do problema que ia criar | `kind` no `issues_new` |
| 2 | Folga `0` valia para "encostado" e "enfiado dentro" | folga negativa |
| 3 | Colisão por caixa sem como declarar convivência | `accept` em `overlap` |
| 4 | `ergonomics` media pelo lado oposto ao que a peça abre | face construída |
| 6 | A folga era o pior ponto, sem a extensão | "54 cm em 34,5 dos 185" |
| 7 | Os números no nome das peças não eram conferidos | `stale` confere nomes |
| 8 | Análises só atrás do MCP; `bounds` sem ângulo no REST | `/api/*` + bounds |
| 9 | Sem `accept` no `check_layout` | `accept` + `orphaned` |
| 10 | `stale` calado não distinguia "tudo certo" de "nada conferido" | `checked: {...}` |
| 11 | `accept` sobrevivia ao achado que o justificava | `orphaned` |
| 12a | `scope=project` ignorava o `q` | `q` respeitado |
| 12b | Score do dry usava a ocupação padrão | usa a da revisão |
| 13 | Cota sem âncora nunca ficava stale | `dims_unanchored` |
| 14 | Rótulo ficava apontando para o vazio após `delete` | `labels_left` |
| 15 | "80,5 cm" era lido como `5` — 28 de 50 achados falsos | vírgula decimal |
| 16 | Resize de grupo invertia a face declarada | face preservada |
| 18 | Parte de grupo não podia ser renomeada | `name` aceito |
| 19 | A sonda atravessava um armário de 280 cm | lê as partes |
| 20 | `cut_list` só enxergava o que o `joinery` fez | `drawn` + `skipped` |
| 22 | Erro de tipo não dizia qual campo | nomeia o campo |
| 24 | Peça a 2,72 m avaliada como obstáculo de circulação | ignora o que está alto |
