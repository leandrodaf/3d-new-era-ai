# Atritos de uso

O que dói quando um agente conduz uma planta real pelo MCP, do lado de fora.

Um agente recebe uma planta de verdade e a tarefa de corrigi-la. Cada vez que
uma ferramenta responde menos do que a pergunta pedia, obriga a um contorno, ou
leva a uma conclusão errada antes de levar à certa, o caso entra aqui com a
chamada que se fez, a resposta literal, o que era verdade, o que custou e o que
teria encurtado o caminho. Tudo anotado à mão.

**Este arquivo guarda só o que ainda não foi resolvido.** Cada caso que cai sai
daqui; o que doía em cada um está no histórico do git. Sessenta já saíram,
todos conferidos em uso na mesma planta e não no changelog.

O banco de provas é o mesmo apartamento de 65 m² desde o começo, com marcenaria
desenhada módulo a módulo. Hoje ele está em **nota 100**, zero achados com
peso, `loose: 0`, `pending: 0` nas três disciplinas e 218 peças, nenhuma sem
apoio em piso, teto, parede ou outra peça.

## Em aberto

Um caso, e é pequeno: um campo novo que grava certo e responde que não gravou.

## 60. `update(fixed=…)` escreve a propriedade e diz que não mudou nada

O `fixed` veio junto com a correção do caso 59: serve para declarar que uma
peça é fixa quando o nome dela não deixa a ferramenta adivinhar. Ele funciona.
O que ele responde é que não funcionou.

Numa cadeira que não tinha a propriedade:

```
update(items=[{"id": "f835", "fixed": true}])
→ ok rev=11 {"unchanged": [{"id": "f835", "now": {…}}],
   "unchanged_note": "these already had the values asked for; nothing was changed on them"}
```

E no `/api/home`, logo depois:

```
f835 props: {"piece:fixed": "true", "ref:tag": "146", "sh3d:id": "…"}
```

A propriedade foi gravada. A resposta diz o contrário, e diz na forma mais
enganosa possível — *"já tinham os valores pedidos"* —, que é a frase que
convence quem chamou a parar de tentar. Foi o que aconteceu comigo: li o
"unchanged", concluí que o campo não existia na versão instalada e só descobri
que tinha gravado ao despejar o JSON da planta.

O `layer` já acerta isso: `update(items=[{"id": "f831", "layer": "joinery"}])`
responde `{"changed": 41}`.

**Segunda metade:** não há como retirar a declaração. `layer: ""` volta à
classificação automática; `fixed: null` não faz nada (e responde "unchanged"
também), e `fixed: false` grava `"false"`, que é uma afirmação diferente de
"não declarei". Na planta ficou um `piece:fixed: "false"` na mesa de jantar,
resíduo de um teste, que não tenho como apagar.

**Reproduzir:** `update(fixed=true)` em qualquer peça sem `piece:fixed`, e
depois ler as propriedades dela.

**Custou:** um despejo do `/api/home` para descobrir que a chamada tinha
funcionado, e um `undo` para limpar a cadeira.

**Deveria:** responder `changed` quando grava, como o `layer` faz; e aceitar
`fixed: ""` (ou `null`) como "volte a decidir sozinho", apagando a propriedade.
