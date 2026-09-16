# Atritos de uso

O que dói quando um agente conduz uma planta real pelo MCP, do lado de fora.

Este documento é escrito de fora para dentro: um agente recebe uma planta de
verdade e a tarefa de corrigi-la. Cada vez que uma ferramenta responde menos do
que a pergunta pedia, obriga a um contorno, ou leva a uma conclusão errada
antes de levar à certa, o caso é anotado aqui com o que aconteceu de fato.

Não é uma lista de desejos, e não é crítica da planta. Cada entrada traz a
chamada que se fez, a resposta literal, o que era verdade, o que custou e o que
teria encurtado o caminho. Tudo é anotado aqui, à mão.

O banco de provas é a mesma planta desde o começo: um apartamento de 65 m² com
marcenaria desenhada módulo a módulo. Nesta rodada ele fechou com **nota 100**,
zero colisões e `pending: 0` nas três disciplinas — elétrica (22 tomadas, 13
pontos de rede, 7 circuitos, quadro de 24 módulos com 3 de reserva, 3 peças de
automação e um access point com cobertura calculada), hidráulica (28 pontos, 3
colunas de ventilação, 57 m de tubo traçados pelo `route`) e arquitetura.

## Rodada em aberto

Dez casos novos, todos nascidos das ações desta rodada: fechar a hidráulica,
completar o cabeamento estruturado, dimensionar o quadro, reaceitar o que a
versão nova reabriu e, depois que o usuário desconfiou, auditar a planta peça
por peça atrás do que estava solto no ar ou na camada errada. Nenhum deles
impediu o trabalho; todos custaram chamadas, leitura de código-fonte, scripts
auxiliares ou um passo desfeito.

## 48. A aceitação some quando a chave do achado muda de prefixo

Duas passagens de cama estavam aceitas há rodadas, com motivo escrito. Na
versão nova elas voltaram como achados abertos, e as aceitações apareceram
assim:

```
ergonomics(city="sao-paulo")
→ "orphaned": [["-:f807:livres-frente-passagem-cama", "13 cm em 50 dos 86 cm …"],
               ["-:f810:livres-frente-passagem-cama", "Mesma situacao: …"]]
   findings:  {"key": "nbr15575g:f807:livres-frente-passagem-cama", …}
              {"key": "nbr15575g:f810:livres-frente-passagem-cama", …}
```

A regra passou a citar a fonte, e a chave ganhou o prefixo `nbr15575g:`. É a
mesma regra, sobre a mesma peça, com o mesmo sufixo — e a aceitação virou órfã.
`orphaned` listou as duas, os achados novos apareceram do lado, e nada disse
que um era o outro. Quem não estivesse relendo o arquivo inteiro atrás disso
veria só a nota cair.

**Reproduzir:** aceitar um achado, mudar a fonte da regra, rodar o check.

**Deveria:** casar órfã e achado novo pelo sufixo (peça + regra) e reaproveitar
o motivo, dizendo que a chave mudou — ou, no mínimo, apontar no `orphaned` o
achado que provavelmente o sucedeu. A chave é identidade; se ela carrega a
fonte, ela muda quando a fonte muda.

## 49. `update` aceitou um campo que não existe e respondeu "nada mudou"

Para escrever a capacidade do quadro e o curto presumido, o caminho óbvio era o
`update`:

```
update(items=[{"id": "f1576", "props": {"elec:modules": 24, "elec:short_ka": 10}}])
→ ok rev=14 {"unchanged": [{"id": "f1576", "now": {…}}],
   "unchanged_note": "these already had the values asked for; nothing was changed on them"}
```

Nada foi escrito. O `props` foi descartado em silêncio e a resposta afirmou o
contrário do que aconteceu: *"these already had the values asked for"*. A peça
nunca teve essas propriedades — nem tem como ter por aí.

O caminho certo estava na descrição do `electrical`, num parágrafo de 3.000
caracteres: `assign(ids=[…], modules=24)` e `voltage(short_ka=10,
earthing="TN-C-S")`. Achei lendo `crates/newera-mcp/src/tools/electrical.rs`.

**Custou:** duas chamadas erradas (uma delas com uma afirmação falsa como
resposta) e uma ida ao código-fonte.

**Deveria:** recusar campo desconhecido — `update: no field "props"; panel
properties are written by electrical(assign modules=…) and
electrical(voltage short_ka=…)`. Um "nada mudou" precisa ser verdade; quando a
chamada não escreveu porque nem entendeu o pedido, isso é erro, não igualdade.

