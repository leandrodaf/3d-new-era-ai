# Atritos de uso

O que dói quando um agente conduz uma planta real pelo MCP, do lado de fora.

Este documento é escrito de fora para dentro: um agente recebe uma planta de
verdade e a tarefa de corrigi-la. Cada vez que uma ferramenta responde menos do
que a pergunta pedia, obriga a um contorno, ou leva a uma conclusão errada
antes de levar à certa, o caso é anotado aqui com o que aconteceu de fato.

Não é uma lista de desejos. Cada entrada traz o que se tentou, o que voltou, e
o que teria encurtado o caminho — com o caso concreto que o produziu, para que
se possa reproduzir.

Desde que existe o `feedback`, o caso vai primeiro por lá, com os mesmos
campos. Este arquivo é o registro que fica no repositório.

## Rodada em aberto

Projeto elétrico montado do zero na mesma planta: 22 tomadas, 4 pontos de rede,
3 de TV, dois quadros e 7 circuitos pela NBR 5410. Os dois casos abaixo já
foram enviados pelo `feedback`.

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
