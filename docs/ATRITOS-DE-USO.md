# Atritos de uso

O que dói quando um agente conduz uma planta real pelo MCP, do lado de fora.

Este documento é escrito de fora para dentro: um agente recebeu uma planta de
apartamento pronta — 65 m², 10 ambientes, marcenaria desenhada módulo a módulo —
e a tarefa de corrigi-la inteira. Cada vez que uma ferramenta respondeu menos do
que a pergunta pedia, obrigou a um contorno, ou levou a uma conclusão errada
antes de levar à certa, o caso foi anotado aqui com o que aconteceu de fato.

Não é uma lista de desejos. Cada entrada traz o que se tentou, o que voltou, e
o que teria encurtado o caminho — na ordem em que apareceram, com o caso
concreto que os produziu, para que se possa reproduzir.

## 1. O dry run não diz que tipo de problema vai criar

`check_layout` classifica cada sobreposição em `collision` (um choque de
verdade), `nesting` (embutido, apoiado, encaixado) ou `cross_level`, e dá a
extensão `[x,y,z]` do espaço comum. É essa classificação que torna o relatório
legível: dezenove `nesting` e um `collision` é um relatório tranquilo.

Os dry runs de `move` e `update` não classificam. Ao testar um deslocamento da
cuba, a resposta foi:

```
"issues_new": ["f817+f830"], "issues_resolved": ["f830+f833"]
```

Dois pares de ids e nada mais. Trocar um choque real por um encosto legítimo é
um bom negócio; trocá-lo por outro choque não é — e a resposta é idêntica nos
dois casos. O caminho que sobra é aplicar, rodar `check_layout`, ler a
classificação e desfazer, que é exatamente a viagem que o dry run existe para
evitar.

**Encurtaria:** `issues_new` carregar o mesmo `kind` e `extent` que
`check_layout` já calcula.

## 2. Folga 0 quer dizer duas coisas opostas

No mesmo dry run, `clearances` reportou `"-x": [0, "f817", "Península — pedra
esquerda"]` para um movimento de 5,5 cm **para dentro** daquela peça. Antes do
movimento a folga também era 0 — as duas pedras se encostam.

Ou seja: "encostado, perfeito" e "enfiado 5,5 cm dentro" são o mesmo número. A
sobreposição só apareceu por dedução, comparando os `bounds` na mão. O próprio
`measure` já faz o certo em outro lugar — com `axis`, a distância entre duas
caixas é negativa quando elas se sobrepõem.

**Encurtaria:** folga negativa em `clearances`, como `measure` já faz.

## 3. Um modelo importado colide pela caixa, e a caixa não é a peça

A cuba da cozinha (`sink-upper.obj`, 70 cm) aparecia em `collision` com a
lava-louças — 5,4 cm de sobreposição. Só que os módulos embaixo estavam
encadeados sem folga nenhuma: gabinete da pia termina em 474, lava-louças de
474,6 a 534,4, torre quente a partir de 535. O aparelho de 59,8 cm está num
nicho de 61 cm, exatamente onde deveria.

Quem invadia era a **caixa** do modelo da cuba, que é uma pedra inteira de 70 cm
com a cuba desenhada no meio. Não existe posição do aparelho que evite essa
caixa: entre a pedra da direita e a torre sobram 55 cm para um aparelho de 59,8.
A saída foi estreitar a peça de 70 para 64,5 cm — isto é, **distorcer o modelo
em 8% para calar um alarme falso**.

Um `embed` existe justamente para dizer "isto está encaixado naquilo", mas ele
constrói o encaixe (recorte na pedra, nicho no armário); não serve para
declarar que duas peças já desenhadas convivem.

**Encurtaria:** poder marcar um par como convivência aceita — o que
`ergonomics` já permite com `accept=[[key, motivo]]` e `check_layout` não
permite com nada. O motivo ficaria no projeto, revisável, em vez de a peça
ficar 8% mais estreita para sempre.

## 4. `ergonomics` e `measure` discordam sobre onde é a frente da peça

