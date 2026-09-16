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

Nada anotado ainda.

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

---

## Corrigidos, a conferir em uso

Enviados pelo `feedback`; o texto completo está em
`git show 87083ec:docs/ATRITOS-DE-USO.md`. Corrigidos no código, com teste que
reproduz o caso; saem daqui quando conferidos na planta.

| # | O que doía | Corrigido em | Conferir com |
|---|---|---|---|
| 32 | Cômodo sem porta não entrava em relatório nenhum — um dormitório lacrado tirava nota 99 | `3f63d0e`: `ergonomics` dá erro "Sem acesso" e `check_layout` traz `no_door` (cômodo e vãos); sala, cozinha e varanda abertas não entram | `delete` da porta do quarto, depois `check_layout()` e `ergonomics()` |
| 34 | `hinge_right` respondia ok e parecia não fazer nada | `e82752e`: a porta já estava com `hinge_right: true`; agora o `update` lista em `unchanged` o que já tinha o valor pedido, com o valor atual, e o `blocks_door` diz o valor a usar | `update(hinge_right=<o valor que a porta já tem>)` |

Os que não se reproduziam: o `fix` que tirava a lava-louças do nicho deixou de
ser oferecido quando a peça perde o encosto do nicho (`187cf25`); e toda
escrita, dry run, undo e lote pelo REST passaram a nomear mudanças nas
anotações e nas properties (`afc3d42`), para que um sumiço não passe calado.

## O que já caiu

Trinta casos, todos verificados em uso na mesma planta de 65 m², não no
changelog. A lista com o que doía em cada um e onde foi resolvido está em
`git show 87083ec:docs/ATRITOS-DE-USO.md`.

Os que mais mudaram o trabalho: a folga negativa no lugar do `0` ambíguo; a
extensão junto da folga (`"54 cm em 34,5 dos 185 cm"`), que separa um móvel
inutilizável de um canto apertado; o `ergonomics` medindo pelo lado em que a
peça abre; o `cut_list` alcançando o que foi desenhado à mão e declarando o que
pulou; o grupo que mantém a espessura das chapas e faz crescer o vão; e o
número da peça preso à peça, que era o mais sério para quem recebe a prancha na
obra.
