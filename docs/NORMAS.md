# Normas e fontes

De onde vêm os números quando o software diz que uma cozinha está errada.

Este documento é a bibliografia do projeto: as normas, a doutrina e a pesquisa
que sustentam cada regra, com edição e link. Ele não é decorativo — o registro
vive em código, em [`crates/newera-core/src/standards.rs`](../crates/newera-core/src/standards.rs),
e cada achado do `ergonomics` carrega o código curto da fonte em que se apoia.
A tabela de cada entrada aqui corresponde a uma entrada lá.

## A escada de confiabilidade

Nem toda fonte tem o mesmo peso, e tratar todas igual é o que produz medida sem
dono. A camada decide quanto um achado pode acusar — é `Tier` no código, e o
teto de severidade é aplicado em `push_ref`, não deixado a critério de quem
escreve a regra.

| Camada | O que é | Teto de severidade |
|---|---|---|
| **A · obriga** | Norma brasileira e código municipal. Descumprir é problema legal ou de segurança. | `Erro` |
| **B · referencia** | Norma estrangeira e diretriz de associação. Boa engenharia, sem força legal aqui. | `Alerta` |
| **C · doutrina** | Arquitetos e fabricantes. Define o que é uma boa cozinha, não o que é uma cozinha legal. | `Dica` |
| **D · mede** | Pesquisa de laboratório. Dado empírico, com método publicado. | `Alerta` |
| **E · descreve** | Survey de mercado. Diz o que as pessoas fazem, nunca o que deveriam fazer. | não gera achado |

Além da camada, cada entrada carrega um `Confidence`. `ConfirmBeforeUse` marca
o que não foi conferido na publicação primária: pode alertar, nunca acusar.
É por isso que uma cozinha de 170 cm sai como `Alerta` contra a especificação
da Caixa, e não como `Erro`, mesmo a regra pedindo erro.

## Regras de honestidade que valem como regra de código

1. **Toda medida tem que ter dono.** Número sem origem não entra em constante.
2. **Norma citada precisa de edição e ano.** "Segundo a ABNT" não é citação. A
   NBR 13103 mudou em 2024; a 15575 em 2021; a 9050 tem emenda de 2020.
3. **Norma paga cujo número não conferimos vira checagem de presença**, não de
   valor. A regra de gás pergunta se existe ventilação; nunca inventa a área
   útil que a edição vigente exige.
4. **Estatística sem metodologia é opinião.** Camada E exige amostra, data de
   campo e recorte — e ainda assim não gera achado.
5. **Fabricante é fonte legítima sobre ergonomia, não sobre necessidade.**
   Aproveite o estudo de movimento; descarte a conclusão de que a solução é a
   gaveta dele.
6. **Dado estrangeiro não é dado local.** Código americano e hábito europeu não
   descrevem cozinha brasileira; entram como camada B, com o rótulo de origem.
7. **Toda regra nasceu num contexto.** O triângulo de trabalho foi calibrado
   para uma cozinha de uma pessoa, sem micro-ondas e sem lava-louças. Saber a
   data de nascimento de uma regra é saber onde ela deixa de valer.

## A · Normas brasileiras

| Código | Fonte | O que governa |
|---|---|---|
| `nbr15575` | ABNT NBR 15575-1:2021 — Norma de Desempenho | Pé-direito mínimo e desempenho dos sistemas |
| `nbr15575g` | ABNT NBR 15575-1:2021, **Anexo G** (informativo) | Mobiliário e equipamento mínimo, e a circulação em volta |
| `nbr9050` | ABNT NBR 9050:2020 (emenda 1:2020) | Aproximação, alcances, alturas de comando, giro de cadeira |
| `nbr13103` | ABNT NBR 13103:2024 (6ª ed.) | Ventilação permanente onde há aparelho a gás, até 80 kW somados |
| `nbr5410` | ABNT NBR 5410 | Um ponto de tomada a cada 3,5 m de perímetro; dois acima da bancada |
| `nbr8995` | ABNT NBR ISO/CIE 8995-1:2013 | Iluminância por atividade, de 50 a 2.000 lx |
| `nbr16280` | ABNT NBR 16280 | Reforma em condomínio: plano, cronograma e responsável técnico |
| `nbr14037` | ABNT NBR 14037 e NBR 5674 | Manual do proprietário e programa de manutenção |
| `nbr14810` | ABNT NBR 14810 | MDP: partículas, 551 a 750 kg/m³, bom em arranque de parafuso |
| `nbr15316` | ABNT NBR 15316 | MDF: fibras de processo seco, usinável de face e de topo |
| `caixa-mcmv` | Caixa — Especificações Mínimas da UH | Cozinha de 1,80 m, pia 120×50, fogão 55×60, geladeira 70×70 |
| `coe-municipal` | Código de obras municipal e Código Sanitário estadual | Áreas, ventilação e o círculo inscrito no piso |
| `rdc216` | ANVISA RDC nº 216/2004 | Cozinha profissional: fora do escopo residencial |

> **Códigos municipais nunca viram constante.** Eles moram em
> `standards::MUNICIPAL_CODES`, indexados por cidade, e o `Profile.city` decide
> se julgam ou apenas avisam. Entre norma e lei local, prevalece o mais
> restritivo. Hoje o registro tem São Paulo capital e o Código Sanitário
> estadual paulista; cidade não informada vira conselho a confirmar.

## B · Normas e códigos estrangeiros

| Código | Fonte | O que governa |
|---|---|---|
| `en1116` | EN 1116:2018 | Larguras nominais de 400, 500, 600 e 900 mm e os nichos de embutir |
| `nkba` | NKBA Kitchen & Bath Planning Guidelines (5ª ed.) | Folgas de trabalho ao lado de fogão e pia, corredores, aéreos |
| `irc2024` | IRC 2024 / NEC 2023 | Bancada a partir de 305 mm exige tomada (contraponto à NBR 5410) |