## 50. Dois achados que se anulam, e o `fix` que anda em círculo

Restava uma dica de peso 1 no sofá:

```
"36 cm livres à frente (sentar, levantar e circular diante do assento:
 mínimo 50 cm); afaste 14 cm."   f933, peso 1
```

Afastei os 14 cm pedidos. Na volta:

```
"71 cm livres à frente (circulação diante de bancada e equipamentos:
 mínimo 85 cm); afaste Sofá Milano … f933 14 cm."   f1068, peso 5
 "fix": {"tool": "move", "ids": ["f933"], "dx": 0.0, "dy": 14.0}
```

O `fix` que veio é exatamente o movimento inverso do que eu acabara de fazer.
Os dois achados disputam o mesmo vão entre a península da cozinha e o rack da
sala: 50 cm na frente do sofá mais 85 cm na frente da bancada dão 135 cm, e o
vão não tem. Não existe posição que satisfaça os dois, e nenhuma das duas
mensagens diz isso.

**Custou:** um `move`, um check, um `move` de volta, e a nota oscilando entre
87 e 99 no caminho.

**Deveria:** quando o `fix` de um achado criaria outro de peso igual ou maior,
dizer na própria mensagem — *"não cabem os dois: as duas folgas somam 135 cm e
o vão não tem; a posição atual é a melhor das duas"*. Um achado que só pode ser
aceito deveria nascer sabendo disso.

## 51. O `route` redesenha, mas o traçado anterior fica na planta

A hidráulica tinha 8 polilinhas traçadas à mão em rodadas anteriores. O `route`
novo traçou tudo sozinho:

```
plumbing(action="route", kind="cold",  via="floor") → 21,8 m, rev=4
plumbing(action="route", kind="hot",   via="floor", from="f801") → 17,3 m, rev=5
plumbing(action="route", kind="sewer", via="floor") → 18,1 m, rev=6
plumbing(check) → "pipes_m": {"cold": 21.8, "hot": 17.3, "sewer": 18.1},
                  "orphaned": []
```

`pipes_m` conta só os traçados novos — mas os 8 antigos continuam desenhados,
por cima, no mesmo layer `plumbing`. Não aparecem no `pipes_m`, não aparecem no
`orphaned`, não aparecem em finding nenhum. Quem imprimir a planta aí recebe
dois ramais paralelos para cada ponto.

Descobri comparando `polylines` do `/api/home` com os metros do check, e limpei
com `delete(ids=["pl1615", …, "pl1622"])`.

**E aconteceu de novo na elétrica, pior.** Depois de reassentar os pontos,
rodei `route` de força, dados e TV. As 9 polilinhas `elec:cable` desenhadas à
mão continuaram lá, agora apontando para coordenadas onde não há mais ponto
nenhum — `[135,620]`, `[100,350]`, `[600,244]`. Cabos fantasmas para pontos que
foram embora, invisíveis para o `check`, que só contou os 36,7 m do run novo.

**Reproduzir:** desenhar um ramal com `cable`/polilinha, depois rodar `route`
da mesma espécie.