O caso mais caro do dia. O gabinete da pia (`f1165`) recebeu:

```
"1 cm livres à frente (circulação diante de bancada: mínimo 85 cm); afaste 84 cm."
```

Um centímetro. Mas `measure(from=f1165)` responde:

```
"faces": "-y",  "-y": [65.2, "f828", "Geladeira"],  "+y": [0, "f830", "Pia"]
```

A peça abre para `-y`, onde há 65,2 cm de corredor. O `1 cm` foi medido para
`+y` — contra a própria pedra que é o tampo dela. Duas ferramentas do mesmo
servidor, lendo o mesmo desenho, discordam sobre para que lado o móvel olha:
`measure` usa a face construída, `ergonomics` usa o `angle`.

`check_layout` **sabe** da discordância e a reporta em `turned`, com um conselho
claro: *"the piece opens where the panels are, so fix the angle, not the
clearance it seems to lack"*.

O problema é que não há como seguir o conselho. `update(angle=180)` não corrige
a declaração: `changed_count: 54` — gira o grupo inteiro e leva as portas para
o lado errado, que hoje estão certas (conferido na elevação, `render_3d
view=back cut=410`: as frentes estão todas voltadas para o corredor). Fica-se
entre um metadado que mente e uma correção que estraga a marcenaria.

Foram dois alertas falsos (`f1165`, `f1076`) puxando a nota para baixo, e os
dois precisaram ser aceitos à mão com a medida como justificativa.

**Encurtaria:** um jeito de acertar o `angle` sem reconstruir o grupo — ou
`ergonomics` perguntando a face construída, como `measure` já faz.

## 5. O `fix` sugerido resolve a métrica desmanchando o móvel

Vários achados vêm com `fix`, "a checked change as tool arguments". O da
lava-louças:

```json
{"tool": "move", "ids": ["f833"], "dx": 0, "dy": 17}
```

Dezessete centímetros para dentro do corredor — isto é, **para fora do nicho de
marcenaria em que o aparelho está embutido**. Rodado em dry:

```
"resolved": [4 alertas],  "score": [51, 52]
```

