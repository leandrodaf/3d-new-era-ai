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

## Resolvidos no código, a conferir na planta

Com resposta no código e no app reinstalado; falta a conferência em uso, com
os comandos abaixo, na mesma planta.

| # | Como conferir | O que deve voltar |
|---|---|---|
| 40 | `electrical(action="route", kind="data")` — ou `cable(kind="data", ids=[...])` | O percurso traçado do quadro de telecom aos pontos, por dentro das paredes ou pela laje/piso, com `by_premise_m` das três premissas, a escolhida (a mais barata que se constrói) e `materials`: eletroduto, curvas, caixas, cabo Cat 5e/6/6A em estrela (um cabo inteiro por ponto, 3 m de sobra no rack) e keystones. Premissa impossível é recusada nomeando os pontos: pela parede, ponto fora de parede; pelo forro, ponto baixo sem parede onde descer. (`2c91f6e`, `e53b25d`, `3630691`) |
| 42 | `electrical(action="assign", circuits={"C1": [...], "C2": [...]})` | A divisão inteira numa chamada e num passo de undo. `assign` aceita também `volts: 220` por ponto. (`2ea1219`) |
| 44 | `plumbing()` e `plumbing(action="route", kind="cold"\|"hot"\|"sewer")` | Numa cópia desta planta o check dá 4 pendências reais: os dois lavatórios sem água quente, a premissa do esgoto (caixa de inspeção ou tubo de queda) e a caixa de gordura. O `route` de água fria sai com tubo, barras, tês, joelhos e registro por ambiente; o de esgoto só por baixo do piso, com caimento, junção 45° (Y) em vez de tê, e com `depth` recusa rebaixo raso dizendo quanto precisa. (`3440f64`, `e96c598`) |
| 45 | `catalog(q="rele automacao sonoff dimmer sensor")`, `place` de `smart-relay`, `smart-switch`, `dimmer`, `presence-sensor`, `smart-lock`; depois `electrical()` e `electrical(action="circuits")` | As cinco peças vêm do catálogo com símbolo de planta. O check cobra neutro na caixa do relé e do interruptor inteligente (aceite quando garantido), a iluminação do cômodo dentro da capacidade do dimmer (`assign max_w`, padrão 200 W) e acima do mínimo, sensor de teto a 2,2 m ou mais e alcançando o canto mais longe, fechadura sobre uma porta. O consumo em espera entra no circuito (`assign standby_w`) sem mudar o tipo dele, e `circuits` mostra `standby_w`. E o `main_breaker` monofásico de 100 A não sai mais: `{a, phases, load_a_per_phase}` — um circuito de 220 V numa rede de 127 V já pede duas fases. (`823a367`, `1f02f28`) |
| 46 | `electrical(action="wifi", ids=["f…"], standard="wifi6e", poe=true, band="6")`, e `electrical()` | Cobertura por cômodo e banda (`[cômodo, banda, mediana dBm, pior dBm, fração ≥ -67, nota]`) pela distância e por cada parede cruzada, conforme o material e a espessura (porta ou janela quando o caminho passa por ela), e a sugestão de onde pôr: o menor número de pontos de teto, nunca em banheiro, varanda ou shaft. Numa cópia desta planta, o AP em [300,600] deixa só o banho da suíte fraco em 5 GHz; para 6 GHz a sugestão é cozinha + sala. O check passa a cobrar do access point o cabo de dados, a alimentação (tomada a até 1,5 m, ou `poe`) e a categoria que sustenta o uplink (Wi-Fi 7 com 10 GbE pede Cat 6A). (`5f3dad1`, `65bb52f`) |
| 47 | `electrical(action="circuits")`; `electrical(action="assign", ids=["<quadro>"], modules=24)`; `electrical(action="voltage", short_ka=6, earthing="TN-S")` | `panel` com os dispositivos e os módulos DIN de cada um — monopolares, bipolares de 220 V entre fases, DR bipolares, geral com um polo por fase, DPS classe II (fase(s) + neutro) e a reserva da NBR 5410 (2 até 6 circuitos, 3 até 12, 4 até 30, 15 % acima) — contra a capacidade do quadro (escrita, ou estimada pelo tamanho: 40 × 60 cm dá 30). Mais esquema de aterramento, capacidade de interrupção pelo curto informado e `selective`. O check diz quando não cabe (erro), quando o geral fica abaixo do dobro do maior parcial, e quando o curto foi só assumido. Numa cópia desta planta: 23 de 30 módulos, geral 2P 40 A bifásico, seletivo. (`c221a61`) |

## 47. O quadro não tem capacidade, e a proteção para no disjuntor

O quadro de distribuição da planta guarda só o tamanho da caixa:

```
{catalog: "electrical-panel", width: 40, depth: 10, height: 60, elevation: 150}
```

Nenhuma capacidade em módulos DIN — e nada cobra isso. Levados os circuitos de
7 para 10, um ponto em cada novo:

```
electrical(circuits) → 10 circuitos, main_breaker 80 A, e mais nada
```

