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

Versão com os fixes, mesma planta, continuando a marcenaria da península.

### O que caiu

Conferido em uso, não por leitura do changelog:

- **1 — o dry run diz o tipo.** `"issues_new": [{"ids": "f1042", "kind": "turned"}]`.
- **2 — folga negativa.** `"+y": [-58, "f830", …]` onde antes vinha `0`.
- **4 — `ergonomics` usa a face construída.** O gabinete da pia, que relatava
  "1 cm livres à frente" medindo contra o próprio tampo, agora relata os
  75,2 cm do corredor, que é para onde ele abre.
- **6 — a folga vem com a extensão.** `"54 cm livres à frente em 34,5 dos
  185 cm"`. Era o que faltava para ler um aperto: 34,5 cm de 185 é a quina da
  mesa de cabeceira, não um armário que não abre. Três justificativas escritas
  à mão na rodada anterior, com sonda, agora vêm prontas na mensagem.
- **8 — as análises no REST**, com `bounds` já girado.
- **9 — `accept` em `check_layout`**, com `orphaned` junto.
- **11 — aceites órfãos aparecem.** `"orphaned": [["nbr15575g:f848:…", "…"]]`.
  O aceite que sobrevivia ao próprio achado agora se anuncia.
- **12a — `catalog(scope=project)` respeita o `q`.** Quatro linhas no lugar de
  duzentas.

O 7 e o 10 foram além do pedido: `annotations(stale=true)` agora **confere os
nomes das peças** e **diz o que conferiu** —

```
"checked": {"dims": 19, "dims_unanchored": 10, "labels": 0, "names": 59}
```

`dims_unanchored: 10` é exatamente a resposta que faltava: dez cotas que ninguém
está vigiando, em vez de um `[]` que parecia um atestado de saúde.

## 15. O parser de medidas não entende a vírgula decimal

A checagem de nomes, recém-chegada, devolveu 50 achados nesta planta. **28
deles** são isto:

```
f954  escrito=5   medido=56,8   "48 — Gabinete do tanque integrado — 80,5 cm, duas portas"
f849  escrito=75  medido=70,8   "Aéreo lavanderia — módulo 70,75 cm; limpeza"
f831  escrito=9   medido=35     "Aéreo geladeira — 79,9 cm; ventilação inferior preservada"
f1043 escrito=9   medido=30,7   "Micro-ondas … 53,9 × 43 × 30,7"
```

`80,5` foi lido como `5`; `70,75` como `75`; `79,9` como `9`. O parser quebra no
separador decimal brasileiro e fica com o que vem depois da vírgula.

Num software que mede em centímetros, cita ABNT e tem a interface em português,
é o formato que a planta inteira usa. O efeito é pior do que não ter a
checagem: 28 alarmes falsos afogam os reais — havia um só, `f1035`, "tampo
aberto 110 × 30" com 119 cm de largura.

**Encurtaria:** aceitar `,` como separador decimal ao ler o número.

## 16. Redimensionar um grupo inverte a face que ele declara

Ampliar a mesa basculante de 110 para 119 cm — mudança só em `x` — fez a peça
trocar de lado:

```
"from": {… "faces": "+y", "wdh": [110, 36, 29]}
"to":   {… "faces": "-y", "wdh": [119, 36, 29]}
```

A geometria em `y` é idêntica antes e depois (o tampo segue projetando de 485 a
515, para a sala). Só a face declarada girou, e com ela veio um `turned` novo
no relatório.

Pior, não há como desfazer só a declaração: `arrange flip` acerta o `faces`
mas **espelha o conteúdo** — o tampo saltou de 485-515 para 479-509, entrando
6 cm dentro do móvel —, e `angle` gira o grupo inteiro. Foi preciso dar undo e
deixar a peça com o metadado errado.

**Encurtaria:** o resize não recalcular a face; ou um jeito de corrigir a
declaração sem mover nada.

## 17. Grupo só sabe encolher junto

A face da península tinha 180 cm: montante de 5,8, a mesa de 110, e 63,8 cm de
trecho cheio. O vão virou 131 cm, e o que se queria era manter os montantes e
**crescer** a mesa até a torre quente.

`update(w=131)` no grupo escala tudo pelo mesmo fator: montante 5,8 → 4,2, mesa
110 → 80,3. O oposto do pedido. Um vão de marcenaria não encolhe assim — os
montantes têm a espessura que têm, e quem cresce é o vão entre eles.

A saída foi `arrange ungroup`, editar as sete peças à mão com as contas
refeitas, e deixar desagrupado.

**Encurtaria:** uma forma de dizer o que estica e o que fica.

## 18. Parte de grupo não pode ser renomeada, e é a parte que mente

Depois do resize, `f1035` continua se chamando "tampo aberto 110 × 30" com
119 cm. Corrigir é recusado:

```
f1035 … is a part of f1042; edit f1042 instead
```