Quatro alertas resolvidos, nota melhor, e nenhum aviso de que a máquina saiu do
armário: não aparece `outgrew_niche`, não aparece overlap novo, nada. A
verificação que existe exatamente para isso (`outgrew_niche`, "an appliance its
host stopped holding") não é consultada pelo `fix` que causou o problema.

Seguir os `fix` em sequência, confiando na palavra "checked", desmancharia a
cozinha aumentando a nota a cada passo.

**Encurtaria:** o `fix` não ser oferecido quando tira uma peça embutida do seu
hospedeiro, ou pelo menos dizer isso no dry run.

## 6. A folga é o pior ponto, e não diz de que extensão

"54 cm livres à frente" no guarda-roupa da filha soa como um armário que não
abre. Sondando a frente dele em `y=120`:

```
[99, 192, null, ""]   → 93 cm livres
```

Noventa e três. Os 54 cm existem só na faixa de 38 cm da mesa de cabeceira, no
canto junto à janela, onde não se abre gaveta nenhuma. O mesmo na suíte (94 cm
no corpo, 55 no canto) e aos pés da cama da filha: o alerta diz 13 cm, a sonda
em `x=85` mostra 58,7 cm — os 13 são só onde está a escrivaninha.

Três alertas que descrevem um apartamento pior do que ele é. Todos exigiram
sonda manual para virar decisão.

**Encurtaria:** a folga vir com a extensão em que ela vale ("54 cm em 38 dos
185 cm de frente"). É a diferença entre um móvel inutilizável e um canto
apertado.

## 7. Os números que o projeto guarda no nome não são verificados por ninguém

Esta planta descreve a marcenaria nos **nomes** das peças: "módulo 70 cm",
"nicho 61 × 87 cm", "aéreo de 118 cm; duas folhas de 59", "vão 45 L × 56 P ×
85 A". É onde a informação de obra realmente mora — quem vai cortar a chapa lê
o nome.

`annotations(stale=true)` respondeu `{"stale": []}`, o que parece um atestado de
saúde e não é: ele só confere labels amarrados com `about`, e **nenhum dos 277
labels da planta tem `about`** — são códigos do índice (`[09]`, `[10]`). Não
havia nada para conferir, e a resposta vazia é idêntica à de uma planta
verificada.

Pior: neste mesmo trabalho a cuba foi redimensionada de 70 para 64,5 cm. Se o
nome dela dissesse "70 cm", passaria a mentir em silêncio.

Foi preciso escrever um script
([`scripts/aux/conferir-medidas-nos-nomes.py`](../scripts/aux/conferir-medidas-nos-nomes.py))
que lê a planta pela API REST, extrai os números dos nomes e compara com a
geometria. As seis peças que ele apontou são todas legítimas — e é o achado
interessante: os números se referem a **vão interno**, **conjunto** e **folha de
porta**, coisas que o modelo não tem onde guardar. Por isso vão para o nome.

**Encurtaria:** o `stale` dizer quantas notas conferiu (zero é uma resposta
diferente de "tudo certo"), e haver onde declarar um vão livre e a largura de
um conjunto — hoje só sobra o nome.

## 8. A API REST lê e escreve, mas não pensa

`GET /api/home` devolveu a planta inteira (351 KB) sem custo nenhum de contexto,
e é o que tornou os scripts auxiliares possíveis. Há `POST /api/commands` para
escrever, `/api/plan.png`, `/api/view.png`, `/api/events`.

O que não há é análise: `check_layout`, `ergonomics`, `measure` existem só
atrás do MCP, e `POST /mcp` sem handshake de sessão responde `422`. Um script
auxiliar pode **editar** a planta, mas não pode perguntar se o que ele fez
ficou bom — tem que devolver a pergunta ao agente.

Uma armadilha junto: no JSON do REST, `width`/`depth` são as medidas da peça
**sem o ângulo aplicado**. Calcular a caixa na mão deu, num armário girado 90°,
`x 128,5..313,5` onde a caixa real é `x 192..250`. O MCP entrega `bounds` já
girado; o REST não, e nada avisa.

**Encurtaria:** expor as análises como `GET /api/check`, `GET /api/ergonomics`,
e mandar `bounds` junto no JSON.

## 9. `check_layout` não tem como aceitar um caso analisado

`ergonomics` tem `accept=[[key, motivo]]`: o achado continua no relatório, com a
razão escrita, e para de custar nota. Foi assim que esta planta saiu de 50 para
91 sem varrer nada para baixo do tapete — cada aceite carrega a medida que o
justifica.

`check_layout` não tem equivalente. As persianas integradas (`f822`, `f824`)
aparecem em `in_wall` porque estão dentro da espessura da parede, que é
exatamente onde uma persiana de rolo embutida fica. O shaft e os vidros do
escritório aparecem em `outside_rooms` por não estarem dentro de nenhum
polígono de cômodo, o que também está certo. São cinco linhas de relatório que
vão continuar ali para sempre, relidas a cada revisão, sem como dizer "visto,
está correto".

**Encurtaria:** `accept` em `check_layout`, igual ao do `ergonomics`.

---

## O que ficou por decidir (não é atrito, é do morador)

- **Escritório sem janela** (peso 5, o maior aberto): 3 m² fechados em vidro
  canelado, com uma parede externa à esquerda. Abrir janela ali é obra de
  fachada.
- **Corredor da cozinha, 69 cm**: as bancadas norte foram desenhadas com 84-85
  cm de profundidade. Recuá-las para 65 cm levaria o corredor a ~88 cm e
  resolveria oito alertas de uma vez, ao custo da bancada e de reposicionar
  cooktop, coifa e gavetões.
- **Cidade**: sem ela, o código de obras municipal só aconselha. `set_home(city=...)`.
- **Sala**: 2 lugares para 3 moradores.
