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
zero achados com peso, `loose: 0` e `pending: 0` nas três disciplinas —
elétrica (22 tomadas, 13 pontos de rede, 7 circuitos, quadro de 24 módulos com
3 de reserva, 3 peças de automação, um access point com cobertura calculada e
158 m de cabo traçados pelo `route`), hidráulica (30 pontos, 3 colunas e 20 m
de ramal de ventilação, 2 grelhas de ventilação permanente, 78 m de tubo) e
arquitetura.

## Rodada em aberto

Dois casos. Dos dez abertos na rodada passada, oito caíram na versão nova,
conferidos um a um na planta com o comando que os produziu. O que sobrou é um
achado que anda em círculo e um traçado que a própria ferramenta não reconhece.

## 50. Dois achados que se anulam, e o `fix` que anda em círculo

*Conferido de novo nesta versão: continua igual.*

Restava uma dica de peso 1 no sofá:

```
"36 cm livres à frente (sentar, levantar e circular diante do assento:
 mínimo 50 cm); afaste 14 cm."   f933, peso 1
```

Afastados os 14 cm pedidos, volta:

```
"71 cm livres à frente (circulação diante de bancada e equipamentos:
 mínimo 85 cm); afaste Sofá Milano … f933 14 cm."   f1068, peso 5
 "fix": {"tool": "move", "ids": ["f933"], "dx": 0.0, "dy": 14.0}
```

O `fix` é exatamente o movimento inverso do que acabou de ser feito, e a nota
vai de 99 a 94. Os dois achados disputam o mesmo vão entre a península da
cozinha e o rack da sala: 50 cm na frente do sofá mais 85 cm na frente da
bancada dão 135 cm, e o vão não tem. Não existe posição que satisfaça os dois,
e nenhuma das duas mensagens diz isso.

**Reproduzir:** `move(ids=["f933"], dx=0, dy=-14)` e rodar `ergonomics`.

**Deveria:** quando o `fix` de um achado criaria outro de peso igual ou maior,
dizer na própria mensagem — *"não cabem os dois: as duas folgas somam 135 cm e
o vão não tem; a posição atual é a melhor das duas"*. Um achado que só pode ser
aceito deveria nascer sabendo disso.

## 58. O `route` de ventilação desenha um ramal que o próprio `check` recusa

Caso novo, e ele nasceu de uma correção: o `plumb:vent-far` agora ensina o
caminho — *"trace o ramal com route kind=vent e ele passa a contar"*. Traçado:

```
plumbing(action="route", kind="vent",
         ids=["f1591","f1598","f1589","f1596","f1594","f1601"], via="wall")
→ {"via":"wall", "length_m":{"total":20.4}, "branches":5,
   "run":"vent:f1589+f1591+f1594+f1596+f1598+f1601",
   "materials":[["Tubo PVC esgoto série normal 50 mm (ramal de ventilação)",21.4,"m"], …]}
```

O ramal foi desenhado, e `pipes_m` passou a contar `"vent": 20.4`. O check, no
passo seguinte, sem nenhuma edição no meio:

```
plumbing(check)
→ ["alerta","Ventilação","Pontos sem tubulação chegando:
    f1601, f1596, f1598, f1611, f1609, f1612, f1629, f1594, f1606, f1604."]
```

Os seis que ele acabou de traçar estão na lista. Medindo as polilinhas que ele
mesmo escreveu contra os pontos que ele mesmo escolheu:

| ponto | distância à polilinha |
|---|---|
| f1591 · lavatório social | 25,5 cm |
| f1589 · vaso social | 27,5 cm |
| f1596 · vaso suíte | 33,0 cm |
| f1598 · lavatório suíte | 33,0 cm |
| f1594 · ralo box social | 49,0 cm |
| f1601 · ralo box suíte | 50,0 cm |

O traçado corre no eixo da parede (`[[30,496],[126,496],[200,496]]`), que é
onde um ramal embutido corre de verdade, e a verificação de alcance não aceita
essa distância. Nas outras espécies isso não acontece: depois de
`route kind=data`, `kind=power` e `kind=cold`, os `unreached` correspondentes
esvaziam.

**Custou:** duas chamadas de `route`, um script para medir ponto a polilinha, e
um aceite escrito à mão para um achado que é da ferramenta, não do projeto.

**Efeito colateral encontrado no caminho:** duas chamadas de `route vent` com
subconjuntos diferentes de `ids` criam dois runs que se empilham
(`vent:a+b+c` e `vent:b+c`), somando 27,6 m de tubo onde há 20,4, e nenhum
`replaced_drawn` avisa — porque o nome do run vem dos ids. Quando o segundo
conjunto está contido no primeiro, deveria substituir.

**Deveria:** a verificação de alcance da ventilação medir contra o run, com a
mesma folga que as outras espécies usam — ou o `route` recusar traçar o que o
`check` não vai aceitar, em vez de entregar um traçado e cobrá-lo em seguida.

---

## Conferido nesta rodada

Oito dos dez casos abertos caíram. Cada um testado na planta, com o comando que
o produziu — não no changelog.

