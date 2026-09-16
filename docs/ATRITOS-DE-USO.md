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

## Ainda de pé da rodada anterior

Enviados pelo `feedback`; o texto completo está em
`git show 87083ec:docs/ATRITOS-DE-USO.md`.

| # | O que dói | Onde | Reproduz com |
|---|---|---|---|
| 32 | Cômodo sem porta não entra em relatório nenhum — um dormitório lacrado ainda tira nota 99 | `check_layout`, `ergonomics` | `delete` da porta do quarto, depois `check_layout()` |
| 34 | `hinge_right` responde ok e não faz nada, e é o que o erro do `blocks_door` recomenda | `update` | `update(hinge_right=true)` numa porta, e reler a peça |

Sem como reproduzir agora: o `fix` que tirava a lava-louças do nicho e melhorava
a nota (a planta não oferece `fix` hoje), e as anotações que sumiram uma vez
sem que comando nenhum mencionasse.

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
