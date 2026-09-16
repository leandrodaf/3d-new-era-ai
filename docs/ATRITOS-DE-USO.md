# Atritos de uso

O que dói quando um agente conduz uma planta real pelo MCP, do lado de fora.

Este documento é escrito de fora para dentro: um agente recebe uma planta de
verdade e a tarefa de corrigi-la. Cada vez que uma ferramenta responde menos do
que a pergunta pedia, obriga a um contorno, ou leva a uma conclusão errada
antes de levar à certa, o caso é anotado aqui com o que aconteceu de fato.

Não é uma lista de desejos. Cada entrada traz o que se tentou, o que voltou, e
o que teria encurtado o caminho — com o caso concreto que o produziu, para que
se possa reproduzir.

Este arquivo é o registro: tudo é anotado aqui, à mão, com o caso inteiro.

## Rodada em aberto

Projeto elétrico e hidráulico montados do zero na mesma planta: 22 tomadas,
21 pontos de luz dimensionados, 4 de rede, 3 de TV, dois quadros, 7 circuitos
pela NBR 5410 e 58 m de cabo; depois 27 pontos hidráulicos e 8 ramais. Os casos
abaixo saíram do que foi preciso fazer para chegar lá.

## 36. O cômodo é classificado pelo nome, e "Banho suíte" vira dormitório

Com o projeto elétrico completo, sobravam dois findings, os dois no banheiro da
suíte — 3,12 m², com box, vaso e lavatório:

```
["alerta","Banho suíte r41","Sem ponto de rede: um cômodo de permanência
  pede ao menos uma tomada RJ45 …","nbr14565"]
["dica","Banho suíte r41","Sem ponto de TV: salas e dormitórios costumam
  ter um ponto coaxial junto ao rack ou à parede da cama.","nbr14565"]
```

O `Banho social`, mesma função e mobiliário, nunca recebeu nenhum dos dois. A
diferença entre os dois cômodos é a palavra "suíte" no nome. Provado trocando
só isso:

```
update(items=[{"id": "r41", "name": "Banheiro 2"}])
electrical(check) → findings: []
```

Nada mais mudou — nem geometria, nem ponto, nem peça. O nome voltou a ser
"Banho suíte", que é o certo e é o que aparece no schedule e nas cotas, e os
dois findings voltaram com ele.

**Reproduzir:** os dois comandos acima, num banheiro cujo nome contenha
"suíte".

**Deveria:** classificar pela função — vaso, box ou lavatório fazem um
banheiro, mesmo com "suíte" no nome. Mantendo que a suíte de dormir (`r39`,
"Suíte") continue pedindo rede e TV, que é o caso certo.

## 37. Planta sem tomada nenhuma tira a mesma nota da planta completa

Acompanhando o score enquanto o projeto elétrico era montado:

| estado | tomadas | score |
|---|---|---|
| como estava | 0 em 10 cômodos | **99** |
| meio caminho | 10, cozinha ainda vazia | **75** |
| completo | 22 | **99** |

O pior estado dos três — nenhuma tomada em lugar nenhum — empata com o melhor.
E a etapa intermediária, que já tinha metade do trabalho feito, é a única
punida: começar o projeto elétrico **piora** a nota antes de melhorar.

As regras de tomada só acordam quando existe ao menos uma no projeto; com
contagem zero o cômodo passa em branco. A planta atravessou várias rodadas de
revisão com 99 e sem uma tomada sequer, e nada no `ergonomics` levantou a mão —
quem apontava era o `electrical(check)`, que é preciso saber que existe.

**Reproduzir:** `ergonomics` numa planta sem pontos de tomada; depois
acrescentar tomadas em parte dos cômodos e rodar de novo.

**Deveria:** a contagem zero valer como o caso pior, não como neutro. Quando a
ausência for proposital, o caminho é o `accept` com o motivo, como nos demais.


<!--
  Modelo de uma entrada:

  ## N. Título que diz o que dói

  O caso, com os números e a resposta que voltou.

  ```
  a resposta literal, quando ela é a prova
  ```

  Por que custou caro, e o que se fez para contornar.

  **Reproduzir:** o comando.
  **Deveria:** a mudança que teria evitado o contorno.
-->

## 38. O quantitativo não agrupa: 31 linhas de "1"

Com o projeto elétrico pronto, o levantamento para a lista de compras:

```
disciplines(action="quantities")
→ ["Tomada — cozinha, sobre a bancada (centro)", 1]
  ["Tomada — cozinha, sobre a bancada (direita)", 1]
  ["Tomada — dormitório, cabeceira", 1]
  …  31 linhas, todas com contagem 1
```

O agrupamento é pelo **nome** da peça. Como cada ponto tem nome descritivo do
lugar onde fica — que é o que serve ao eletricista na obra —, nada agrupa. O
mesmo acontece em `annotations(legend=true)`, que repete as 31 linhas.

O dado certo existe e sai no mesmo instante, da outra ferramenta:

```
electrical(check) → points: {"Iluminação":21, "TUG":22, "Rede":4, "TV":3}
```