| # | O que doía | O comando desta rodada | O que voltou |
|---|---|---|---|
| 48 | A aceitação virava órfã quando a chave do achado ganhava a fonte como prefixo | Subida de versão inteira, depois `ergonomics(prune=true)` e `plumbing(check)` | Nenhuma aceitação se perdeu nesta subida: os 20 motivos escritos continuaram colados aos achados. O mecanismo que o caso pedia — casar a órfã com o achado sucessor — não existe: `orphaned` ainda devolve só `[chave, motivo]`, testado com uma aceitação inventada (`plumb:vent-far:f9999`). Sem sintoma, e sem rede de proteção para a próxima vez |
| 49 | `update` aceitava um campo inexistente e respondia "nada mudou" | `update(items=[{"id":"f1576","props":{…}}])` | `unknown field 'props', expected one of id, a, b, t, h, arc, name, pts, …` — recusa na cara, com a lista do que existe |
| 51 | O `route` redesenhava e o traçado à mão ficava por cima | `cable(kind="tv", pts=[[170,750],[340,750],[340,700]])` e depois `route(kind="tv")` | `"replaced_drawn": ["pl1877"]` — apagou o traçado antigo e disse qual |
| 52 | `move` só andava por delta | `move(ids=["f933"], to=[515,630])` | `ok` |
| 53 | Achado que desenho nenhum fechava | `catalog(q="grelha veneziana ventilação")` e `plumbing(route, kind="vent")` | Existem os dois: `vent-grille` "Grelha de ventilação permanente (gás)" 20 × 4 × 15, e `route kind=vent`, que recusa o piso com a razão certa (*"enterrado ele enche de água e não ventila"*). A grelha zera o alerta de gás quando está no cômodo do aparelho — aqui a cozinha não tem parede externa e a envoltória é o fechamento da varanda, então o achado segue aceito, agora apontando para uma peça desenhada |
| 54 | `check_layout` afogava os casos reais em 49 aninhamentos esperados | `check_layout()` | `overlap_kinds: {nesting: 20, served: 31}` — o ponto dentro da peça que ele serve virou relação própria — e uma relação nova, `loose`, com a razão escrita: *"está no vão da porta Porta do banho social (f1512): não há parede ali para a caixa"* |
| 55 | `route hot` pedia a origem e não dizia que era um id | `plumbing(route, kind="hot")` sem `from` | *"name the heater (aquecedor), or give from as the id of the piece it starts from (a heater, a shaft, a column), e.g. from=\"f801\""* |
| 56 | Nada avisava que um ponto ficou solto no ar | `place(cat="outlet-low", at=[400,40])` e `place(cat="network-outlet", at=[135,620])` | A primeira foi assentada sozinha: pedida em `y=40`, ficou em `y=12`, com as costas na face da parede. A segunda foi recusada: *"fica embutido em parede, e a parede mais próxima (w20) está a 119 cm: dê at junto a uma parede ou wall=<id>"* |
| 57 | A camada automática seguia a primeira palavra do nome | `update(items=[{"id":"f831","layer":""}])` e o mesmo nas 14 peças do armário da coifa | Tirados os overrides manuais, "Aéreo geladeira" continua `joinery`, e as 14 peças de madeira do armário da coifa também — a peça de um grupo só sai da camada do grupo quando a camada dela foi escrita à mão |

### O que as regras novas encontraram na planta

Duas verificações que não existiam antes acharam **nove erros reais**, seis
deles introduzidos por mim na rodada passada, quando assentei os pontos à mão
sem saber de vão de porta nem de vidro:

- `loose` — a tomada e o RJ45 da cozinha **dentro do vão da porta do banho
  social**; o registro geral **sobre o vidro do fechamento da varanda**; o ponto
  de água da lava-e-seca solto no meio da lavanderia;
- `elec:hidden` — cinco pontos **atrás da folha aberta de uma porta**
  (*"ponha-o do lado da maçaneta"*): a tomada do dormitório atrás da porta da
  suíte, os dois pontos da cozinha atrás da porta do banheiro, a tomada e o
  RJ45 do escritório atrás da porta de entrada.

Todos corrigidos. A tomada da cozinha foi para junto da geladeira, o RJ45 para
o extremo da bancada, o registro geral desceu para 90 cm (abaixo do peitoril do
vidro) e o ponto da lava-e-seca subiu para a parede atrás da máquina.

Estado final: **nota 100**, zero achados com peso, `loose: 0`, `pending: 0` nas
três disciplinas, 213 peças e **nenhuma sem apoio** em piso, teto, parede ou
outra peça — medido peça a peça, não por amostragem.

## O que já caiu

Cinquenta e seis casos, todos verificados em uso na mesma planta, não no
changelog. A lista com o que doía em cada um e onde foi resolvido está no
histórico do git.

Os que mais mudaram o trabalho: a folga negativa no lugar do `0` ambíguo; a
extensão junto da folga (`"54 cm em 34,5 dos 185 cm"`), que separa um móvel
inutilizável de um canto apertado; o `ergonomics` medindo pelo lado em que a
peça abre; o `cut_list` alcançando o que foi desenhado à mão e declarando o que
pulou; o grupo que mantém a espessura das chapas e faz crescer o vão; o número
da peça preso à peça; o `no_door` para cômodo sem acesso; o quantitativo
agrupado por tipo; o ponto de disciplina que deixou de ser lido como móvel; e
as três disciplinas com o mesmo par `check` + `route`, que é o que transformou
a hidráulica de adivinhação em trabalho conferido; e, nesta rodada, o `place`
que assenta o ponto na parede sozinho e as duas relações que dizem quando uma
peça não está apoiada em nada ou ficou atrás da folha de uma porta — as três
coisas que faziam a planta parecer certa no número e errada no desenho.
