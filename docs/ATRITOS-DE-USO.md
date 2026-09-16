# Atritos de uso

O que dói quando um agente conduz uma planta real pelo MCP, do lado de fora.

Este documento é escrito de fora para dentro: um agente recebe uma planta de
verdade e a tarefa de corrigi-la. Cada vez que uma ferramenta responde menos do
que a pergunta pedia, obriga a um contorno, ou leva a uma conclusão errada
antes de levar à certa, o caso é anotado aqui com o que aconteceu de fato.

Não é uma lista de desejos. Cada entrada traz o que se tentou, o que voltou, e
o que teria encurtado o caminho — com o caso concreto que o produziu, para que
se possa reproduzir. Tudo é anotado aqui, à mão.

O banco de provas é a mesma planta desde o começo: um apartamento de 65 m² com
marcenaria desenhada módulo a módulo, hoje com projeto elétrico e hidráulico
completos, nota 99 e zero colisões.

## Rodada em aberto

Três casos, todos da montagem do elétrico e do hidráulico. São de esforço, não
de resultado: o que se pediu saiu certo, mas custou mais chamadas do que a
intenção tinha.

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

## 42. Um circuito por chamada

`electrical(action="assign")` recebe `ids` e um `circuit`. Os 7 circuitos do
apartamento — C1 e C2 de iluminação, C3 a C7 de tomadas — custaram 7 chamadas,
cada uma com a lista de ids colada à mão.

O conjunto é sempre pensado de uma vez: a divisão em circuitos é uma decisão só,
que separa iluminação de tomadas e isola cozinha, lavanderia e molhados.
Ninguém atribui um circuito hoje e outro semana que vem.

**Deveria:** aceitar um mapa — `{"C1": [...], "C2": [...]}` — numa chamada, um
passo de undo.

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

## 45. Automação não existe no projeto elétrico

O elétrico calcula bem o que é de norma. Testado na planta:

```
electrical(circuits)
→ ["C5", ["TUG"], 4, 1900 VA, 127 V, 15,0 A, 2,5 mm², 16 A, DR true]
   ["C8", ["TUE"], 1, 5500 VA, 220 V, 25,0 A, 4,0 mm², 25 A, DR true]
   main_breaker: {a: 100, load_a: 90,4}
```

Bitola, disjuntor por circuito, DR onde há área molhada e disjuntor geral saem
calculados. Uma carga dedicada é reconhecida sozinha: colocado um
`shower-point`, o circuito virou TUE, passou a 220 V, subiu para 4,0 mm² e 25 A,
e o geral foi de 80 para 100 A.

O que não existe é a camada de automação. O catálogo `electrical` tem tomadas,
interruptores, pontos de luz, quadros, rede, TV, Wi-Fi, campainha, ar e
chuveiro — e mais nada:

```
catalog(q="rele automacao sonoff dimmer sensor") → {"items": []}
```

Numa reforma de hoje isso aparece antes da obra acabar: um relé atrás da
luminária ou do interruptor, um interruptor inteligente, um dimmer, um sensor
de presença no hall, uma fechadura eletrônica. Cada um desses muda o projeto de
verdade — o relé precisa de neutro na caixa do interruptor, que a instalação
antiga costuma não ter; o dimmer limita a carga que pode pendurar; o sensor
quer altura e ângulo; e todos consomem em espera, o que entra no cálculo.

Sem isso, o projeto sai "de norma" e ainda assim incompleto para quem vai
morar: a decisão de automatizar acaba tomada na obra, com o eletricista, longe
do desenho.

**Deveria:** peças de automação no catálogo (relé, interruptor inteligente,
dimmer, sensor de presença, fechadura), com o que cada uma exige — neutro na
caixa, carga mínima e máxima, altura — entrando nas verificações como o resto
já entra. E o `assign` aceitando o consumo de espera, para que apareça na carga
total.

## 46. O ponto de Wi-Fi é um símbolo: não há cobertura, banda nem alimentação

Colocado um access point no forro da sala:

```
place(cat="wifi-point", at=[300,600], elev=270)
electrical(check) → points: {…, "Wi-Fi": 1}, findings: []
```

Ele é contado e mais nada. A peça guarda largura, altura, elevação e
disciplina — nenhuma potência, nenhuma banda, nenhum padrão:

```
{catalog: "wifi-point", width: 16, depth: 16, height: 4,
 elevation: 270, discipline: "electrical"}
```

