# Atritos de uso

O que dói quando um agente conduz uma planta real pelo MCP, do lado de fora.

Um agente recebe uma planta de verdade e a tarefa de corrigi-la. Cada vez que
uma ferramenta responde menos do que a pergunta pedia, obriga a um contorno, ou
leva a uma conclusão errada antes de levar à certa, o caso entra aqui com a
chamada que se fez, a resposta literal, o que era verdade, o que custou e o que
teria encurtado o caminho. Tudo anotado à mão.

**Este arquivo guarda só o que ainda não foi resolvido.** Cada caso que cai sai
daqui; o que doía em cada um está no histórico do git. Cinquenta e seis já
saíram, todos conferidos em uso na mesma planta e não no changelog.

O banco de provas é o mesmo apartamento de 65 m² desde o começo, com marcenaria
desenhada módulo a módulo. Hoje ele está em **nota 100**, zero achados com
peso, `loose: 0` e `pending: 0` nas três disciplinas.

## Em aberto

Três casos: um achado que anda em círculo, um traçado que a própria ferramenta
não reconhece, e um mecanismo que não deu sintoma na última subida mas continua
sem existir.

## 50. Dois achados que se anulam, e o `fix` que anda em círculo

*Conferido de novo nesta versão: continua igual.*

Restava uma dica de peso 1 no sofá:

```
"36 cm livres à frente (sentar, levantar e circular diante do assento:
 mínimo 50 cm); afaste 14 cm."   f933, peso 1
```

Afastados os 14 cm pedidos, volta:

```
"71 cm livres à frente (circulação diante de bancada e equipamentos:
 mínimo 85 cm); afaste Sofá Milano … f933 14 cm."   f1068, peso 5
 "fix": {"tool": "move", "ids": ["f933"], "dx": 0.0, "dy": 14.0}
```

O `fix` é exatamente o movimento inverso do que acabou de ser feito, e a nota
vai de 99 a 94. Os dois achados disputam o mesmo vão entre a península da
cozinha e o rack da sala: 50 cm na frente do sofá mais 85 cm na frente da
bancada dão 135 cm, e o vão não tem. Não existe posição que satisfaça os dois,
e nenhuma das duas mensagens diz isso.

**Reproduzir:** `move(ids=["f933"], dx=0, dy=-14)` e rodar `ergonomics`.

**Deveria:** quando o `fix` de um achado criaria outro de peso igual ou maior,
dizer na própria mensagem — *"não cabem os dois: as duas folgas somam 135 cm e
o vão não tem; a posição atual é a melhor das duas"*. Um achado que só pode ser
aceito deveria nascer sabendo disso.

## 58. O `route` de ventilação desenha um ramal que o próprio `check` recusa

Caso novo, e ele nasceu de uma correção: o `plumb:vent-far` agora ensina o
caminho — *"trace o ramal com route kind=vent e ele passa a contar"*. Traçado:

```
plumbing(action="route", kind="vent",
         ids=["f1591","f1598","f1589","f1596","f1594","f1601"], via="wall")
→ {"via":"wall", "length_m":{"total":20.4}, "branches":5,
   "run":"vent:f1589+f1591+f1594+f1596+f1598+f1601",
   "materials":[["Tubo PVC esgoto série normal 50 mm (ramal de ventilação)",21.4,"m"], …]}
```

O ramal foi desenhado, e `pipes_m` passou a contar `"vent": 20.4`. O check, no
passo seguinte, sem nenhuma edição no meio:

```
plumbing(check)
→ ["alerta","Ventilação","Pontos sem tubulação chegando:
    f1601, f1596, f1598, f1611, f1609, f1612, f1629, f1594, f1606, f1604."]
```

Os seis que ele acabou de traçar estão na lista. Medindo as polilinhas que ele
mesmo escreveu contra os pontos que ele mesmo escolheu:

| ponto | distância à polilinha |
|---|---|
| f1591 · lavatório social | 25,5 cm |
| f1589 · vaso social | 27,5 cm |
| f1596 · vaso suíte | 33,0 cm |
| f1598 · lavatório suíte | 33,0 cm |
| f1594 · ralo box social | 49,0 cm |
| f1601 · ralo box suíte | 50,0 cm |

O traçado corre no eixo da parede (`[[30,496],[126,496],[200,496]]`), que é
onde um ramal embutido corre de verdade, e a verificação de alcance não aceita
essa distância. Nas outras espécies isso não acontece: depois de
`route kind=data`, `kind=power` e `kind=cold`, os `unreached` correspondentes
esvaziam.

**Custou:** duas chamadas de `route`, um script para medir ponto a polilinha, e
um aceite escrito à mão para um achado que é da ferramenta, não do projeto.

**Efeito colateral encontrado no caminho:** duas chamadas de `route vent` com
subconjuntos diferentes de `ids` criam dois runs que se empilham
(`vent:a+b+c` e `vent:b+c`), somando 27,6 m de tubo onde há 20,4, e nenhum
`replaced_drawn` avisa — porque o nome do run vem dos ids. Quando o segundo
conjunto está contido no primeiro, deveria substituir.

**Deveria:** a verificação de alcance da ventilação medir contra o run, com a
mesma folga que as outras espécies usam — ou o `route` recusar traçar o que o
`check` não vai aceitar, em vez de entregar um traçado e cobrá-lo em seguida.

## 48. A aceitação órfã não aponta para o achado que a sucedeu

*Sem sintoma na última subida de versão — nenhuma das 20 aceitações se perdeu.
Fica aberto porque a rede de proteção continua não existindo.*

Quando uma regra passa a citar a fonte, a chave do achado ganha o prefixo
(`-:f807:livres-frente-passagem-cama` virou
`nbr15575g:f807:livres-frente-passagem-cama`). É a mesma regra, sobre a mesma
peça, com o mesmo sufixo — e a aceitação escrita à mão vira órfã, o achado
volta aberto, e a nota cai sem explicação.

O `orphaned` continua devolvendo só o par, testado com uma aceitação inventada:

```
plumbing(check, accept=[["plumb:vent-far:f9999", "teste de orfandade"]])
→ "orphaned": [["plumb:vent-far:f9999", "teste de orfandade"]]
```

Nada liga a órfã ao achado que ocupou o lugar dela.

**Deveria:** casar órfã e achado novo pelo sufixo (peça + regra) e reaproveitar
o motivo, dizendo que a chave mudou — ou, no mínimo, apontar no `orphaned` o
achado que provavelmente o sucedeu. A chave é identidade; se ela carrega a
fonte, ela muda quando a fonte muda.