Nenhuma menção ao quadro. Ninguém somou o que aquilo ocupa: dez disjuntores
monopolares valem dez módulos, cada DR bipolar vale dois — os cinco da planta,
dez —, e o geral mais dois. São 22 módulos numa caixa de 40 × 60, que na
prática comporta de 24 a 32 conforme o modelo. Estamos no limite e o desenho
não sabe.

Isso não é detalhe de acabamento: é o que decide se o quadro previsto na
parede serve ou se vai virar um segundo quadro na obra — com parede aberta,
alimentador novo e o lugar já ocupado por outra coisa. E é uma decisão que muda
conforme o projeto cresce: cada carga dedicada que entra come dois módulos.

**A proteção também para cedo.** O que já sai calculado é bom: disjuntor por
circuito, bitola, DR onde há molhado, disjuntor geral que subiu sozinho de 80
para 100 A quando entrou um chuveiro. O que não existe:

- **DPS**, que a NBR 5410 exige na entrada em boa parte dos casos — não há no
  catálogo nem no cálculo
- **aterramento**: esquema (TN-S, TT), barramento de terra, condutor de
  proteção por circuito — nada disso aparece
- **capacidade de interrupção** do disjuntor em kA, que depende do curto
  presumido no ponto de entrega
- **seletividade** entre o geral e os parciais, para que uma falta no chuveiro
  não apague a casa

**Deveria:** o `electrical-panel` ter capacidade em módulos, e `circuits`
somar o que os dispositivos ocupam, avisando quando não cabe — no mesmo tom do
`no_door`, que hoje diz que um cômodo ficou sem acesso. E a proteção seguir
até onde a norma vai: DPS, aterramento, kA e seletividade.

## Auditoria das normas: o que mudou, a conferir na planta

Cada número que as ferramentas usam foi conferido no texto das normas. Foram
lidos o texto integral da NBR 5410:2004, da NBR 8160:1999, da NBR 5626:2020,
da NBR 15575-1:2013 e da NBR 9050:2020, a ET 0017 v02 da Enel SP, o Decreto
57.776/2017, o Decreto estadual 12.342/78, a NBR 5413:1992 e as diretrizes da
NKBA. A NBR 16264 e a NBR 13103 foram vistas por fontes que reproduzem suas
tabelas; por isso continuam marcadas como "confirmar antes de usar". O que
estava errado foi corrigido. **Os números da planta vão mudar**: nota,
findings, cargas e disjuntores. A tabela diz o que conferir e o que deve
voltar. Tudo está no app reinstalado.