Três perguntas que a planta não responde:

**Onde pôr.** Qual cômodo cobre melhor os 65 m², quantos pontos são precisos,
onde o sinal morre. A casa tem paredes de 11 a 20 cm, dois shafts de alvenaria
e um box de vidro — cada um atenua de um jeito, e 2,4 GHz, 5 GHz e 6 GHz
atravessam de forma bem diferente: a banda que dá velocidade é a que menos
passa parede. Sem isso, decidir o lugar do roteador é chute.

**O que ele precisa para funcionar.** O access point pediu, em silêncio, um
cabo de dados e alimentação — PoE vindo do switch, ou uma tomada no forro. O
`check` não cobrou nenhum dos dois, embora cobre cabo para cada ponto de rede e
de TV (*"Pontos sem cabo chegando: f1555, f1550, f1567"*). O ponto de Wi-Fi
ficou de fora dessa conta.

**Com o que ele fala.** Não há onde registrar o padrão (Wi-Fi 5, 6, 6E, 7), a
banda, o canal, nem se o backbone até ele é Cat 6 ou Cat 6A — que é o que
decide se 6 GHz vale a pena.

O contraste está dentro do próprio app: o `lighting` faz photometria de
verdade — lança o fluxo de cada luminária, deixa as paredes fazerem sombra,
soma a interreflexão e devolve lux e uniformidade por cômodo, contra a
NBR 8995. Propagação de rádio com atenuação por parede é o mesmo problema, com
outra constante; o motor que resolve um resolveria o outro.

**Deveria:** `wifi-point` com padrão, banda e potência; uma cobertura por
cômodo como a do `lighting` (dBm em vez de lux, atenuação por material da
parede, por banda); o `check` cobrando cabo de dados e alimentação do access
point como já cobra dos outros pontos; e o cabeamento sabendo distinguir Cat 6
de Cat 6A, que é o que sustenta as bandas altas.

---

## Conferido nesta rodada

Cada um testado na planta, com o comando que o reproduzia.

| # | O que doía | Como está agora |
|---|---|---|
| 36 | "Banho suíte" virava dormitório e pedia rede e TV num banheiro de 3 m² | `electrical(check)` → `findings: []`, com o cômodo ainda chamado "Banho suíte" |
| 37 | Planta sem tomada nenhuma tirava 99, a mesma nota da completa | Apagadas as 22 tomadas: score **67** e nove findings, um por cômodo. Restaurado por checkpoint: 99 |
| 38 | Quantitativo agrupava por nome: 31 linhas de "1" | Agrupa por tipo **e** lista os nomes: `["Tomada baixa (30 cm)", 13, [...]]`, `["Ponto de água fria", 10, [...]]` |
| 39 | Luminária nascia sem `lm`, `w` nem `lamp` | `place(cat="light-ceiling")` já vem com `{lumens: 1000, lamp: "led", kelvin: 3000, watts: 10}` |
| 41 | `electrical` não tinha `accept`, e o projeto não fechava | Os findings trazem chave (`elec:outlets:r46`), e o check reporta `pending` e `orphaned` |
| 43 | Ponto de projeto era cobrado como a louça que servia | `place(cat="sewer", name="Esgoto — vaso banho social")` → nenhum finding. Os 27 pontos voltaram a ter nome descritivo |

O 43 tinha um custo que também foi desfeito: a nomenclatura da hidráulica havia
sido reduzida a código e ambiente para calar os findings. Agora cada ponto diz o
aparelho que serve — `AF-05 — lavatório, banho suíte`, `ES-06 — lava-louças,
cozinha` —, que é o que o instalador lê na obra.

## O que já caiu

Quarenta casos, todos verificados em uso na mesma planta, não no changelog. A
lista com o que doía em cada um e onde foi resolvido está no histórico do git.

Os que mais mudaram o trabalho: a folga negativa no lugar do `0` ambíguo; a
extensão junto da folga (`"54 cm em 34,5 dos 185 cm"`), que separa um móvel
inutilizável de um canto apertado; o `ergonomics` medindo pelo lado em que a
peça abre; o `cut_list` alcançando o que foi desenhado à mão e declarando o que
pulou; o grupo que mantém a espessura das chapas e faz crescer o vão; o número
da peça preso à peça; o `no_door` para cômodo sem acesso; e agora o quantitativo
agrupado por tipo e o ponto de disciplina que deixou de ser lido como móvel.
