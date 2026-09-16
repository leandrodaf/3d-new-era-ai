# Atritos de uso

O que dói quando um agente conduz uma planta real pelo MCP, do lado de fora.

Um agente recebe uma planta de verdade e a tarefa de corrigi-la. Cada vez que
uma ferramenta responde menos do que a pergunta pedia, obriga a um contorno, ou
leva a uma conclusão errada antes de levar à certa, o caso entra aqui com a
chamada que se fez, a resposta literal, o que era verdade, o que custou e o que
teria encurtado o caminho. Tudo anotado à mão.

**Este arquivo guarda só o que ainda não foi resolvido.** Cada caso que cai sai
daqui; o que doía em cada um está no histórico do git. Cinquenta e nove já
saíram, todos conferidos em uso na mesma planta e não no changelog.

O banco de provas é o mesmo apartamento de 65 m² desde o começo, com marcenaria
desenhada módulo a módulo. Hoje ele está em **nota 100**, zero achados com
peso, `loose: 0`, `pending: 0` nas três disciplinas e 216 peças, nenhuma sem
apoio em piso, teto, parede ou outra peça.

## Em aberto

Um caso, e ele é o 57 outra vez, em outro arquivo: uma peça classificada pela
palavra que aparece no nome dela, e não pelo que ela é.

## 59. Uma bancada batizada com o nome do eletrodoméstico ao lado não pode receber tomada embutida

O `outlet-tower` e o `desk-outlet-box` são peças novas e boas: torre retrátil de
embutir e caixa de tomadas de mesa, com regras próprias de fabricante — 2,5 cm
entre o furo e a borda do tampo, 30 cm da cuba e da cocção, e uma tomada dentro
do gabinete quando a torre é de plugue. As três dispararam certo no primeiro
posicionamento errado que fiz.

O que não funciona é pôr a torre na cozinha desta planta:

```
place(cat="outlet-tower-auto", at=[510,300], elev=91)
→ ok rev=15
ergonomics → ["erro", "elec:mount:f1901",
  "… está solta: vai embutida no tampo de uma bancada, ilha ou móvel
   (ou numa mesa, a caixa de mesa)."]
```

`[510, 300]` está em cima da bancada `f847`, que vai de x 440 a 632 e de y 264
a 329, com o topo a 91 cm — exatamente a elevação pedida. Ela é o tampo.

Duas sondas mostram por quê:

```
place(cat="outlet-tower-auto", at=[130,515], elev=75)   # "Escritório 1 — bancada de madeira"
→ ok rev=22
place(cat="outlet-tower-auto", at=[800,300], elev=101)  # "Tampo contínuo sobre lava e seca até fachada"
→ vai embutida numa bancada, ilha ou móvel fixo: não há um sob [800.0, 300.0]
```

A bancada do escritório aceita; a da lavanderia não. A diferença está no nome.
Em `crates/newera-core/src/mounting.rs:658`, `movable()` decide se a peça é
móvel solto procurando palavras no nome, e a lista tem `"lava"`, `"geladeira"`,
`"forno"`, `"mesa"`. O `host_of` descarta tudo que `movable()` aponta:

| tampo | por que é descartado |
|---|---|
| `Bancada contínua junto à geladeira` | "geladeira" |
| `Tampo contínuo sobre lava e seca até fachada` | "lava" |
| `Península — pedra sobre lava-louças 60,5 cm` | "lava" |

Nesta cozinha os três tampos são nomeados pelo eletrodoméstico que servem, que
é como um projeto de marcenaria nomeia as peças, e nenhum deles pode receber
uma torre. O contorno seria rebatizar a bancada com uma palavra que a regra não
conheça — trocar um nome que descreve a peça por um que engana a ferramenta.
Foi o que o caso 43 já custou uma vez na hidráulica; não fiz.

**Custou:** quatro posicionamentos, duas sondas e uma leitura do `mounting.rs`.
A torre saiu da planta; a caixa de mesa do escritório ficou, porque ali o tampo
se chama "bancada de madeira".

**Deveria:** `mounting.rs` usar a mesma correção que `layers.rs` recebeu no caso
57 — palavra de marcenaria ("bancada", "tampo", "península", "gabinete") ganha
da palavra de eletrodoméstico quando as duas aparecem no mesmo nome. E, já que
a peça pode ser declarada em vez de adivinhada, deixar o `update` marcar um
tampo como fixo, para quando o nome não ajudar.

**Não piorar:** a geladeira, a lava-louças e o forno de verdade têm que
continuar recusados como anfitriões — o achado *"está dentro de LG WD18GNTS6BA
— Lava e Seca 18 kg (f825)"*, que apareceu nesta mesma rodada e pegou um erro
real meu, depende disso.