| Área | Como conferir | O que deve voltar |
|---|---|---|
| Carga de iluminação (5410 9.5.2.1.2) | `electrical(action="circuits")` | A carga é do cômodo, pela área, e se divide entre os pontos dele: 100 VA até 6 m², mais 60 VA a cada 4 m² inteiros. Antes eram 100 VA por ponto. Os VA de C1 e C2 mudam. (`a3bc454`) |
| Iluminação e tomadas no mesmo circuito (9.5.3.3) | `electrical()` | Só vira erro acima de 16 A, ou se toda a iluminação ou todas as tomadas estiverem em circuitos mistos. Nos demais casos, a residência admite. |
| Tomadas de cozinha e lavanderia (9.5.3.2) | `electrical()` | Novo erro `elec:kitchen-circuit:Cn` quando essas tomadas dividem circuito com iluminação ou com pontos de outros cômodos. |
| Circuito exclusivo (9.5.3.1) | `electrical()` | Só é exigido para equipamento acima de 10 A. |
| DR (5.1.3.2.2) | `circuits`, coluna DR | Agora também a iluminação de cozinha, lavanderia, área de serviço e garagem abaixo de 2,50 m, e as tomadas externas. O C2 da cozinha passa a ter DR. |
| Seção e agrupamento (tabela 42) | `circuits`, depois de `route` de força | A seção é corrigida pelo número de circuitos que dividem o mesmo trecho de eletroduto no traçado. Também pode ser informado: `grouping` no projeto (`elec:grouping`). |
| Queda de tensão (6.2.7.2) | `route` de força, depois `electrical()` | `elec:drop:Cn` quando passa de 4 % até o ponto mais longe. |
| Disjuntores (NM 60898) | `circuits` | Série 6, 10, 13, 16, 20, 25, 32, 40, 50, 63, 80, 100, 125 A. |
| Chuveiro | `circuits` | Sem potência escrita, conta 7500 W (os mais vendidos): 34 A, 6 mm², 40 A. Informe a potência da placa com `assign va`. |
| Fornecimento Enel SP (ET 0017) | `circuits`, campo `main_breaker` | Monofásico até 12 kW (antes 8), bifásico até 20 kW, trifásico até 75 kW. O disjuntor de entrada sai da tabela fixa da Enel: 50, 63, 80, 100, 125… A. |
| Quadro | `circuits`, campo `panel` | Capacidade de interrupção de 10 kA (Enel, até 63 A), aterramento TN-C-S e DPS com Up ≤ 1,5 kV e N-PE ≥ 10 kA. A seletividade virou regra prática, citando a NM 60898 e não a 5410. |
| Automação | `electrical()` e `circuits` | Os achados citam dado de fabricante, não a 5410. Consumo em espera: 1 W (relé, dimmer, sensor) e 1,2 W (interruptor inteligente). Dimmer até 1,1 A (~140 W em 127 V). Sensor de teto entre 2,2 e 3 m, alcance de 1,45 × a altura. |
| Rede e TV: norma residencial | `electrical()` | Citam a **NBR 16264** (residencial), não a 14565 (comercial). A tabela 1 pede por cômodo: dormitório, sala, escritório, cozinha e lavanderia com 2 RJ45 e 1 TV; banheiro e demais cômodos com 1 e 1; home theater com 3 e 2. O Wi-Fi não conta como RJ45. Achado novo `elec:telecom-power` quando o distribuidor de telecom não tem tomada junto. Espere dicas novas por cômodo. (`148c9ae`) |
| Cabos e Wi-Fi | `route kind=data` e `wifi` | Cat 6 leva 10 GbE só até 37 m, e 5 GbE até 100 m. Cat 6A pode ser U/UTP ou F/UTP. PoE passa a ser 802.3at ou 802.3bt, nunca af. O uplink é propriedade do AP (`wifi uplink=`); no Wi-Fi 7 o padrão é 2,5 GbE. O concreto atenua mais (20/33/38 dB). A faixa de 6 GHz respeita o corte da Anatel (Ato 10.400/2026). O limite de 90 m conta as sobras. |
| Hidráulica (8160, 5626, Código Sanitário SP) | `plumbing()` e `plumbing(action="route")` | Máquina de lavar precisa de ponto de esgoto próprio de 50 mm (`plumb:sewer`), nunca a caixa sifonada. Ralo no piso de todo banheiro, cozinha, copa e lavanderia (`plumb:drain`, art. 15 II). Ventilação (`plumb:vent`, e `plumb:vent-far` pela tabela 1; peça nova `vent-pipe`). Caixa sifonada dimensionada pelas UHC: saída de 50 mm até 6, 75 acima, aviso acima de 15. Vaso, caixa sifonada e caixa de gordura a até 10 m da inspeção (`plumb:inspection-far`). Profundidade pelo caimento de cada ramal. Série reforçada nos ramais de 50 mm. Y de 50 com bucha para ramal de 40. Nota sobre válvula de descarga (50 mm, ramal próprio). Espere pendências novas: ventilação e ralo seco na cozinha e na lavanderia. (`d12538e`) |
| Pé-direito (15575-1 16.1.1) | `ergonomics` | Cozinha e lavanderia pedem 2,50 m; só banheiro e circulação admitem 2,30 m. (`16e7032`) |
| Folgas (15575-1, Anexo F) | `ergonomics` | 40 cm diante de vaso e lavatório (antes 60); 50 cm diante de tanque e máquina (antes 60); 50 cm diante de sofá e poltrona (antes 35). Pés da cama e mesa passam a citar a norma. Guarda-roupa: 80 cm por adulto (1,60 m no casal). |
| São Paulo (Decreto 57.776, 12.342) | `ergonomics(city="sao-paulo")` | Círculo de 1,50 m na cozinha (antes 1,20). Lados mínimos do decreto e do anexo F: dormitório 2,00, sala 2,40, cozinha 1,50, banheiro 1,10, lavanderia e circulação 0,90. Área mínima de 5 m² nos cômodos de permanência. Iluminação natural de 1/8 do piso, 1/5 no escritório e 1/10 na lavanderia. |
| NBR 9050 | `ergonomics(wheelchair=true)` | Interruptor entre 60 e 100 cm, tomada entre 40 e 100 (antes 40–120 para os dois). |
| Gás | `ergonomics` | Com janela, o gás ainda pede abertura permanente (Alerta): janela que fecha não é ventilação. Sem janela, continua erro, citando o Decreto 57.776 3.M. |
| Cozinha | `ergonomics` | Triângulo pela NKBA: total até 792 cm, cada lado entre 122 e 274 cm. MCMV vira comparação (Dica). Duas tomadas sobre a bancada podem ser uma tomada dupla (`elec:sockets=2`). Lavabo não precisa de box. |
| Iluminação | `lighting()` | Os valores residenciais vêm da NBR 5413: cozinha e banheiro 150 lx (bancada e espelho 300); antes eram 300 e 200. A 8995-1 fica como referência de local de trabalho. |

**Ainda não coberto** (fica para a próxima rodada):
- **Elétrica:**
  - caixa de passagem a cada 15 m menos 3 m por curva, e no máximo 3 curvas entre caixas;
  - DR agrupado para mais de um circuito, que hoje conta um DR por circuito;
  - fator de demanda.
- **Esgoto:**
  - declividade máxima de 5 %;
  - a proibição da 4.2.3.5 (ramal ligado pela inspeção da curva do vaso);
  - dimensões da caixa de inspeção.
- **Wi-Fi:** atenuação entre andares e vidro low-e.
- **Ergonomia:**
  - 60 cm entre camas de solteiro;
  - acessibilidade: pia a até 85 cm e cama a 46 cm;
  - gás em banheiro, que só admite aparelho tipo C;
  - limite de 8,14 kW em ambiente integrado.

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
