# Atritos de uso

O que dói quando um agente conduz uma planta real pelo MCP, do lado de fora.

Este documento é escrito de fora para dentro: um agente recebe uma planta de
verdade e a tarefa de corrigi-la. Cada vez que uma ferramenta responde menos do
que a pergunta pedia, obriga a um contorno, ou leva a uma conclusão errada
antes de levar à certa, o caso é anotado aqui com o que aconteceu de fato.

Não é uma lista de desejos. Cada entrada traz o que se tentou, o que voltou, e
o que teria encurtado o caminho — com o caso concreto que o produziu, para que
se possa reproduzir.

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

  **Encurtaria:** a mudança que teria evitado o contorno.
-->

---

## O que a rodada anterior deixou para verificar

A primeira rodada levantou 14 atritos sobre um apartamento de 65 m² com
marcenaria desenhada módulo a módulo. O texto completo de cada um, com os
números e as respostas literais, está no histórico:

    git show caf4ec1:docs/ATRITOS-DE-USO.md

Um deles — as análises existirem só atrás do MCP, e o REST mandar `width`/`depth`
sem o ângulo aplicado — foi resolvido em `94cdd11`. Os outros treze são o ponto
de partida da revisão nova: conferir, na versão que sobe agora, o que caiu, o
que continua e o que apareceu junto.

| # | O que doía | Onde |
|---|---|---|
| 1 | Dry run não diz o *tipo* do problema que vai criar (só `["f817+f830"]`) | `move`, `update` |
| 2 | Folga `0` quer dizer "encostado" e "enfiado dentro" | `clearances` |
| 3 | Modelo importado colide pela caixa, e não há como declarar convivência | `check_layout` |
| 4 | Discorda de `measure` sobre para que lado a peça abre; `turned` não tem conserto | `ergonomics` |
| 5 | O `fix` sugerido tira o embutido do móvel e melhora a nota | `ergonomics` |
| 6 | A folga é o pior ponto, sem dizer em que extensão vale | `ergonomics` |
| 7 | Os números que a planta guarda no nome das peças não são conferidos | `annotations` |
| 9 | Não há `accept` com motivo, como em `ergonomics` | `check_layout` |
| 10 | Cota sem âncora nunca fica stale; ancorar depois congela o erro | `annotations` |
| 11 | `accept` sobrevive ao achado que o justificava, e volta silenciado | `ergonomics` |
| 12a | `scope=project` ignora o `q` e despeja o projeto inteiro | `catalog` |
| 12b | O `score` do dry run usa a ocupação padrão, não a da revisão | dry runs |
| 13 | Âncora morre com a peça e `anchor=true` não religa; cota fica stale para sempre | `annotations` |
| 14 | Apagada a peça, o rótulo dela fica apontando para o vazio | `delete` |
