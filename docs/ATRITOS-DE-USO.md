# Atritos de uso

O que ainda dói quando um agente conduz uma planta real pelo MCP, do lado de fora.

Cada caso traz **o que aconteceu**, com a resposta literal que serve de prova,
**como reproduzir** em um comando, e **o que deveria acontecer**. O banco de
provas é a mesma planta desde o começo — um apartamento de 65 m² com marcenaria
desenhada módulo a módulo, hoje em nota 99 e zero colisões.

A partir desta rodada existe o `feedback`, que leva o caso direto aos
desenvolvedores com os mesmos campos que este arquivo vinha guardando à mão. Os
dois casos abertos abaixo já foram enviados por lá.

---

## 34. `hinge_right` não faz efeito, e o erro do `blocks_door` o recomenda

O `check_layout` acusa o choque e instrui o caminho:

```
"A folha da porta bate em Lavatório social f814:
 invertendo o lado da dobradiça ela abre livre."
```

Feito exatamente isso:

```
update(items=[{"id": "f1508", "hinge_right": true}], dry=true)
→ {"dry": true}                     objeto vazio: nem mudança, nem erro

update(items=[{"id": "f1508", "hinge_right": true}])
→ ok rev=4
   mirrored: None  →  None
   angle:      90  →  90
   blocks_door: continua f1510 + f814
```

Nada muda, e nada diz que nada mudou. O dry devolver vazio é o pior caso:
parece "não há o que mudar" quando é "não sei fazer isso".

O que resolveu foi mover a porta 4 cm ao longo da parede — caminho que o erro
não sugere.

**Reproduzir:** os dois comandos acima, e reler a peça pelo `/api/home`.

**Deveria:** `hinge_right` ter efeito em peça com `opening`; ou `update`
recusar o campo com erro, e o texto do `blocks_door` parar de recomendá-lo.

## 32. Cômodo sem porta não entra em relatório nenhum

A planta veio do `.sh3d` com dormitório, suíte, banho social e banho da suíte
**sem porta**: cada um tinha um vão livre (`passage`) e, ao lado, um painel
decorativo chamado "Porta dormitório — aberta junto à parede" (78 × 6 × 208,
sem `opening`), fingindo a folha encostada na parede.

Nenhuma verificação mencionou, o projeto inteiro. E o teste extremo também
passa em branco:

```
delete(ids=["f1508"])        # dormitório fica sem vão e sem porta
check_layout()  → in_wall, outside_rooms, overlap, turned — nada
ergonomics(…)   → score 99, só a dica da NBR 13103 sobre gás
```

Um quarto sem acesso nenhum, nota 99.

Foi o morador que viu, olhando o desenho. E, no instante em que os quatro vãos
viraram portas de verdade, o `blocks_door` acusou que a folha do banho social
batia no lavatório — defeito que estava lá desde sempre, invisível enquanto a
porta era só um móvel desenhado.

**Reproduzir:** o `delete` acima, seguido de `check_layout` e `ergonomics`.

**Deveria:** quarto ou banheiro cujo único acesso é `passage`, ou que não tem
acesso algum, entrar no relatório. Cuidando para não pegar sala, cozinha e
varanda, onde `passage` é o certo.

---

## Sem como reproduzir agora

**O `fix` que desmancha o móvel** (antigo caso 5): o `fix` de um alerta mandava
mover a lava-louças 17 cm, para fora do nicho em que estava embutida, e isso
*melhorava* a nota sem que `outgrew_niche` fosse consultado. A planta hoje não
oferece `fix` em nenhum finding, então não dá para dizer se mudou.

**As anotações que sumiram** (antigo caso 30): depois de uma sequência de
limpeza, `annotations` estava `None` e os números tinham sumido do desenho.
Não reproduzido: `set_properties`, `update` de nível e `update` de móvel, todos
testados, não apagam.

---

## O que já caiu

Verificado em uso, não no changelog — cada um testado na planta com o comando
que o reproduzia. O texto integral de cada caso está no histórico do git.

| # | O que doía | Caiu em |
|---|---|---|
| 1 | Dry run não dizia o tipo do problema que ia criar | `kind` no `issues_new` |
| 2 | Folga `0` valia para "encostado" e "enfiado dentro" | folga negativa |
| 3 | Colisão por caixa sem como declarar convivência | `accept` em `overlap` |
| 4 | `ergonomics` media pelo lado oposto ao que a peça abre | face construída |
| 6 | A folga era o pior ponto, sem a extensão | "54 cm em 34,5 dos 185" |
| 7 | Os números no nome das peças não eram conferidos | `stale` confere nomes |
| 8 | Análises só atrás do MCP; `bounds` sem ângulo no REST | `/api/*` + bounds |
| 9 | Sem `accept` no `check_layout` | `accept` + `orphaned` |
| 10 | `stale` calado não distinguia "tudo certo" de "nada conferido" | `checked: {...}` |
| 11 | `accept` sobrevivia ao achado que o justificava | `orphaned` |
| 12a | `scope=project` ignorava o `q` | `q` respeitado |
| 12b | Score do dry usava a ocupação padrão | usa a da revisão |
| 13 | Cota sem âncora nunca ficava stale | `dims_unanchored` |
| 14 | Rótulo ficava apontando para o vazio após `delete` | `labels_left` |
| 15 | "80,5 cm" era lido como `5` — 28 de 50 achados falsos | vírgula decimal |
| 16 | Resize de grupo invertia a face declarada | face preservada |
| 17 | Grupo escalava as chapas junto: lateral 2 cm → 2,7 | espessura mantida, vão cresce |
| 18 | Parte de grupo não podia ser renomeada | `name` aceito |
| 19 | A sonda atravessava um armário de 280 cm | lê as partes |
| 20 | `cut_list` só enxergava o que o `joinery` fez | `drawn` + `skipped` |
| 21 | `accept` sem chave para `in_wall` | `key` em tudo |
| 22 | Erro de tipo não dizia qual campo | nomeia o campo |
| 23 | Não havia como reusar um modelo do projeto | `cat` aceita id de peça |
| 24 | Peça a 2,72 m avaliada como obstáculo de circulação | ignora o que está alto |
| 25 | `joinery` encolhia pela frente e pelo fundo | fundo fica na parede |
| 26 | `turned` parou de avisar com a face errada | face certa, aviso de volta |
| 27 | `anchor` via dentro dos grupos, `stale` não | `stale` desce nos grupos |
| 28 | Sem busca por padrão; REST não via partes de grupo | `q` acha `[0`; REST alcança |
| 29 | Ligar um modo custava 6k tokens da mesma lista | cada modo responde o seu |
| 33 | Mover uma porta 4 cm a girava 180° | move não gira mais |
| 35 | O número da peça mudava sozinho a cada inserção | número preso à peça |