> **Erro comum:** a DIN 68935 trata de móveis de banheiro, não de cozinha. Para
> cozinha a referência é a EN 1116.

## C · A linhagem de projeto

Cada regra de cozinha que circula hoje nasceu em um destes pontos.

| Ano | Código | Quem, e o que trouxe |
|---|---|---|
| 1926 | — | **Cozinha de Frankfurt**, Margarete Schütte-Lihotzky. 1,90 × 3,44 m, ~10.000 unidades. A origem da cozinha embutida e da ideia de que layout é problema mensurável. |
| 1929 | `gilbreth-triangulo` | **Lillian Moller Gilbreth** apresenta o triângulo de trabalho; o Small Homes Council de Illinois o formaliza em laboratório nos anos 1940. Calibrado para uma cozinha de uma pessoa. |
| 1952 | — | **Charlotte Perriand e Le Corbusier**, Unité d'Habitation: a cozinha-bar aberta, o nascimento da cozinha integrada. |
| 1977 | `alexander184` | **Christopher Alexander**, padrão 184: bancada total ≥ 366 cm fora de pia, fogão e geladeira; nenhum trecho < 122 cm; nenhum par > 305 cm. Mesa solta conta. |
| hoje | `blum-zonas` | **Blum e Hettich**: cinco zonas na ordem do fluxo, e bancada 15–20 cm abaixo do cotovelo flexionado. |
| hoje | `bulthaup-b1` | **bulthaup**: ilha, linha de parede e bloco de torres. Tipologia útil; material de marca, sem método publicado. |
| — | `neufert` | **Neufert** (1936): a origem da bancada de 90 × 60 cm que virou padrão de mercado. |
| — | `panero-zelnik` | **Panero & Zelnik**: alcances e folgas por percentis populacionais. |

## D · Pesquisa de laboratório

| Código | Fonte | O achado |
|---|---|---|
| `lbnl-coifa` | Lawrence Berkeley National Laboratory | Testando sete coifas de US$ 40 a US$ 650, a captura variou de **15 % a 98 %**, e preço não previu desempenho. ~80 % nas bocas traseiras contra ~50 % nas frontais. Vazão sozinha não diz quanto poluente sai de casa. |
| `ibge-adensamento` | IBGE | Acima de três moradores por dormitório o domicílio é adensado. |

## E · Dados de mercado

Presentes no registro como contexto; por construção **não geram achado**.

| Código | Fonte | Recorte declarado |
|---|---|---|
| `houzz2026` | 2026 U.S. Houzz Kitchen Trends Study | 1.780 respondentes americanos, campo em julho de 2025. Renda e envolvimento acima da média; nada transferível ao Brasil. |
| `abimovel` | Abimóvel — Anuário Brasil Móveis | 2024: 22 mil empresas, receita acima de R$ 91,5 bilhões. |

## Onde cada fonte encaixa no software

| Fonte | Superfície | Situação |
|---|---|---|
| `nbr8995` | `lighting` · `newera-core/src/lighting.rs` | integrada desde antes — `recommended_lux` é o modelo que o resto seguiu |
| `nbr9050` | `ergonomics` | giro, vão de porta, alcance de tomadas e aéreos, perfil `wheelchair` |
| `nbr15575` / `nbr15575g` | `ergonomics` | pé-direito, mobiliário mínimo, circulação em volta das peças |
| `coe-municipal` | `ergonomics` + `Profile.city` | círculo inscrito no piso, áreas e janelas |
| `caixa-mcmv` | `ergonomics` | largura mínima de cozinha |
| `nbr13103` | `ergonomics` | aparelho a gás sem abertura permanente |
| `nbr5410` | `ergonomics` | tomadas por perímetro e acima da bancada, onde há projeto elétrico |
| `alexander184` | `ergonomics` | bancada total, menor trecho, distância entre pares |
| `blum-zonas` | `ergonomics` | as cinco zonas, altura de bancada por estatura |
| `gilbreth-triangulo` | `ergonomics` | o triângulo, com o limite do contexto em que nasceu |
| `lbnl-coifa` | `ergonomics` + catálogo (`hood`) | cocção sem captura, coifa mais estreita que a cocção |
| `nkba` | `ergonomics` | 60 cm entre pia e fogão, altura do aéreo sobre a bancada |
| `en1116` | `newera-ergonomics/src/scene.rs` · `cabinet_run` | classifica lava-louças, forno e micro-ondas como eletrodoméstico de nicho, não como bancada; os defaults do `cabinet_run` são as larguras nominais |
| `nbr14810` / `nbr15316` | `cut_list` | definição dos painéis, no `refs` da resposta |
| `nbr16280` / `nbr14037` | — | processo e entrega: metadado de projeto, não geometria |

### Uma nota sobre a EN 1116 e a marcenaria sob medida

A EN 1116 existe para que eletrodoméstico, frente e ferragem de fabricantes
diferentes sejam intercambiáveis numa cozinha industrializada. O `cabinet_run`
produz marcenaria sob medida, com lista de corte e fita de borda: ali, quatro
portas iguais de 61,25 cm num trecho de 245 cm são melhores que 60+60+60+65,
e foi assim que o divisor ficou. A norma entra onde realmente governa — na
classificação dos eletrodomésticos de embutir e nos defaults de módulo — e o
resto é escolha de projeto, não desvio.

---

Dossiê compilado em 15 de setembro de 2026 a partir de busca e leitura direta
das fontes listadas, e trazido para o repositório em seguida. Itens marcados
`ConfirmBeforeUse` no registro não foram confirmados na publicação primária.
