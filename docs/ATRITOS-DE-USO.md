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

## 10. A cota que vira mentira não é detectada, e a proteção só serve antes

Recuar a bancada da cozinha de 84 para 65 cm deixou a cota do corredor dizendo
`66,5` sobre um vão que passara a ter 88. É o erro mais caro que uma planta de
obra pode ter: quem for executar lê a cota.

`annotations(stale=true)` respondeu `{"stale": []}` **depois** da mudança.

A razão está na própria ferramenta: uma cota só é reconferida se tiver sido
ancorada antes, com `anchor=true` — *"run it while the numbers are still
right"*. Nesta planta nenhuma cota estava ancorada, então não havia o que
conferir, e a resposta vazia foi indistinguível de "tudo certo".

Pior: a proteção não é aplicável depois. Rodar `anchor=true` com a cota já
errada **congela o erro** — ela passa a seguir o desenho a partir de um número
que já não corresponde. É preciso corrigir à mão primeiro (foram quatro cotas:
`d95`, `d102`, `d103`, `d105`) e só então ancorar, que foi o que se fez: 20
cotas ancoradas, e daqui em diante elas se remedem sozinhas.

**Encurtaria:** `stale` dizer quantas cotas conferiu e quantas estão sem
âncora — "0 de 29 ancoradas" é uma resposta muito diferente de `[]`. E o
`anchor` recusar, ou ao menos avisar, quando a cota que vai ancorar já não bate
com o que ela mede.

## 11. Um `accept` sobrevive ao problema que o justificava

Oito achados de circulação da cozinha foram aceitos com a medida do corredor de
69 cm escrita como razão. Depois, a bancada foi recuada e **os oito
desapareceram de verdade** — o corredor passou a ter 88 cm.

As oito justificativas continuaram gravadas no projeto, agora órfãs. Não
aparecem em lugar nenhum do relatório (o achado não existe mais), e nada avisa
que estão lá. Se um dia alguém reaproximar a bancada, o problema volta **já
silenciado**, com peso 0 e uma razão de 2026 explicando um corredor que não
existe mais.

Foi preciso limpá-las à mão, uma a uma, com `accept=[[key, ""]]` — e só porque
se sabia quais tinham sido aceitas.

**Encurtaria:** listar os aceites órfãos ("8 aceites não correspondem a nenhum
achado atual"), ou expirá-los quando o achado some.

## 12. Dois atritos pequenos que custam caro em tokens

**`catalog(scope=project)` ignora o `q`.** Pedir `q="janela"` com
`scope="project"` devolveu as 200 e poucas entradas do projeto inteiro, da
pétala de flor ao puxador de latão. O parâmetro existe na mesma chamada e é
silenciosamente descartado.

**O `score` do dry run não usa os parâmetros da revisão.** A planta estava em
91 com `occupants=3, children=1`; os dry runs relatavam `"score": [92, 87]` —
87 é a nota com os defaults (2 ocupantes). Não há onde informar a ocupação num
dry, então o número que ele devolve não é comparável com o da revisão que se
está conduzindo. Dá para usar o *sinal* (subiu/desceu), nunca o valor.

---

## O que a revisão fez na planta

De 50 para 97 de nota, sem esconder nada — cada aceite carrega a medida que o
justifica, e os que deixaram de fazer sentido foram removidos.

- **Choque real na bancada** (o único `collision` da planta): era a caixa do
  modelo da cuba, não a peça. Caixa ajustada ao vão que ela ocupa.
- **Cozinha corredor**: a bancada norte tinha 84-85 cm de profundidade e
  deixava 69 cm de passagem. Recuada para 65 cm — armários, pedra, cooktop,
  coifa e arremates —, o corredor passou a 88 cm e **oito alertas de circulação
  deixaram de existir**. A geladeira, que tem 74,5 cm e não encolhe, foi
  encostada a 5 cm da parede (o mínimo do fabricante): 66,5 → 75,2 cm.
- **Escritório**: janela de 100 × 110 cm na fachada oeste, peitoril a 100 cm
  acima da bancada. 1,1 m² de vão para 3 m² de piso.
- **Cotas**: quatro corrigidas e 20 ancoradas — de agora em diante elas seguem
  o desenho.
- **Camada de referência**: a planta antiga estava na mesma elevação da nova e
  entrava nos checks.

## O que ficou por decidir (não é atrito, é do morador)

- **Cidade**: não há pista dela em lugar nenhum do projeto, e chutar faria o
  código de obras errado julgar a planta. `set_home(city=...)`.
- **Terceiro lugar na sala**: 10,2 m² não comportam uma poltrona solta —
  testada em três posições, todas estrangulam a passagem diante do sofá. O
  caminho é trocar o retrátil de 2 lugares por um de 3 no mesmo vão de 200 cm.
- **Ventilação do aparelho a gás**: conferir contra a edição vigente da
  NBR 13103, cujos valores mudaram entre edições.