Mas editar `f1042` renomeia o grupo, não a parte. O nome errado é o da peça de
dentro — a mesma que o `stale` novo acusa, com razão, e a única que não se pode
consertar. O achado real fica para sempre na lista.

**Encurtaria:** permitir `name` em parte de grupo. É metadado, não geometria.

## 19. A sonda do `measure` não vê um armário de 280 cm

Sondando a bancada da lavanderia:

```
measure(axis="x", at=450, range=[612, 650], z=[0, 280])
→ spans: [[612, 613, "f1080", "Torre — rodapé recuado"], [613, 650, null, ""]]
```

De 613 a 650 estaria vazio. Não está: o vassoureiro `f988` ocupa 615-645, com a
lateral `f957` em 615-617 subindo de z 0 a 280. A sonda atravessa um armário
inteiro como se fosse ar — e reporta, na mesma resposta, uma peça vizinha
(`f1080`) que também é parte de um grupo.

O mesmo `measure`, perguntado de outro jeito, acerta:

```
measure(from="f988") → "+x": [50, "f955", …]
```

A consequência apareceu sozinha: `annotations(stale=true)` marcou a cota `d106`
("passagem de 50 cm") como errada, dizendo que a ponta dela não toca nada e
que `f1080` está 32 cm adiante — porque a ancoragem enxerga o mesmo vazio. A
cota está certa; quem não vê é a sonda.

**Encurtaria:** a sonda ler as partes de todo grupo, como `from` já faz. É a
ferramenta que responde "o que tem aqui" — e um armário que some dela some
também da ancoragem que depende dela.


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

## 20. A lista de corte só enxerga o que o `joinery` fez

Esta planta é marcenaria do começo ao fim: torre quente, gabinete do tanque,
gavetões, aéreos, vassoureiro — cada um desenhado módulo a módulo, com as
larguras, as chapas, os puxadores e os rodapés recuados no nome de cada peça.

```
cut_list()          → "no joinery builds here (make one with the joinery tool)"
cut_list(ids=[…])   → idem
```

Nenhum móvel do projeto entra na lista de corte. Os que vieram de modelos
importados não entram; o vassoureiro reconstruído aqui, como grupo de sólidos,
também não. A saída mais valiosa do software — a lista que vai para a serra,
com chapas, fitas de borda e ferragens — está fechada para quem desenhou de
outro jeito, e é justamente quem mais precisaria dela.

O `joinery` resolveria, mas refazer um vassoureiro paramétrico custa o
acabamento: as molduras 3D, as frentes rebaixadas e os puxadores de latão que
combinam com o resto da cozinha não sobrevivem à troca.

**Encurtaria:** o `cut_list` aceitar um grupo qualquer, tratando cada sólido
como uma peça — as medidas estão todas lá.

> **Confirmado pelo outro lado.** O buffet da varanda foi refeito com
> `cabinet_run` + `joinery`, e o `cut_list` respondeu na hora: 28 linhas de
> peças com material, quantidade, medidas e fita de borda (`"2+2"`), mais a
> ferragem separada por módulo — corrediças, dobradiças de caneco, suportes de
> prateleira, puxadores —, tudo apoiado em NBR 15316 e NBR 14810. É uma
> ferramenta excelente atrás de uma porta fechada: quem desenhou a planta com
> modelos importados não a alcança, e o custo de alcançá-la é refazer o móvel.

## 22. O erro de tipo não diz qual campo

`cabinet_run` com dez parâmetros em `p` e um deles errado:

```
{"row": "base", "h": 87, "d": 65, "top": true, "drawers": true, …}
→ "invalid parameters: invalid type: boolean `true`, expected u32"
```

Qual deles? `top` também é booleano e está certo; `drawers` é que esperava um
número. A mensagem não nomeia o campo, e num `p` de dez chaves sobra tentativa
e erro.

O contraste está na chamada seguinte, que errou o tipo de `room`:

```
→ "invalid id `Varanda` (expected e.g. `r12`)"
```

Essa diz o que veio, o que se esperava e dá um exemplo. É o padrão que a outra
deveria seguir.

**Encurtaria:** nomear o campo no erro de tipo dentro de `p`.

## 21. `accept` no `check_layout` só vale para `overlap`

O `accept` chegou (era o atrito 9), e funciona: aceitar um par de sobreposição
tira o peso e mantém a linha no relatório, com `orphaned` avisando quando o
motivo deixa de valer.

Mas a chave só existe para `overlap` —

```
"key": "overlap:f826+f847"
```

`in_wall` e `outside_rooms` vêm sem `key`. São, nesta planta, as cinco linhas
que sobram para sempre: as duas persianas de rolo que estão dentro da parede
porque é ali que elas ficam, o shaft e os dois vidros do escritório que não
estão dentro de nenhum polígono de cômodo porque não deveriam estar. Aceitar
uma delas devolve `orphaned` — a chave inventada não corresponde a nada.

**Encurtaria:** `key` também em `in_wall` e `outside_rooms`.

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