**Deveria:** o `route` substituir o traçado anterior da mesma espécie (é o que
a descrição do `electrical route` promete — *"drawing it replacing the earlier
run of the same circuit"* —, e no `plumbing` não aconteceu), ou dizer na
resposta quantas polilinhas da disciplina ficaram fora do run, com os ids.

## 52. `move` só anda por delta

Toda leitura devolve posição absoluta: `{"id": "f933", "at": [515, 630]}`. Para
mover, não:

```
move(ids=["f933"], to=[515, 616])
→ failed to deserialize parameters: missing field `dx`
```

O jeito é `move(ids=["f933"], dx=0, dy=-14)`. Numa peça só é subtração de
cabeça; em cinco peças que precisam ir cada uma para um lugar lido antes, são
cinco subtrações feitas por fora, e um sinal trocado move o móvel para o lado
errado sem erro nenhum.

**Deveria:** aceitar `to` como alternativa a `dx`/`dy` — a mesma unidade e o
mesmo sistema que toda leitura devolve. E a mensagem de erro dizer o que
existe (`move takes dx/dy, or to`), não só o campo que faltou.

## 53. Achado que desenho nenhum fecha

Seis achados desta rodada só puderam ser encerrados com `accept`, porque não há
peça que os represente.

**Ventilação permanente de gás.** O achado é correto e sério:

```
"Aparelho a gás: janela que fecha não é ventilação permanente. Preveja abertura
 permanente direta para o exterior (veneziana ou grelha, inferior e superior)"
```

Só que a grelha não existe:

```
catalog(q="grelha veneziana ventilação") → {"items": [["vent-pipe", …]]}
catalog(q="air vent grille louver abertura permanente")
  → poltrona, cadeira, escada, ponto de ar-condicionado, tubo de ventilação
```

Em `crates/newera-ergonomics/src/lib.rs:1524` a regra dispara sempre que há
aparelho a gás e `glass > 0`. Não há estado do desenho que a apague.

**Ramal ventilador de esgoto.** Depois de plantar as três colunas
(`vent-pipe`), sobraram cinco `plumb:vent-far` medindo distância em linha reta
até a coluna mais próxima — 139 cm no lavatório da suíte, 312 cm na pia da
cozinha. Todos se resolvem na obra com ramal ventilador de 40/50 mm correndo
dentro da parede ou por cima dos armários, que é o que a NBR 8160 manda. Só que
a única peça de ventilação é a coluna: não há como desenhar o ramal, e não há
como abrir furo novo na laje de um apartamento para satisfazer a distância.

**Custou:** seis `accept` com motivo escrito à mão, para achados que eu
preferiria ter fechado desenhando.

**Deveria:** uma peça de abertura permanente (grelha/veneziana, com área útil)
que a regra de gás reconheça; e um ramal ventilador — uma polilinha da
disciplina, como os ramais de água — que o `plumb:vent-far` aceite como
percurso, já que a própria mensagem diz *"em linha reta; confira pelo
percurso"*. A ferramenta sabe que a medida dela é uma aproximação, e não dá o
caminho de dar a medida certa.

## 54. `check_layout` afoga três casos reais em 49 aninhamentos esperados

Depois de plantar 10 pontos de rede, 3 de automação, 1 de Wi-Fi e 3 colunas de
ventilação:

```
check_layout() → "overlap_kinds": {"nesting": 49}
```

Dos 49, 46 são por construção: o ponto de água fria está dentro do lavatório
que ele serve, o de esgoto dentro do vaso, o de gás dentro do gaveteiro sob o
cooktop, a coluna de ventilação dentro do shaft. Um ponto de disciplina fora da
peça que ele serve é que seria erro.

Os 3 que importavam estavam no meio da lista, com o mesmo `kind` e o mesmo peso
visual:

- `f1660`, RJ45 da suíte, dentro da cama;
- `f1665` e `f1666`, RJ45 da lavanderia, dentro da lava-e-seca e do gavetão,
  porque `network-outlet` nasce com elevação baixa e a bancada está a 87 cm.

Achei lendo a lista inteira de 49 linhas, uma a uma.

**Deveria:** não reportar como aninhamento o ponto de disciplina contido na
peça que ele serve — ou separá-los em `nesting: {expected, unexpected}`. E o
ponto de rede sobre bancada nascer na altura de quem o usa, como o
`outlet-mid` já faz.

## 55. `route hot` pede a origem e não diz que ela é um id

```
plumbing(action="route", kind="hot", via="floor")
→ route hot: where does hot water come from? name the heater (aquecedor) or give from
plumbing(action="route", kind="hot", via="floor", from=[665, 266])
→ from: [665, 266] is not a piece of this storey
```

A primeira mensagem diz "name the heater **or give from**", o que se lê como
"dê o ponto de onde vem". `from` é o id de uma peça. A segunda mensagem só diz
o que aquilo não é.

**Custou:** uma chamada.

**Deveria:** a primeira mensagem já dizer o formato — *"give `from` as the id
of the piece it starts from (a heater, a shaft, the column)"* —, como o `route`
do `electrical` faz na descrição.

## 56. Nada avisa que um ponto ficou solto no ar

O usuário desconfiou que havia peças flutuando. Havia, e nenhuma ferramenta
disse.

Sonda: uma tomada pedida a 30 cm de qualquer parede.

```
place(cat="outlet-low", at=[400,40], name="PROBE")     → ok rev=39
/api/home → {"id":"f1706","position":[400,40],"bounds":[[395,38],[405,42]]}
```

Ela ficou exatamente onde foi pedida, boiando. O `electrical(check)` contou-a
como ponto existente, o `check_layout` não a listou, e a nota não se moveu.
Medindo os 79 pontos de disciplina contra a face da parede mais próxima, com
um script escrito para isso:

| Situação | Pontos |
|---|---|
| a 4–7 cm da face da parede (toda a elétrica e a hidráulica de parede) | 45 |
| sem nenhuma parede ao alcance, soltos no meio do cômodo | 4 |
| dentro de um armário ou móvel, no ar | 3 |
| pendurados abaixo do teto (Wi-Fi, sensor, relé) | 3 |

Três exemplos do que isso significa na obra: a `Tomada — sala, parede oeste`
estava a 96 cm da parede mais próxima, numa divisa que é vidro do escritório;
o `Rede RJ45 — escritório` a 119 cm, no meio do cômodo; o relé inteligente
dentro do armário aéreo da varanda, a 250 cm do piso.

O `check_layout` tem `outside_rooms`, `in_wall`, `overlap` e `turned` — quatro
relações — e nenhuma chamada "não está apoiado em nada". As 49 sobreposições
que ele reporta são todas `nesting` esperado (o ponto de água dentro do
lavatório, o cooktop dentro da bancada), o que torna a lista o lugar errado
para procurar.

**Custou:** um script de 40 linhas para medir ponto a ponto contra as paredes,
outro para procurar apoio em piso, teto, parede ou peça, e 24 chamadas de
`move`/`update` para assentar tudo.

**Reproduzir:** `place` de qualquer ponto de parede com um `at` que não esteja
exatamente na face.

**Deveria:** o `place` assentar o ponto na estrutura — é o que
`crates/newera-core/src/mounting.rs` faz no repositório (`seat`, commit
`6b9f021`), e o app instalado aqui ainda não tem. Até chegar, o `check_layout`
poderia ter uma relação `loose`: peça que não toca piso, teto, parede nem
outra peça. Foi a pergunta que eu precisei responder, e a única forma foi por
fora.

## 57. A camada automática segue a primeira palavra do nome, não a peça

O usuário apontou dois casos, e os dois eram o mesmo defeito.

**O aéreo da geladeira não virou marcenaria.** `Aéreo geladeira — 79,9 cm;
ventilação inferior preservada` é um armário planejado, e saiu em
`plan_layer: "appliances"`. Em `crates/newera-core/src/layers.rs:166` as
palavras de eletrodoméstico são testadas antes das de marcenaria, e
"geladeira" ganha de "aereo".

**O armário da coifa perde as laterais.** O grupo `f1067` — o caixote de
madeira oliva que embute a coifa, com molduras iguais às dos aéreos — está em
`joinery`, certo. Mas 14 das suas peças estão em `appliances`, porque o nome
de cada uma começa com "Coifa —":

```
f1049 "Coifa — lateral oliva"          → appliances
f1051 "Coifa — painel oliva rebaixado" → appliances
f1063 "Coifa — veneziana superior oliva" → appliances
f1056 "Coifa — puxador pequeno dourado" → appliances
…
f1052 "Coifa — moldura vertical oliva" → joinery   (só porque tem "moldura")
```

O efeito é visível: esconder a camada de eletrodomésticos tira as laterais, os
painéis e a veneziana, e deixa as molduras penduradas no ar em volta da coifa.
Um armário que some pela metade.

A regra em `layer_in_group` diz que a peça com camada própria vence a do grupo
— boa ideia para o forno embutido na torre. Mas a camada própria aqui foi
adivinhada pelo nome, e o nome descreve *o que a peça embute*, não o que ela é.

**Custou:** replicar `layer_of` num script para varrer as 213 peças, e um
`update(layer=...)` com 19 ids.

**Deveria:** dentro de um grupo, a peça só sair da camada do grupo quando a
camada dela for escrita à mão (`plan:layer`), não quando for adivinhada pelo
nome; e, fora de grupo, as palavras de marcenaria pesarem mais que as de
eletrodoméstico quando as duas aparecem ("aéreo geladeira", "nicho do forno",
"torre da lava-louças" são todos marcenaria).

---

## Conferido nesta rodada

Os seis casos que estavam em aberto foram testados na planta, não no changelog.
Todos caíram.

| # | O que doía | O comando desta rodada | O que voltou |
|---|---|---|---|
| 40 | Cabear era digitar coordenada por coordenada: 9 chamadas de `cable` guiadas pelo alerta do check | `electrical(action="route", kind="data")` e `kind="tv"` | Uma chamada por espécie. 34,2 m de percurso, `by_premise_m` das três premissas (parede e forro recusados nomeando os pontos), e a lista de material: 35,9 m de eletroduto, 15 curvas, 149,6 m de Cat 6, 14 keystones, 14 portas de patch panel, 14 caixas 4×2 |
| 42 | Um circuito por chamada, 7 chamadas para uma decisão só | `electrical(action="assign", ids=[…], circuit="C2")` com os 3 ids de automação | Aceita o mapa `{"C1": […], "C2": […]}` inteiro num passo de undo; `assign` também escreve `modules`, `standby_w`, `max_w`, `volts` e `va` |
| 44 | A hidráulica não tinha quem conferisse: 27 pontos decididos na mão | `plumbing(check)` e `plumbing(route)` das três espécies | O check achou 6 problemas reais que eu não tinha visto — os dois lavatórios sem água quente, a cozinha sem ralo (COE-SP), o esgoto sem ventilação, sem caixa de inspeção e sem caixa de gordura. Corrigidos com 4 peças novas e 2 aceites motivados. O `route` de esgoto sai com caimento de 1 %, tronco de 100 mm, Y de 45° no lugar de tê, `needs_depth_cm: 31` e 16 linhas de material |
| 45 | Automação não existia no projeto elétrico | `place` de `dimmer`, `presence-sensor` e `smart-relay`, depois `electrical(check)` e `circuits` | As três peças entraram, contadas como `"Automação": 3`. O check cobrou neutro na caixa do relé citando fabricante (Shelly, Exatron, Qualitronix), não a 5410, e cobrou circuito para as três. Nenhum falso positivo de altura no sensor nem de carga no dimmer |
| 46 | O ponto de Wi-Fi era um símbolo: nem cobertura, nem banda, nem alimentação | `electrical(action="wifi")`, depois com `ids`, `standard`, `poe`, `uplink` | Sugeriu sozinho o lugar: `[[447, 374.5, 270, "Cozinha"]]`, Wi-Fi 6, 5 GHz. Plantado ali, devolveu cobertura por cômodo e banda em dBm — mediana, pior caso, fração acima de −67 e nota em português. Só o banho da suíte fica fraco em 5 GHz, e o 2,4 cobre. O check passou a cobrar cabo de dados e alimentação do AP |
| 47 | O quadro não tinha capacidade, e a proteção parava no disjuntor | `electrical(assign, modules=24)`, `voltage(short_ka=10, earthing="TN-C-S")`, `circuits` | `panel` com os dispositivos e os módulos DIN de cada um, `modules: {capacity: 24, capacity_written: true, used: 21, spare: 3}`, DPS classe II com Up ≤ 1,5 kV e N-PE ≥ 10 kA, aterramento TN-C-S, `icn_ka: 10`, `selective: true` e `main_breaker: {a: 63, phases: 1, load_a_per_phase: 57.6}` |

A auditoria das normas anunciada na rodada anterior também bateu na planta, com
os efeitos que ela previa: o círculo da cozinha passou de 1,20 m para 1,50 m
(COE-SP, tabela 5.A.6) e reabriu como erro; a carga de iluminação virou carga
de cômodo; a NBR 16264 entrou por cômodo e gerou as dicas de rede e TV que
levaram aos 10 pontos novos; a ventilação de esgoto e o ralo seco da cozinha
apareceram como a tabela prometia. Nada disso virou caso aqui — foi mudança
anunciada, e o único atrito que sobrou dela está no **48**.

## O que já caiu

Quarenta e seis casos, todos verificados em uso na mesma planta, não no
changelog. A lista com o que doía em cada um e onde foi resolvido está no
histórico do git.

Os que mais mudaram o trabalho: a folga negativa no lugar do `0` ambíguo; a
extensão junto da folga (`"54 cm em 34,5 dos 185 cm"`), que separa um móvel
inutilizável de um canto apertado; o `ergonomics` medindo pelo lado em que a
peça abre; o `cut_list` alcançando o que foi desenhado à mão e declarando o que
pulou; o grupo que mantém a espessura das chapas e faz crescer o vão; o número
da peça preso à peça; o `no_door` para cômodo sem acesso; o quantitativo
agrupado por tipo; o ponto de disciplina que deixou de ser lido como móvel; e
agora as três disciplinas com o mesmo par `check` + `route`, que é o que
transformou a hidráulica de adivinhação em trabalho conferido.