O incentivo fica invertido: para o `quantities` funcionar seria preciso dar o
mesmo nome genérico a todas as tomadas, perdendo a indicação de onde cada uma
vai.

**Reproduzir:** `disciplines(action="quantities")` numa planta cujos pontos
tenham nomes próprios.

**Deveria:** agrupar por catálogo (`outlet-low`, `outlet-mid`,
`network-outlet`…), com o nome como detalhe da linha.

## 39. As luminárias vieram sem fluxo, e só a photometria conta

As 21 luminárias da planta — as mesmas que o `electrical` conta como pontos de
iluminação e distribui nos circuitos C1 e C2 — estavam **sem potência nenhuma**:

```
f870  Cozinha — geral        light: {lm: None, w: None, lamp: None}
f865  Escritório — geral     light: {lm: None, w: None, lamp: None}
…  todas as 21
```

O resultado, pela NBR ISO/CIE 8995-1:

```
lighting() → Escritório  24 lx (referência 500)  "abaixo: faltam 476 lx"
             Cozinha     19 lx (referência 300)  "abaixo: faltam 281 lx"
             Dormitório  15 lx (referência 150)  "abaixo: faltam 135 lx"
```

Dez cômodos, dez vezes "abaixo". A casa inteira com 5.184 lm, quando precisa de
uns 34.000.

E o `ergonomics` dava 99 o tempo todo. É o mesmo padrão do caso 37 com as
tomadas: a ferramenta específica aponta, o score não reflete, e quem não sabe
que `lighting` existe entrega a planta assim. Aqui é pior que no 37, porque
lá o buraco era não ter ponto nenhum; aqui os pontos existem, estão desenhados,
contados e distribuídos em circuito — só não iluminam.

**Reproduzir:** `lighting()` numa planta cujas luminárias não tenham `lm` nem
`w` definidos.

**Deveria:** luminária sem fluxo entrar no relatório como pendência, no mesmo
lugar em que o cômodo sem porta agora entra (`no_door`). Ou o catálogo trazer
um fluxo padrão por tipo de peça, para que um ponto de luz recém-colocado já
ilumine algo plausível.

## 40. Cabear é digitar coordenada por coordenada

Para levar rede aos 4 pontos RJ45, TV aos 3 coaxiais e os troncos de força, a
partir dos dois quadros:

```
electrical(action="cable", kind="data", pts=[[170,750],[320,750]])
→ {"added": ["pl1579"]}
```

E mais oito chamadas iguais, cada uma com a polilinha digitada à mão a partir
das coordenadas lidas antes no `/api/home`. O retorno de cada uma é só o id da
polilinha; quem diz se o cabo chegou é o check seguinte:

```
electrical(check) → ["alerta","Cabo de rede (Cat 6)",
                     "Pontos sem cabo chegando: f1555, f1550, f1567."]
```

Foi esse alerta que guiou as chamadas seguintes, uma de cada vez, até a lista
esvaziar. Nove chamadas para uma intenção só — e uma polilinha que erra o ponto
por um dígito é aceita em silêncio, aparecendo apenas na conferência posterior.

A ferramenta já sabe tudo o que falta para traçar sozinha: onde está cada
ponto, onde está o quadro da espécie, quais pontos estão sem cabo, e como medir
o percurso com a folga das descidas.

**Reproduzir:** cabear qualquer projeto com mais de dois pontos.

**Deveria:** `cable` aceitar ids — `cable(kind="data", ids=[...])` ligando cada
um ao seu quadro. Mantendo a polilinha à mão para quando o percurso é imposto
por shaft, viga ou forro.

## 41. `electrical` não tem `accept`, e o projeto não fecha

`ergonomics` e `check_layout` têm `accept=[[key, motivo]]`: o achado continua
visível com a razão escrita, para de custar nota, e `orphaned` avisa quando a
razão deixa de valer. Foi assim que esta planta chegou a 99 honestamente.

`electrical` não tem. Os dois findings do Banho suíte — que são o caso 36, um
banheiro classificado como cômodo de permanência por causa da palavra "suíte"
no nome — voltam em toda chamada, sem `key` e sem onde registrar que já foram
analisados.

O projeto elétrico não fecha limpo, e quem abrir a planta daqui a seis meses
reinvestiga do zero: não há onde dizer que o motivo é um bug de classificação,
nem que a alternativa seria renomear um cômodo cujo nome está certo.

**Reproduzir:** qualquer finding do `electrical` que não se queira corrigir.

**Deveria:** `accept` com `key` e `orphaned`, igual às outras duas.

## 42. Um circuito por chamada

`electrical(action="assign")` recebe `ids` e um `circuit`. Os 7 circuitos do
apartamento — C1 e C2 de iluminação, C3 a C7 de tomadas — custaram 7 chamadas,
cada uma com a lista de ids colada à mão.

O conjunto é sempre pensado de uma vez: a divisão em circuitos é uma decisão só,
que separa iluminação de tomadas e isola cozinha, lavanderia e molhados.
Ninguém atribui um circuito hoje e outro semana que vem.

**Deveria:** aceitar um mapa — `{"C1": [...], "C2": [...]}` — numa chamada, um
passo de undo.

## 43. Ponto de projeto é avaliado como a louça que ele serve

Lançando os 27 pontos hidráulicos com o nome do aparelho que cada um serve,
como se faz em projeto:

```
place(cat="sewer", at=[126,468], name="Esgoto — vaso banho social")
→ ["alerta","Esgoto — vaso banho social f1589",
   "17 cm livres à frente (uso do vaso: mínimo 60 cm); afaste 43 cm."]
```

Seis findings desses de uma vez, e o score de 99 para 69. Um ponto de esgoto de
10 × 10 × 5 cm sendo cobrado por circulação de vaso sanitário — ele fica
justamente atrás do aparelho, que é onde deve ficar.

Provado trocando só o nome, mesma peça e mesmo lugar:

```
place(cat="sewer", at=[126,468], name="ES-01")   → nenhum finding
```

É o mesmo mecanismo do "Banho suíte" que vira dormitório: a regra lê o nome. O
catálogo (`sewer`, `cold-water`, `floor-drain`) já diz o que a peça é.

**O custo foi a nomenclatura.** Tive que abandonar os nomes descritivos e usar
código e ambiente — `AF-01 — banho social`, `ES-05 — cozinha`. Num banheiro com
três pontos de água fria, "AF-04, AF-05, AF-06 — banho suíte" não diz qual é do
vaso, qual do lavatório e qual do chuveiro. É o que o instalador lê na obra.

**Reproduzir:** qualquer ponto de disciplina cujo nome cite a louça que serve.

**Deveria:** peça de catálogo de disciplina não entrar nas regras de circulação
e uso de móvel. O catálogo tem precedência sobre o nome.

## 44. A hidráulica não tem quem confira

O elétrico foi montado guiado a cada passo: `electrical(check)` apontava cômodo
a cômodo quantas tomadas faltavam, depois listava os pontos sem cabo chegando,
até a lista esvaziar. Foi o que tornou 22 tomadas, 7 circuitos e 58 m de cabo
um trabalho seguro.

A hidráulica não tem equivalente. Existe o catálogo (`cold-water`, `hot-water`,
`sewer`, `floor-drain`, `valve`, `grease-trap`, `inspection-box`,
`water-meter`, `gas-point`) e a camada funciona — as 8 polilinhas entraram em
`plumbing` sozinhas e `lines_cm` passou a `{"electrical":5247,
"plumbing":2483.6}`. Mas não há `plumbing(check)`.

Os 27 pontos saíram de conferência própria: ler a lista de louças e decidir na
mão o que cada uma pede. Se eu tivesse esquecido o ralo de um banheiro ou a
água quente da pia, nada teria dito. Das três disciplinas, a hidráulica é a
única que sai sem ninguém ter conferido.

**Deveria:** um `plumbing` espelhando `electrical` — um ponto de água e um de
esgoto por louça, ralo em área molhada, caixa de gordura na cozinha, ramal
chegando a cada ponto —, apoiado nas NBR 5626, 8160 e 13103, esta última já
citada pelo `ergonomics`.

---

## Conferidos na planta real

Os dois que restavam foram conferidos com o app instalado, sobre uma cópia da
planta de 65 m² tirada da sessão aberta, com os mesmos comandos do relato:

| # | O que doía | Resultado |
|---|---|---|
| 32 | Cômodo sem porta não entrava em relatório nenhum | Com as portas: `no_door` vazio, nota 99. `delete` da porta do dormitório (`f1546`): `no_door: Dormitório`, `ergonomics` com erro "Sem acesso", nota 87. (`3f63d0e`) |
| 34 | `hinge_right` respondia ok e parecia não fazer nada | A porta já tinha `hinge_right: true`. Pedir o mesmo valor agora responde `unchanged` com o valor atual, no dry e aplicado; pedir o oposto mostra `true → false`. (`e82752e`, e o `false` explícito no diff) |

Os que não se reproduziam também têm resposta no código: o `fix` que tirava a
lava-louças do nicho não é mais oferecido quando a peça perde o encosto do
nicho (`187cf25`); e toda escrita, dry run, undo e lote pelo REST nomeiam
mudanças nas anotações e nas properties (`afc3d42`).

Nenhum caso aberto. A próxima rodada começa em "Rodada em aberto".

## O que já caiu

Trinta e dois casos, todos verificados em uso na mesma planta de 65 m², não no
changelog. A lista com o que doía em cada um e onde foi resolvido está em
`git show 87083ec:docs/ATRITOS-DE-USO.md`.

Os que mais mudaram o trabalho: a folga negativa no lugar do `0` ambíguo; a
extensão junto da folga (`"54 cm em 34,5 dos 185 cm"`), que separa um móvel
inutilizável de um canto apertado; o `ergonomics` medindo pelo lado em que a
peça abre; o `cut_list` alcançando o que foi desenhado à mão e declarando o que
pulou; o grupo que mantém a espessura das chapas e faz crescer o vão; e o
número da peça preso à peça, que era o mais sério para quem recebe a prancha na
obra.
