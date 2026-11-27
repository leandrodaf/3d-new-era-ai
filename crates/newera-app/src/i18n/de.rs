//! German editor catalog, keyed by the original Portuguese text.

#[allow(clippy::too_many_lines, clippy::match_same_arms)]
pub(super) fn translation(pt: &str) -> Option<&'static str> {
    Some(match pt {
        " · arco" => " · Bogen",
        " (cópia)" => " (Kopie)",
        "Nova casa" => "Neues Projekt",
        "Versão {}" => "Version {}",
        "A tarefa termina sozinha em segundo plano; o resultado é descartado." => {
            "Die Aufgabe läuft im Hintergrund zu Ende. Das Ergebnis wird verworfen."
        }
        "Abrindo imagens e modelos" => "Bilder und Modelle werden geöffnet …",
        "Abrindo projeto" => "Projekt wird geöffnet …",
        "Desenhando a planta" => "Grundriss wird gezeichnet …",
        "Exportando" => "Export läuft …",
        "Extraindo imagens e modelos" => "Bilder und Modelle werden extrahiert …",
        "Forçar e encerrar a espera" => "Wartevorgang sofort beenden",
        "Gravando o projeto" => "Projektdatei wird geschrieben …",
        "Guardando imagens e modelos" => "Bilder und Modelle werden gespeichert …",
        "Importando modelo" => "Modell wird importiert …",
        "Lendo o arquivo" => "Datei wird gelesen …",
        "Lendo o projeto" => "Projekt wird gelesen …",
        "Montando o modelo 3D" => "3D-Modell wird aufgebaut …",
        "Parando…" => "Vorgang wird beendet …",
        "Salvando projeto" => "Projekt wird gespeichert …",
        "Trabalhando…" => "Vorgang läuft …",
        "⚠ Espera encerrada — a tarefa termina em segundo plano." => {
            "⚠ Warten beendet. Die Aufgabe läuft im Hintergrund weiter."
        }
        "Ajustar ao telhado" => "An das Dach anpassen",
        "Código de obras" => "Örtliche Bauvorschriften",
        "Embutir peça no móvel selecionado" => "Element in das ausgewählte Möbelstück einbauen",
        "Embutido:" => "Eingebaut:",
        "Armários na parede" => "Schränke entlang der Wand",
        "Armários na parede…" => "Schränke entlang der Wand …",
        "Lista de corte…" => "Zuschnittliste …",
        "Lista de corte" => "Zuschnittliste",
        "Nenhuma marcenaria neste pavimento: crie armários (Planta → Armários na parede…) ou desenhe um grupo com chapas." => {
            "Keine Schreinerarbeiten in diesem Geschoss: Schränke anlegen (Grundriss → Schränke entlang der Wand …) oder eine Gruppe aus Platten zeichnen."
        }
        "{} peça de marcenaria|{} peças de marcenaria" => {
            "{} Schreinerarbeit|{} Schreinerarbeiten"
        }
        "{} grupo desenhado|{} grupos desenhados" => "{} gezeichnete Gruppe|{} gezeichnete Gruppen",
        "{} e {}." => "{} und {}.",
        "Salvar como" => "Speichern als",
        "CSV: a tabela para a planilha ou o serviço de corte. DXF e SVG: as peças dispostas nas chapas, para a seccionadora ou o CNC." => {
            "CSV: die Tabelle für eine Tabellenkalkulation oder einen Zuschnittservice. DXF und SVG: die Teile auf den Platten angeordnet, für die Formatkreissäge oder die CNC."
        }
        "Chapa da marcenaria" => "Platte der Werkstatt",
        "Serra" => "Sägeschnitt",
        "Refilo por lado" => "Besäumung je Seite",
        "{}: {}" => "{}: {}",
        "{} peça|{} peças" => "{} Teil|{} Teile",
        "{} chapa|{} chapas" => "{} Platte|{} Platten",
        "Maior que a chapa, prever emenda ou chapa maior: {}" => {
            "Größer als die Platte, Stoß oder größere Platte vorsehen: {}"
        }
        "Nome da peça" => "Teil",
        "Qtd" => "Anz.",
        "Comprimento × largura" => "Länge × Breite",
        "Fita" => "Kantenumleimer",
        "Ferragens" => "Beschläge",
        "Fora da lista, não é chapa: {}" => "Nicht in der Liste, keine Platte: {}",
        "Lista de corte salva em {}" => "Zuschnittliste gespeichert unter {}",
        "Chapa da caixa" => "Korpusplatte",
        "Chapa das frentes" => "Frontplatte",
        "a mesma da caixa" => "wie der Korpus",
        "Portas basculantes (abrem para cima)" => "Klappen (öffnen nach oben)",
        "Balcões" => "Unterschränke",
        "Aéreos" => "Oberschränke",
        "Entre a norma e a lei do município, prevalece o mais restritivo." => {
            "Wenn die Norm und die örtliche Bauordnung voneinander abweichen, gilt die strengere Anforderung."
        }
        "Fontes desta revisão:" => "Grundlagen der Prüfung:",
        "Não informado" => "Nicht festgelegt",
        "Referências brasileiras onde existe norma; áreas e janelas variam com o código de obras do município." => {
            "Wo vorhanden, dienen brasilianische Normen als Grundlage. Anforderungen an Flächen und Fenster richten sich nach der örtlichen Bauordnung."
        }
        "Torres / guarda-roupa" => "Hochschränke / Kleiderschrank",
        "Pia centrada em" => "Mitte der Spüle bei",
        "Cooktop centrado em" => "Mitte des Kochfelds bei",
        "descreve" => "beschreibt",
        "do início da parede" => "gemessen ab Wandbeginn",
        "Bancada de pedra por cima" => "Arbeitsplatte aus Stein",
        "Frentes" => "Möbelfronten",
        "Gaveteiros" => "Schubladenschränke",
        "automático" => "automatisch",
        "Mede o trecho livre entre cantos, portas, janelas e eletrodomésticos e divide em módulos sem sobras; substitui os armários que já estão nessa parede." => {
            "Ermittelt die freie Wandlänge zwischen Ecken, Türen, Fenstern und Geräten und teilt sie passgenau in Schrankmodule auf. Vorhandene Schränke an dieser Wand werden ersetzt."
        }
        "Pré-visualizar" => "Vorschau",
        "Criar armários" => "Schränke erstellen",
        "Criado (Ctrl+Z desfaz):" => "Erstellt (Strg+Z macht den Vorgang rückgängig):",
        "Prévia:" => "Vorschau:",
        "doutrina" => "Lehre",
        "mede" => "misst",
        "obriga" => "schreibt vor",
        "portas" => "Türen",
        "gaveteiro" => "Schubladenschrank",
        "porta-temperos" => "Gewürzauszug",
        "canto cego" => "Tote Ecke",
        "cabideiro" => "Kleiderstange",
        "referencia" => "verweist auf",
        "sobre a geladeira" => "Über dem Kühlschrank",
        "pia" => "Spüle",
        "tamponamento" => "Passblende",
        "bancada" => "Arbeitsplatte",
        "Ergonomia" => "Ergonomie",
        "Ergonomia…" => "Ergonomie …",
        "Moradores" => "Bewohner",
        "Crianças" => "Kinder",
        "Idosos" => "Ältere Personen",
        "Altura de quem cozinha" => "Körpergröße der kochenden Person",
        "Alguém usa cadeira de rodas (NBR 9050)" => "Eine Person nutzt einen Rollstuhl (NBR 9050)",
        "Incluir elétrica na nota" => "Elektroplanung in die Bewertung einbeziehen",
        "Incluir hidráulica na nota" => "Sanitärplanung in die Bewertung einbeziehen",
        "Pendências fora do escopo continuam visíveis. A nota não certifica o projeto." => {
            "Offene Punkte außerhalb der Bewertung bleiben sichtbar. Die Bewertung ersetzt keine Zertifizierung des Projekts."
        }
        "Fora do escopo da nota" => "Außerhalb des Bewertungsumfangs",
        "nota de habitabilidade" => "Bewertung der Wohnqualität",
        "lugares para dormir" => "Schlafplätze",
        "banheiros" => "Badezimmer",
        "à mesa" => "Sitzplätze am Tisch",
        "na sala" => "Sitzplätze im Wohnzimmer",
        "de guarda-roupa" => "Kleiderschrank",
        "Nada a apontar para esses moradores." => "Keine Auffälligkeiten für diese Bewohner.",
        "Selecionar na planta" => "Im Grundriss auswählen",
        "Aplicar correção" => "Korrektur anwenden",
        "Correção aplicada (Ctrl+Z desfaz)." => {
            "Korrektur angewendet (Strg+Z macht den Vorgang rückgängig)."
        }
        "Referências: NBR 9050, NBR 15575-1 e IBGE; áreas e janelas variam com o código de obras do município." => {
            "Referenzen: NBR 9050, NBR 15575-1 und IBGE. Flächen und Fenster variieren je nach Bauordnung der Gemeinde."
        }
        " · luz" => " · Licht",
        "(cópia)" => "(Kopie)",
        "Abrir" => "Öffnen",
        "Abrir (Ctrl+O)" => "Öffnen (Strg+O)",
        "Abrir link" => "Link öffnen",
        "Abrir…" => "Öffnen …",
        "Adicionar texto" => "Text einfügen",
        "Adicionar um andar acima do mais alto" => "Etage über der höchsten hinzufügen",
        "Afastamento" => "Versatz",
        "Afastar (Ctrl -)" => "Verkleinern (Strg+−)",
        "Ajuda" => "Hilfe",
        "Quadro de cargas e NBR 5410" => "Stromkreisübersicht nach NBR 5410",
        "Nenhum circuito: atribua os pontos a circuitos (MCP electrical assign)." => {
            "Keine Stromkreise. Weisen Sie die Anschlusspunkte über den MCP-Befehl `electrical assign` einem Stromkreis zu."
        }
        "Circuito" => "Stromkreis",
        "Pontos" => "Anschlusspunkte",
        "VA" => "VA",
        "V" => "V",
        "A" => "A",
        "Fio mm²" => "Leiterquerschnitt in mm²",
        "Disjuntor" => "Leitungsschutzschalter",
        "DR" => "FI-Schutzschalter",
        "Total instalado:" => "Installierte Gesamtleistung:",
        "Nada a apontar." => "Keine Auffälligkeiten.",
        "Camadas" => "Ebenen",
        "Iluminação" => "Beleuchtung",
        "Eletrodomésticos" => "Haushaltsgeräte",
        "Marcenaria" => "Möbelbau",
        "Mostrar ou esconder na planta e no 3D" => {
            "Im Grundriss und in der 3D-Ansicht ein- oder ausblenden"
        }
        "Mostrar tudo no 3D" => "In 3D alles anzeigen",
        "Ligado, o 3D mostra a obra inteira; desligado, esconde o que a planta esconde" => {
            "Aktiviert zeigt die 3D-Ansicht das gesamte Gebäude. Deaktiviert übernimmt sie die Sichtbarkeit aus dem Grundriss."
        }
        "Enviar relatórios de erro" => "Fehlerberichte senden",
        "Falhas e notas de uso vão para os desenvolvedores, sem o seu projeto, sem IP e sem nome da máquina." => {
            "Absturzberichte und Nutzungshinweise gehen an das Entwicklungsteam. Projekt, IP-Adresse und Gerätename werden dabei nicht übermittelt."
        }
        "Ajustes…" => "Einstellungen…",
        "Alinhamento" => "Ausrichtung",
        "Altura" => "Höhe",
        "Andar" => "Geschoss",
        "Aproximar (Ctrl +)" => "Vergrößern (Strg +)",
        "Apoiar o projeto" => "Projekt unterstützen",
        "Apoiar no Buy Me a Coffee" => "Auf Buy Me a Coffee unterstützen",
        "Abrir o GitHub" => "GitHub öffnen",
        "Agora não" => "Nicht jetzt",
        "Nova versão {} disponível" => "Neue Version {} verfügbar",
        "Muita gente usa o 3D New Era AI todos os dias, mas quase ninguém apoia o projeto." => {
            "Viele Menschen nutzen 3D New Era AI jeden Tag. Das Projekt erhält bisher nur wenig Unterstützung."
        }
        "As IAs que usam a ferramenta enviam sozinhas relatos de problemas e sugestões de melhoria, e eles chegam em grande número. Quero corrigir todos, mas me falta tempo e dinheiro para pagar as ferramentas de IA e os tokens que esse trabalho exige." => {
            "KI-Anwendungen senden selbstständig viele Fehlerberichte und Verbesserungsvorschläge. Ich möchte sie alle bearbeiten. Dafür fehlen mir Zeit und Geld für die nötigen KI-Werkzeuge und deren Nutzung."
        }
        "Se a ferramenta é útil para você, considere apoiar no Buy Me a Coffee." => {
            "Wenn Ihnen das Programm hilft, können Sie das Projekt auf Buy Me a Coffee unterstützen."
        }
        "Também dá para ajudar com código: abra issues e pull requests no GitHub. Todos serão avaliados, e o que for aceito entra na próxima versão." => {
            "Auch Beiträge auf GitHub helfen. Dort können Sie Fehler melden und Änderungen vorschlagen. Ich prüfe alle Beiträge. Angenommene Änderungen fließen in die nächste Version ein."
        }
        "Arco" => "Bogen",
        "Arquitetura" => "Architektur",
        "Arquivo" => "Datei",
        "Arraste para mover a vista" => "Ansicht per Drag-and-Drop verschieben",
        "Arraste: girar · Shift/botão do meio: mover · Scroll: zoom · F: enquadrar" => {
            "Ziehen: drehen · Umschalt- oder Mitteltaste: verschieben · Mausrad: zoomen · F: Ansicht einpassen"
        }
        "Atalhos e ferramentas" => "Tastenkürzel und Werkzeuge",
        "Automático (símbolos e vista de cima dos modelos)" => {
            "Automatisch (Symbole, Oberansichten für Modelle)"
        }
        "Autor" => "Autor",
        "Boa" => "Gut",
        "Buscar: cama, janela, sofá…" => "Suche: Bett, Fenster, Sofa …",
        "Calibrar e posicionar" => "Skalierung kalibrieren und platzieren",
        "Calibrar escala da imagem" => "Bildskalierung kalibrieren",
        "Cancelar" => "Abbrechen",
        "Casa e bússola" => "Haus und Kompass",
        "Casa e bússola…" => "Haus und Kompass …",
        "Catálogo" => "Katalog",
        "Catálogo de origem" => "Quellenkatalog",
        "Centro" => "Mitte",
        "Centro (x, y)" => "Mitte (x, y)",
        "Clique dois pontos de medida conhecida · arraste para posicionar a imagem" => {
            "Zwei Punkte mit bekanntem Abstand anklicken. Das Bild anschließend an die gewünschte Stelle ziehen."
        }
        "Clique e depois clique na planta para posicionar" => {
            "Auswählen und dann im Grundriss an der gewünschten Stelle klicken."
        }
        "Clique encadeia paredes · digite o comprimento + Enter · Shift desliga o ímã · duplo clique encerra" => {
            "Mit Klicks Wände aneinanderreihen · Länge eingeben + Eingabetaste · Umschalttaste deaktiviert das Einrasten · Doppelklick beendet"
        }
        "Clique início e fim, mova para afastar e clique · duplo clique numa parede cota a parede" => {
            "Anfang und Ende anklicken, Maßlinie versetzen und absetzen · Doppelklick auf eine Wand bemaßt sie direkt"
        }
        "Clique onde o texto deve ficar" => "Klicken Sie dort, wo der Text stehen soll",
        "Clique os cantos e duplo clique fecha · duplo clique dentro de paredes detecta o cômodo" => {
            "Eckpunkte anklicken und die Kontur per Doppelklick schließen · Doppelklick innerhalb der Wände erkennt den Raum"
        }
        "Clique os pontos e duplo clique encerra · na Elétrica/Hidráulica a linha vira eletroduto/tubulação" => {
            "Punkte anklicken und per Doppelklick abschließen · In der Elektro- oder Sanitärplanung entstehen daraus Leitungen bzw. Rohre"
        }
        "Clique para posicionar · portas e janelas encaixam na parede mais próxima · Esc cancela" => {
            "Zum Platzieren klicken · Türen und Fenster rasten in der nächsten Wand ein · Esc bricht ab"
        }
        "Clique seleciona · Ctrl+clique soma · arraste move · alças editam · duplo clique modifica" => {
            "Klick: auswählen · Strg+Klick: zur Auswahl hinzufügen · Ziehen: verschieben · Anfasspunkte: anpassen · Doppelklick: bearbeiten"
        }
        "Colar" => "Einfügen",
        "Comece desenhando paredes (W), importe uma planta como imagem de fundo, ou peça para a IA via MCP." => {
            "Wände zeichnen (W), einen Grundriss als Hintergrundbild importieren oder die Planung per MCP mit einer KI beginnen."
        }
        "Comparar" => "Vergleichen",
        "Comparar as versões" => "Versionen vergleichen",
        "Comparar versões" => "Versionen vergleichen",
        "Comprimento" => "Länge",
        "Comprimento exato da parede sendo desenhada" => {
            "Genaue Länge der gerade gezeichneten Wand"
        }
        "Contorno" => "Umriss",
        "Contínuo" => "Durchgezogen",
        "Copiar" => "Kopieren",
        "Copiar · Recortar · Colar · Duplicar" => {
            "Kopieren · Ausschneiden · Einfügen · Duplizieren"
        }
        "Cor" => "Farbe",
        "Cotar paredes selecionadas" => "Ausgewählte Wände bemaßen",
        "Cotas automáticas (engenharia)" => "Automatische Bemaßung (technisch)",
        "Criar cotas" => "Bemaßung erstellen",
        "Criar cômodos" => "Räume erstellen",
        "Criar foto" => "Foto erstellen",
        "Criar foto…" => "Foto erstellen …",
        "Criar vídeo" => "Video erstellen",
        "Criar vídeo…" => "Video erstellen …",
        "Pontos do caminho" => "Pfadpunkte",
        "Adicionar ponto atual" => "Aktuellen Punkt hinzufügen",
        "Ative o visitante na vista 3D" => "In der 3D-Ansicht in den Rundgang wechseln",
        "Órbita aérea" => "Kamerafahrt aus der Vogelperspektive",
        "Limpar" => "Löschen",
        "Quadros por segundo" => "Bilder pro Sekunde",
        "Velocidade" => "Geschwindigkeit",
        "Gerar vídeo…" => "Video rendern …",
        "Vídeo salvo em" => "Video gespeichert in",
        "quadros" => "Bilder",
        "Sol pela bússola e hora" => "Sonnenstand nach Ausrichtung und Uhrzeit",
        "Vista 3D" => "3D-Ansicht",
        "Mostrar em 3D" => "In 3D anzeigen",
        "Inclinação" => "Neigung",
        "Deitado" => "Liegend",
        "Em pé" => "Stehend",
        "Editor no navegador" => "Browser-Editor",
        "Disponível no aplicativo para desktop." => "Verfügbar in der Desktop-Anwendung.",
        "No navegador, exporte em .glb." => "Im Browser als .glb exportieren.",
        "Nenhum plugin instalado" => "Keine Plugins installiert",
        "Pastas" => "Ordner",
        "Rodando" => "Läuft",
        "concluído" => "abgeschlossen",
        "Acompanha as paredes" => "Folgt den Wänden",
        "O contorno é detectado pelas paredes e divisores e se ajusta quando eles mudam" => {
            "Die Kontur wird aus Wänden und Raumteilern ermittelt und bei Änderungen automatisch angepasst."
        }
        "Ambientes" => "Räume",
        "Divisor de ambiente" => "Raumteiler",
        "Separa ambientes sem parede (ex.: sala e jantar integrados)" => {
            "Trennt Räume ohne Wand, etwa bei einem offenen Wohn- und Essbereich."
        }
        "Escala vertical" => "Vertikale Skalierung",
        "Diferente" => "Unterschiedlich",
        "Criar paredes" => "Wände erstellen",
        "Curva suave" => "Glatte Kurve",
        "Cômodos" => "Räume",
        "Da sua biblioteca de modelos. Clique para pôr no centro da planta" => {
            "Aus Ihrer Modellbibliothek. Klicken Sie, um es in die Mitte des Grundrisses zu platzieren"
        }
        "Descartar" => "Verwerfen",
        "Descrição" => "Beschreibung",
        "Desenhar linhas" => "Linien zeichnen",
        "Desfazer" => "Rückgängig",
        "Desfazer (Ctrl+Z)" => "Rückgängig (Strg+Z)",
        "Desfazer · Refazer (inclusive o que a IA fez)" => {
            "Rückgängig · Wiederholen (auch Änderungen der KI)"
        }
        "Desliga o ímã (ângulos de 15°, pontos e grade)" => {
            "Deaktiviert das Einrasten an 15°-Winkeln, Punkten und Raster"
        }
        "Detalhes: marca, modelo e link" => "Details: Marke, Modell und Link",
        "Digitar número + Enter" => "Zahl eingeben und mit der Eingabetaste bestätigen",
        "Direita" => "Rechts",
        "Disco" => "Scheibe",
        "Distância real" => "Tatsächlicher Abstand",
        "Dividir parede ao meio" => "Wand in der Mitte teilen",
        "Diâmetro" => "Durchmesser",
        "Dobradiça à direita" => "Anschlag rechts",
        "Duplicar" => "Duplizieren",
        "Duplicar versão atual" => "Aktuelle Version duplizieren",
        "Duplicar versão · Próxima versão (guias)" => {
            "Version duplizieren · Zur nächsten Version wechseln"
        }
        "Duplo clique" => "Doppelklick",
        "Editar" => "Bearbeiten",
        "Editar andar…" => "Geschoss bearbeiten …",
        "Elevação" => "Höhenlage",
        "Elevação do piso" => "Fußbodenhöhe",
        "Elétrica" => "Elektroplanung",
        "Elétrica (tomadas, luz, cabos)" => "Elektroplanung (Steckdosen, Leuchten, Leitungen)",
        "Encerra paredes · fecha/detecta cômodo · cota parede · modifica" => {
            "Wandzug beenden · Raum schließen oder erkennen · Wand bemaßen · Objekt bearbeiten"
        }
        "Enquadrar (Ctrl+0)" => "Ansicht einpassen (Strg+0)",
        "Enquadrar 3D" => "3D-Ansicht einpassen",
        "Enquadrar planta" => "Grundriss einpassen",
        "Escala" => "Maßstab",
        "Escala calibrada. Arraste para posicionar a imagem ou troque de ferramenta." => {
            "Skalierung kalibriert. Ziehen, um das Bild zu platzieren, oder Werkzeug wechseln."
        }
        "Espelhado" => "Gespiegelt",
        "Espessura" => "Dicke",
        "Espessura da laje" => "Deckendicke",
        "Esquerda" => "Links",
        "Esta versão e o histórico dela serão removidos do projeto." => {
            "Diese Version und deren Verlauf werden aus dem Projekt entfernt."
        }
        "Estilo" => "Stil",
        "Excluir" => "Löschen",
        "Excluir andar" => "Geschoss löschen",
        "Exibir" => "Anzeigen",
        "Exportar 3D" => "3D-Modell exportieren",
        "Exportar planta" => "Grundriss exportieren",
        "Fechada" => "Geschlossen",
        "Fechar" => "Schließen",
        "Fechar versão" => "Version schließen",
        "Fim" => "Ende",
        "Fim (x, y)" => "Ende (x, y)",
        "Grupo" => "Gruppe",
        "Hidráulica" => "Sanitärplanung",
        "Hora do dia" => "Uhrzeit",
        "Imagem" => "Bild",
        "Imagem de fundo" => "Hintergrundbild",
        "Imagem importada. Clique em dois pontos de medida conhecida para calibrar; arraste para posicionar." => {
            "Bild importiert. Zwei Punkte mit bekanntem Abstand anklicken, um den Maßstab festzulegen. Zum Positionieren das Bild ziehen."
        }
        "Imagem…" => "Bild…",
        "Imagens" => "Bilder",
        "Importar imagem de fundo" => "Hintergrundbild importieren",
        "Importar modelo 3D…" => "3D-Modell importieren …",
        "Importar…" => "Importieren …",
        "Informações" => "Informationen",
        "Início" => "Start",
        "Início (x, y)" => "Start (x, y)",
        "Itálico" => "Kursiv",
        "Lado direito" => "Rechte Seite",
        "Lado esquerdo" => "Linke Seite",
        "Largura" => "Breite",
        "Legenda de símbolos (elétrica e hidráulica)" => {
            "Symbollegende für Elektro- und Sanitärplanung"
        }
        "Licença" => "Lizenz",
        "Linhas (tubulação / eletroduto)" => "Linien (Rohre / Leerrohre)",
        "Link" => "Link",
        "MCP desligado" => "MCP aus",
        "Manter proporções" => "Proportionen beibehalten",
        "Marca" => "Marke",
        "Me paga um café" => "Auf Buy Me a Coffee unterstützen",
        "Medida" => "Messen",
        "Minha biblioteca" => "Meine Bibliothek",
        "Modelo" => "Modell",
        "Modelo da biblioteca posicionado. Arraste para o lugar." => {
            "Modell aus der Bibliothek eingefügt. Zum Positionieren an die gewünschte Stelle ziehen."
        }
        "Modelo importado. Ajuste medidas com Enter ou pelas alças." => {
            "Modell importiert. Die Maße lassen sich per Eingabetaste oder über die Anfasspunkte anpassen."
        }
        "Modelos 3D" => "3D-Modelle",
        "Modificar andar" => "Geschoss bearbeiten",
        "Modificar cota" => "Bemaßung bearbeiten",
        "Modificar cômodo" => "Raum bearbeiten",
        "Modificar linha" => "Linie bearbeiten",
        "Modificar parede" => "Wand bearbeiten",
        "Modificar texto" => "Text bearbeiten",
        "Modificar · Excluir · Cancelar" => "Bearbeiten · Löschen · Abbrechen",
        "Modificar…" => "Bearbeiten…",
        "Mostrar bússola" => "Kompass anzeigen",
        "Move a seleção 1 cm (10 cm)" => "Auswahl um 1 cm (10 cm) verschieben",
        "Mover vista" => "Ansicht verschieben",
        "Máxima" => "Beste",
        "Móveis" => "Möbel",
        "Móveis na planta" => "Möbel im Grundriss",
        "Nada encontrado." => "Nichts gefunden.",
        "Negrito" => "Fett",
        "Nenhum espaço fechado por paredes aqui" => {
            "Hier gibt es keinen von Wänden umschlossenen Raum."
        }
        "Nenhum ponto" => "Keine Punkte",
        "Nenhum ponto de vista salvo" => "Keine gespeicherten Kamerapositionen",
        "Nome" => "Name",
        "Nome do projeto" => "Projektname",
        "Norte" => "Nord",
        "Nova versão da planta" => "Neue Grundrissversion",
        "Nova versão em branco" => "Neue leere Version",
        "Novo" => "Neu",
        "Novo (Ctrl+N)" => "Neu (Strg+N)",
        "Níveis na mesma elevação funcionam como layouts alternativos" => {
            "Geschosse auf derselben Höhe dienen als alternative Grundrisse."
        }
        "O andar e tudo o que está nele serão removidos (dá para desfazer)." => {
            "Das Geschoss und alle darauf liegenden Elemente werden entfernt. Der Vorgang lässt sich rückgängig machen."
        }
        "O projeto tem alterações que ainda não foram salvas." => {
            "Das Projekt hat ungespeicherte Änderungen."
        }
        "OBJ + MTL…" => "OBJ + MTL…",
        "OK" => "OK",
        "Opacidade" => "Deckkraft",
        "Ordem (mesma elevação)" => "Reihenfolge (gleiche Höhe)",
        "PDF (A3, ajustado à folha)…" => "PDF (A3, an Blattgröße angepasst) …",
        "PDF 1:100…" => "PDF 1:100…",
        "PDF 1:50…" => "PDF 1:50…",
        "PNG…" => "PNG…",
        "Paredes" => "Wände",
        "Inteiras" => "In voller Höhe",
        "Recortadas" => "Aufgeschnitten",
        "Baixas" => "Abgesenkt",
        "Todas as paredes em pé" => "Alle Wände in voller Höhe anzeigen",
        "As paredes da frente descem para mostrar os cômodos" => {
            "Vordere Wände absenken, damit die Räume sichtbar sind"
        }
        "Todas as paredes descem" => "Alle Wände absenken",
        "Personalizada" => "Benutzerdefiniert",
        "Peça" => "Fliese",
        "Pintura" => "Farbe",
        "Piso" => "Fußboden",
        "Planta" => "Grundriss",
        "Pontilhado" => "Gepunktet",
        "Posição (x, y)" => "Position (x, y)",
        "Potência da luz" => "Leuchtenleistung",
        "Preço" => "Preis",
        "Problemas" => "Auffälligkeiten",
        "Profundidade" => "Tiefe",
        "Projeto em edição: novos símbolos e linhas vão para ele" => {
            "Aktive Planung. Neue Symbole und Linien werden hier eingefügt."
        }
        "Projetos (3D New Era AI, Sweet Home 3D)" => "Projekte (3D New Era AI, Sweet Home 3D)",
        "Pé-direito" => "Lichte Raumhöhe",
        "Qualidade" => "Qualität",
        "Quantitativos" => "Mengenermittlung",
        "Rascunho" => "Entwurf",
        "Recentes" => "Zuletzt geöffnet",
        "Recortar" => "Ausschneiden",
        "Refazer" => "Wiederholen",
        "Refazer (Ctrl+Shift+Z)" => "Wiederholen (Strg+Umschalt+Z)",
        "Referências dos cômodos" => "Raumbezogene Vorgaben",
        "Remover imagem" => "Bild entfernen",
        "Renderizar" => "Rendern",
        "Renomear" => "Umbenennen",
        "Rotação" => "Drehung",
        "SVG em escala real…" => "SVG im Originalmaßstab …",
        "Sair" => "Beenden",
        "Salvar" => "Speichern",
        "Salvar (Ctrl+S)" => "Speichern (Strg+S)",
        "Salvar PNG…" => "Als PNG speichern …",
        "Salvar alterações?" => "Änderungen speichern?",
        "Salvar como…" => "Als … speichern",
        "Salvar ponto de vista" => "Kameraposition speichern",
        "Scroll · botão do meio · F" => "Mausrad · Mitteltaste · F",
        "Selecionar" => "Auswählen",
        "Selecionar tudo" => "Alles auswählen",
        "Selecionar · Mover vista · Paredes · Cômodos · Cotas · Texto" => {
            "Auswählen · Ansicht verschieben · Wände · Räume · Bemaßung · Text"
        }
        "Sem acabamento" => "Ohne Oberfläche",
        "Sem nome" => "Unbenannt",
        "Sem seta" => "Ohne Pfeil",
        "Seta aberta" => "Offener Pfeil",
        "Seta cheia" => "Gefüllter Pfeil",
        "Setas (+Shift)" => "Pfeiltasten (+Umschalttaste)",
        "Shift (segurado)" => "Umschalttaste gedrückt halten",
        "Sweet Home 3D" => "Sweet Home 3D",
        "Símbolos arquitetônicos" => "Architektonische Symbole",
        "Tamanho" => "Größe",
        "Teto" => "Decke",
        "Texto" => "Text",
        "Tipo" => "Typ",
        "Tracejado" => "Gestrichelt",
        "Traço" => "Strich",
        "Traço e dois pontos" => "Strich–Punkt–Punkt",
        "Traço e ponto" => "Strich–Punkt",
        "Térreo" => "Erdgeschoss",
        "Unidade" => "Einheit",
        "Unir paredes selecionadas" => "Ausgewählte Wände verbinden",
        "Usa o ponto de vista atual da vista 3D (aérea ou visitante)." => {
            "Übernimmt die aktuelle Kameraposition der 3D-Ansicht aus der Vogelperspektive oder dem Rundgang."
        }
        "Ver" => "Ansicht",
        "Versão" => "Version",
        "Visitante" => "Rundgang",
        "Visitante — arraste: olhar · W/A/S/D ou setas: andar · Scroll: avançar · Esc: visão aérea" => {
            "Rundgang · Ziehen: umsehen · W/A/S/D oder Pfeiltasten: gehen · Mausrad: vorwärts · Esc: Vogelperspektive"
        }
        "Vista de cima de todos" => "Alle Modelle von oben zeigen",
        "Visualização 3D requer o backend wgpu." => "Die 3D-Ansicht benötigt den wgpu-Backend.",
        "Visão aérea" => "Vogelperspektive",
        "Frente" => "Vorderseite",
        "Trás" => "Rückseite",
        "Topo" => "Oben",
        "Visível" => "Sichtbar",
        "Visível no 3D" => "In 3D sichtbar",
        "Zoom · mover vista · enquadrar" => "Zoomen · Ansicht verschieben · Ansicht einpassen",
        "glTF binário (.glb)…" => "Binäres glTF (.glb)…",
        "peças" => "Elemente",
        "Área" => "Fläche",
        "Área na planta" => "Fläche im Grundriss",
        "Ângulo" => "Winkel",
        "Cotas" => "Bemaßung",
        "Textos" => "Texte",
        "Linhas" => "Linien",
        "paredes" => "Wände",
        "cômodos" => "Räume",
        "móveis" => "Möbelstücke",
        "grupo de" => "Gruppe von",
        "Sala de estar" => "Wohnzimmer",
        "Sala de jantar" => "Esszimmer",
        "Cozinha" => "Küche",
        "Quarto" => "Schlafzimmer",
        "Banheiro" => "Bad",
        "Lavanderia" => "Hauswirtschaft",
        "Escritório" => "Büro",
        "Portas e janelas" => "Türen und Fenster",
        "Estrutura" => "Bauteile",
        "Decoração" => "Dekoration",
        "Área externa" => "Außenbereich",
        "atual" => "aktuell",
        "Importado de {} — salve como projeto para manter tudo num arquivo" => {
            "Aus {} importiert. Als Projekt speichern, um alles in einer Datei zusammenzuhalten."
        }
        "Aberto: {}" => "Geöffnet: {}",
        " · {} aviso(s)" => " · Warnungen: {}",
        "⚠ Não foi possível abrir {}: {}" => "⚠ {} konnte nicht geöffnet werden: {}",
        "Pontos de vista ({})" => "Kamerapositionen ({})",
        "Ponto de vista {}" => "Kameraposition {}",
        "Salvo em {}" => "In {} gespeichert",
        "⚠ Não foi possível salvar: {}" => "⚠ Speichern fehlgeschlagen: {}",
        "Modelo 3D exportado para {}" => "3D-Modell nach {} exportiert",
        "⚠ Falha ao exportar: {}" => "⚠ Export fehlgeschlagen: {}",
        "Planta exportada para {}" => "Grundriss nach {} exportiert",
        "⚠ Imagem inválida: {}" => "⚠ Ungültiges Bild: {}",
        "Modificar {} paredes" => "{} Wände bearbeiten",
        "Modificar {}" => "{} bearbeiten",
        "{} peças" => "{} Elemente",
        "Os pontos marcados estão a {} na escala atual." => {
            "Beim aktuellen Maßstab beträgt der Abstand zwischen den markierten Punkten {}."
        }
        "Fechar “{}”?" => "„{}“ schließen?",
        "Excluir “{}”?" => "„{}“ löschen?",
        "Renderizando… {} s" => "Rendering läuft … {} s",
        "Pronta em {} s" => "Fertig nach {} s",
        "Foto salva em {}" => "Foto in {} gespeichert",
        "Mostrar {}" => "{} anzeigen",
        "Tema" => "Farbschema",
        "Do sistema" => "Systemeinstellung",
        "Noite" => "Dunkel",
        "Dia" => "Hell",
        "luz" => "Licht",
        "Arraste ou dois dedos: girar · Shift: mover · Pinça ou scroll: zoom · F: enquadrar" => {
            "Ziehen oder zwei Finger: drehen · Umschalttaste: verschieben · Finger zusammenziehen oder Mausrad: zoomen · F: Ansicht einpassen"
        }
        "Aparência" => "Darstellung",
        "Sofá 3 lugares" => "3-Sitzer-Sofa",
        "Sofá 2 lugares" => "2-Sitzer-Sofa",
        "Sofá em L (chaise)" => "Ecksofa mit Chaiselongue",
        "Poltrona" => "Sessel",
        "Poltrona Win" => "Sessel Win",
        "Cadeira Ares" => "Stuhl Ares",
        "Mesa de centro" => "Couchtisch",
        "Mesa lateral" => "Beistelltisch",
        "Rack de TV" => "TV-Möbel",
        "Televisão 55\"" => "55-Zoll-Fernseher",
        "Estante" => "Regal",
        "Tapete" => "Teppich",
        "Radiador de painel 60×10×50" => "Flachheizkörper 60×10×50",
        "Radiador de painel 100×10×50" => "Flachheizkörper 100×10×50",
        "Radiador de painel 120×10×50" => "Flachheizkörper 120×10×50",
        "Convector elétrico 100×10×45" => "Elektrokonvektor 100×10×45",
        "Luminária de piso" => "Stehleuchte",
        "Abajur de mesa" => "Tischleuchte",
        "Spot embutido LED 7 W" => "LED-Einbaustrahler, 7 W",
        "Pendente" => "Pendelleuchte",
        "Painel LED 60×60 36 W" => "LED-Panel 60×60 36 W",
        "Fita LED (perfil)" => "LED-Streifen im Profil",
        "Mesa de jantar 4 lugares" => "Esstisch für 4 Personen",
        "Mesa de jantar 6 lugares" => "Esstisch für 6 Personen",
        "Mesa com 4 cadeiras" => "Tisch mit 4 Stühlen",
        "Mesa com 6 cadeiras" => "Tisch mit 6 Stühlen",
        "Mesa redonda" => "Runder Tisch",
        "Cadeira" => "Stuhl",
        "Aparador" => "Sideboard",
        "Geladeira" => "Kühlschrank",
        "Fogão 4 bocas" => "Herd mit 4 Kochstellen",
        "Bancada com pia" => "Arbeitsplatte mit Spüle",
        "Armário de cozinha (baixo)" => "Küchenunterschrank",
        "Armário aéreo" => "Küchenoberschrank",
        "Coifa" => "Dunstabzugshaube",
        "Lava-louças" => "Spülmaschine",
        "Micro-ondas" => "Mikrowelle",
        "Cooktop 4 bocas (embutir)" => "Einbaukochfeld mit 4 Kochstellen",
        "Cuba de inox (embutir)" => "Edelstahl-Einbauspüle",
        "Forno de embutir" => "Einbaubackofen",
        "Ilha de cozinha" => "Kücheninsel",
        "Banqueta alta" => "Barhocker",
        "Cama de casal" => "Doppelbett",
        "Cama queen" => "Queen-Bett",
        "Cama king" => "King-Bett",
        "Cama de solteiro" => "Einzelbett",
        "Criado-mudo" => "Nachttisch",
        "Guarda-roupa" => "Kleiderschrank",
        "Cômoda" => "Kommode",
        "Berço" => "Gitterbett",
        "Vaso sanitário" => "Toilette",
        "Gabinete com lavatório" => "Waschtischunterschrank mit Becken",
        "Box de chuveiro" => "Duschwanne",
        "Box de vidro" => "Duschabtrennung aus Glas",
        "Banheira" => "Badewanne",
        "Máquina de lavar" => "Waschmaschine",
        "Secadora" => "Wäschetrockner",
        "Tanque" => "Ausgussbecken",
        "Escrivaninha" => "Schreibtisch",
        "Cadeira de escritório" => "Bürostuhl",
        "Porta" => "Tür",
        "Porta dupla" => "Doppeltür",
        "Porta de correr" => "Schiebetür",
        "Portão de garagem" => "Garagentor",
        "Vão livre" => "Freie Wandöffnung",
        "Janela" => "Fenster",
        "Janela basculante" => "Kippfenster",
        "Porta-janela" => "Fenstertür",
        "Escada reta" => "Geradläufige Treppe",
        "Pilar" => "Stütze",
        "Coluna redonda" => "Rundstütze",
        "Sapata" => "Einzelfundament",
        "Viga / caibro" => "Balken / Sparren",
        "Painel" => "Platte",
        "Telha ondulada" => "Wellplatte",
        "Gradil de ferro (guarda-corpo de barras verticais)" => {
            "Eisengeländer mit senkrechten Stäben"
        }
        "Guarda-corpo de vidro laminado com corrimão metálico" => {
            "Geländer aus Verbundglas mit Metallhandlauf"
        }
        "Fechamento de vidro da sacada (envidraçamento retrátil)" => {
            "Verschiebbare Balkonverglasung"
        }
        "Caixa" => "Kiste",
        "Planta em vaso" => "Topfpflanze",
        "Floreira" => "Pflanzkasten",
        "Muro / cerca" => "Wand / Zaun",
        "Piscina" => "Schwimmbecken",
        "Piscina oval" => "Ovales Schwimmbecken",
        "Espreguiçadeira" => "Sonnenliege",
        "Churrasqueira" => "Grill",
        "Banco" => "Bank",
        "Árvore" => "Baum",
        "Carro" => "Auto",
        "Tomada baixa (30 cm)" => "Steckdose auf 30 cm Höhe",
        "Tomada média (1,10 m)" => "Steckdose auf 1,10 m Höhe",
        "Tomada alta (2,20 m)" => "Steckdose auf 2,20 m Höhe",
        "Interruptor simples" => "Einfachschalter",
        "Interruptor duplo" => "Doppelschalter",
        "Interruptor paralelo (three-way)" => "Wechselschalter",
        "Ponto de luz no teto" => "Deckenleuchtenauslass",
        "Arandela (ponto de luz na parede)" => "Wandleuchtenauslass",
        "Quadro de distribuição" => "Elektroverteiler",
        "Ponto de ar-condicionado" => "Anschluss für Klimagerät",
        "Ponto para chuveiro elétrico" => "Elektroanschluss für die Dusche",
        "Tomada de dados / TV" => "Daten-/TV-Steckdose",
        "Ponto de rede RJ45 (Cat 6)" => "RJ45-Netzwerkanschluss (Cat 6)",
        "Ponto de TV (coaxial)" => "Koaxialer TV-Anschluss",
        "Ponto de Wi-Fi no teto (access point)" => "WLAN-Zugangspunkt an der Decke",
        "Quadro de telecomunicações (rack / DG)" => {
            "Telekommunikationsverteiler (Rack / Hauptverteiler)"
        }
        "Campainha" => "Türklingel",
        "Torre de tomadas retrátil de embutir (furo 60 mm, 3 tomadas, plugue)" => {
            "Versenkbare Steckdosensäule zum Einbau (Bohrung 60 mm, 3 Steckdosen, Netzstecker)"
        }
        "Torre de tomadas automática com indução (furo 85 mm, ligada à instalação)" => {
            "Automatische Steckdosensäule mit induktiver Ladefläche (Bohrung 85 mm, Festanschluss)"
        }
        "Torre de 4 tomadas + USB e indução (furo 100 mm)" => {
            "Steckdosensäule mit 4 Steckdosen, USB und induktiver Ladefläche (Bohrung 100 mm)"
        }
        "Caixa de tomadas de mesa (4 tomadas, RJ45, USB, HDMI; recorte 120×335 mm)" => {
            "Tischanschlussbox (4 Steckdosen, RJ45, USB, HDMI, Ausschnitt 120×335 mm)"
        }
        "Tomada de embutir em móvel (furo 35 mm)" => "Möbeleinbausteckdose (Bohrung 35 mm)",
        "Relé de automação (atrás do interruptor ou da luminária)" => {
            "Schaltrelais zur Automatisierung (hinter dem Schalter oder der Leuchte)"
        }
        "Interruptor inteligente" => "Intelligenter Schalter",
        "Dimmer" => "Dimmer",
        "Sensor de presença de teto" => "Präsenzmelder zur Deckenmontage",
        "Fechadura eletrônica" => "Elektronisches Türschloss",
        "Ponto de água fria" => "Kaltwasseranschluss",
        "Ponto de água quente" => "Warmwasseranschluss",
        "Ponto de esgoto" => "Abwasseranschluss",
        "Caixa sifonada 150×150×50 (ralo)" => "Bodenablauf mit Geruchsverschluss 150×150×50",
        "Caixa sifonada 100×150×50 (ralo)" => "Bodenablauf mit Geruchsverschluss 100×150×50",
        "Caixa sifonada 150×185×75 (ralo)" => "Bodenablauf mit Geruchsverschluss 150×185×75",
        "Ralo sifonado 100 mm (fecho hídrico curto)" => {
            "Bodenablauf 100 mm mit niedrigem Geruchsverschluss"
        }
        "Ralo seco 100 mm" => "Bodenablauf 100 mm ohne Geruchsverschluss",
        "Ralo linear 70 cm (sem sifão)" => "Ablaufrinne 70 cm ohne Geruchsverschluss",
        "Ralo linear sifonado 70 cm" => "Ablaufrinne 70 cm mit Geruchsverschluss",
        "Ralo de águas pluviais (varanda descoberta, terraço)" => {
            "Regenwasserablauf (ungedeckter Balkon, Terrasse)"
        }
        "Registro" => "Absperrventil",
        "Caixa de gordura" => "Fettabscheider",
        "Grelha de ventilação permanente (gás)" => "Lüftungsgitter für Dauerbelüftung (Gas)",
        "Tubo de ventilação" => "Entlüftungsrohr",
        "Caixa de inspeção" => "Revisionsschacht",
        "Hidrômetro" => "Wasserzähler",
        "Ponto de gás" => "Gasanschluss",
        "Drywall 73 mm" => "Trockenbauwand 73 mm",
        "Drywall 95 mm" => "Trockenbauwand 95 mm",
        "Drywall 115 mm" => "Trockenbauwand 115 mm",
        "Drywall 120 mm chapa dupla" => "Trockenbauwand 120 mm, doppelt beplankt",
        "Tijolo cerâmico 9 cm" => "Mauerziegel 9 cm",
        "Tijolo cerâmico 14 cm" => "Mauerziegel 14 cm",
        "Tijolo cerâmico 19 cm" => "Mauerziegel 19 cm",
        "Bloco de concreto 14 cm" => "Betonstein 14 cm",
        "Bloco de concreto 19 cm" => "Betonstein 19 cm",
        "Parede de concreto 10 cm" => "Betonwand 10 cm",
        "Concreto armado 15 cm" => "Stahlbeton 15 cm",
        "Steel frame 14 cm" => "Stahlrahmenbau 14 cm",
        "Wood frame 10 cm" => "Holzrahmenbau 10 cm",
        "Divisória de vidro" => "Glastrennwand",
        "Montante 48 mm + 1 chapa de 12,5 mm em cada face" => {
            "Ständerprofil 48 mm, je Seite eine Platte mit 12,5 mm"
        }
        "Montante 70 mm + 1 chapa de 12,5 mm em cada face" => {
            "Ständerprofil 70 mm, je Seite eine Platte mit 12,5 mm"
        }
        "Montante 90 mm + 1 chapa de 12,5 mm em cada face" => {
            "Ständerprofil 90 mm, je Seite eine Platte mit 12,5 mm"
        }
        "Montante 70 mm + 2 chapas de 12,5 mm em cada face (acústica)" => {
            "Ständerprofil 70 mm, je Seite zwei Platten mit 12,5 mm (Schallschutz)"
        }
        "Bloco de 9 cm + reboco de 2,5 cm em cada face" => "Mauerstein 9 cm, je Seite 2,5 cm Putz",
        "Bloco de 14 cm + reboco de 2,5 cm em cada face" => {
            "Mauerstein 14 cm, je Seite 2,5 cm Putz"
        }
        "Bloco de 19 cm + reboco de 2,5 cm em cada face" => {
            "Mauerstein 19 cm, je Seite 2,5 cm Putz"
        }
        "Bloco estrutural de 14 cm + revestimento de 1,5 cm em cada face" => {
            "Tragender Mauerstein 14 cm, je Seite 1,5 cm Bekleidung"
        }
        "Bloco estrutural de 19 cm + revestimento de 1,5 cm em cada face" => {
            "Tragender Mauerstein 19 cm, je Seite 1,5 cm Bekleidung"
        }
        "Concreto moldado in loco, sistema parede de concreto" => "Ortbetonwand",
        "Parede estrutural de concreto armado" => "Tragende Stahlbetonwand",
        "Perfil 90 mm + OSB e placa cimentícia/gesso" => {
            "Profil 90 mm mit OSB- und Zement- oder Gipsbauplatte"
        }
        "Estrutura de madeira com fechamento em chapas" => "Holzrahmen mit Plattenbeplankung",
        "Vidro temperado de 10 mm" => "Einscheibensicherheitsglas 10 mm",
        "Madeira (réguas)" => "Holzdielen",
        "Taco espinha de peixe" => "Fischgrätparkett",
        "Porcelanato / cerâmica" => "Feinsteinzeug / Keramikfliesen",
        "Azulejo metrô" => "Metrofliesen",
        "Tijolo aparente" => "Sichtmauerwerk",
        "Pedra" => "Stein",
        "Cimento queimado" => "Geglätteter Zementboden",
        "Mármore" => "Marmor",
        "Carpete" => "Teppich",
        "Grama" => "Gras",
        "Água (piscina)" => "Wasser (Pool)",
        "Deck de madeira" => "Holzterrasse",
        "Projeto recuperado desta sessão." => "Projekt aus dieser Sitzung wiederhergestellt.",
        "No terminal:" => "Im Terminal:",
        "Os instaladores já registram sozinhos se o Claude Code estiver instalado." => {
            "Die Installationsprogramme registrieren es automatisch, wenn Claude Code installiert ist."
        }
        "Configurações → Desenvolvedor → Editar configuração" => {
            "Einstellungen → Entwickler → Konfiguration bearbeiten"
        }
        "Precisa do Node.js instalado para a ponte mcp-remote. Reinicie o Claude Desktop." => {
            "Die MCP-Verbindung über mcp-remote benötigt Node.js. Anschließend Claude Desktop neu starten."
        }
        "Os instaladores já registram sozinhos se o Codex estiver instalado." => {
            "Die Installationsprogramme registrieren es automatisch, wenn Codex installiert ist."
        }
        "Depois abra o Gemini CLI e peça o projeto." => {
            "Gemini CLI öffnen und dort nach dem Projekt fragen."
        }
        "Use no modo agente do Copilot." => "Verwenden Sie es im Agentenmodus von Copilot.",
        "Arquivo ~/.cursor/mcp.json" => "Datei ~/.cursor/mcp.json",
        "Reinicie o Cursor depois de salvar." => "Cursor nach dem Speichern neu starten.",
        "Outro app" => "Andere Anwendung",
        "Nas configurações de MCP do app (Cline, Roo Code, Cherry Studio, LM Studio…)" => {
            "In den MCP-Einstellungen der Anwendung (Cline, Roo Code, Cherry Studio, LM Studio…)"
        }
        "O que importa é o app falar MCP: o modelo pode ser qualquer um." => {
            "Die Anwendung muss MCP unterstützen. Das verwendete KI-Modell ist frei wählbar."
        }
        "agora" => "gerade eben",
        "há {} s" => "vor {} s",
        "há {} min" => "vor {} min",
        "há {} h" => "vor {} Std.",
        "{} · {} chamadas" => "{} · {} Aufrufe",
        "MCP · esperando sua IA" => "MCP · wartet auf eine KI-Verbindung",
        "IA: só no aplicativo" => "KI-Funktionen in der Desktop-App",
        "Conectar sua IA — clique para ver como" => "Anleitung zum Verbinden einer KI öffnen",
        "IA" => "KI",
        "{} conectado · {}" => "{} verbunden · {}",
        "Nenhuma IA conectada ainda" => "Noch keine KI verbunden",
        "Conectar sua IA…" => "KI verbinden …",
        "Copiar o endereço do MCP" => "MCP-Adresse kopieren",
        "Endereço copiado" => "Adresse kopiert",
        "Conectar sua IA" => "KI verbinden",
        "O editor abre uma porta MCP: a sua IA desenha aqui dentro, em centímetros, e você vê acontecer." => {
            "Der Editor stellt eine MCP-Verbindung bereit. Eine verbundene KI zeichnet in Zentimetern, und die Änderungen erscheinen live im Editor."
        }
        "Agora" => "Aktueller Status",
        "Servidor MCP ligado" => "MCP-Server aktiv",
        "No navegador o editor roda sozinho: o MCP vive no aplicativo do computador." => {
            "Im Browser läuft der Editor eigenständig. Der lokale MCP-Server ist Teil der Desktop-App."
        }
        "O servidor está desligado (--no-server): reabra o aplicativo sem essa opção." => {
            "Der MCP-Server wurde mit --no-server deaktiviert. Die App ohne diese Option erneut öffnen."
        }
        "Nenhuma IA conectada ainda — siga os três passos abaixo." => {
            "Noch keine KI verbunden. Die drei Schritte unten richten die Verbindung ein."
        }
        "1. Escolha o aplicativo de IA que você usa" => "1. KI-Anwendung auswählen",
        "2. Cole isto onde ele pede" => "2. Den Text in das vorgesehene Feld einfügen",
        "3. Peça alguma coisa" => "3. Eine Aufgabe eingeben",
        "Quantas paredes tem este projeto? Depois coloque uma janela de 120 cm na sala." => {
            "Wie viele Wände hat dieses Projekt? Setze danach ein 120 cm breites Fenster ins Wohnzimmer."
        }
        "Copiado" => "Kopiert",
        "Últimas chamadas" => "Letzte Aufrufe",
        "Nada ainda. Assim que sua IA usar uma ferramenta, ela aparece aqui." => {
            "Noch keine Werkzeugaufrufe. Sobald die KI ein Werkzeug nutzt, erscheint der Aufruf hier."
        }
        "Baixar o aplicativo" => "App herunterladen",
        "{} conectado · {} chamadas · {}" => "{} verbunden · {} Aufrufe · {}",
        "{} conectou — sua IA já pode desenhar aqui" => {
            "{} ist verbunden und kann jetzt im Editor zeichnen"
        }
        "Registrar agora" => "Jetzt registrieren",
        "Registrando…" => "Registrierung läuft …",
        "Roda esse mesmo comando aqui, sem abrir o terminal." => {
            "Führt denselben Befehl direkt aus, ohne ein Terminal zu öffnen."
        }
        "Registrado no {} — agora é só pedir" => {
            "Bei {} registriert. Jetzt kann die KI eine Aufgabe übernehmen."
        }
        "Não deu para registrar: {}" => "Registrierung fehlgeschlagen: {}",
        "Ligue e esta aba ganha um endereço que a sua IA alcança. O projeto não sai daqui: o servidor no meio só passa recados." => {
            "Nach dem Einschalten erhält der Tab eine Adresse, über die eine KI ihn erreichen kann. Das Projekt bleibt im Tab. Der Vermittlungsserver leitet nur Nachrichten weiter."
        }
        "Ligar o MCP nesta aba" => "MCP in diesem Tab aktivieren",
        "Ligando…" => "Verbindung wird eingerichtet …",
        "Esta aba está no ar para a sua IA" => "Dieser Tab ist für die KI erreichbar",
        "Quem tiver o endereço abaixo pode editar este projeto: trate como senha." => {
            "Mit dieser Adresse lässt sich das Projekt bearbeiten. Die Adresse wie ein Passwort behandeln."
        }
        "Não deu para ligar: {}" => "Aktivierung fehlgeschlagen: {}",
        "Tentar de novo" => "Nochmal versuchen",
        "{} só no aplicativo do computador: aqui ele renderiza na CPU e travaria a aba" => {
            "{} ist in der Desktop-App verfügbar. Im Browser würde das Rendern über die CPU den Tab blockieren."
        }
        "MCP desligado · ligar" => "MCP deaktiviert · einschalten",
        "Desligar" => "Ausschalten",
        "o relay recusou esta aba — ligue de novo" => {
            "Der Vermittlungsserver hat diesen Tab abgelehnt. Die Verbindung erneut aktivieren."
        }
        "Conectado como {}" => "Als {} angemeldet",
        "Com este endereço, o Claude, o ChatGPT e outras IAs chegam nesta aba entrando com a sua conta." => {
            "Mit dieser Adresse können Claude, ChatGPT und andere KI-Anwendungen nach der Anmeldung mit dem eigenen Konto auf diesen Tab zugreifen."
        }
        "Entrar para usar no Claude e no ChatGPT" => "Anmelden, um Claude und ChatGPT zu verbinden",
        "Com uma conta, a sua IA chega nesta aba por um endereço fixo, sem colar nada." => {
            "Mit einem Konto ist dieser Tab für die KI über eine feste Adresse erreichbar. Ein manuelles Einfügen entfällt."
        }
        "Fecha o endereço. A sua IA perde o acesso na hora; o projeto continua aqui." => {
            "Deaktiviert die Adresse. Der Zugriff endet sofort und das Projekt bleibt in diesem Tab."
        }
        "Conectar sua IA (no navegador, liga e desliga)" => {
            "KI verbinden (Browserzugriff ein- oder ausschalten)"
        }
        "3D" => "3D",
        "Catálogo · Planta · 3D (telas estreitas)" => {
            "Katalog · Grundriss · 3D (schmale Bildschirme)"
        }
        "o servidor que leva a sua IA até esta aba não respondeu — no aplicativo do computador o MCP não precisa dele" => {
            "Der Vermittlungsserver antwortet nicht. In der Desktop-App läuft MCP ohne diesen Vermittlungsserver."
        }
        "Disciplina, camadas e andares" => "Fachplanung, Ebenen und Geschosse",
        "IA · esperando" => "KI · wartet auf Verbindung",
        "Conectar IA" => "KI verbinden",
        "{} conectado · {} chamadas" => "{} verbunden · {} Aufrufe",
        "Esperando a sua IA chegar" => "Warte auf eine KI-Verbindung",
        "Ainda não dá para alcançar esta janela" => "Dieses Fenster ist noch nicht erreichbar",
        "Escolha o aplicativo de IA que você usa" => "KI-Anwendung auswählen",
        "Cole isto onde ele pede" => "Den Text in das vorgesehene Feld einfügen",
        "Peça alguma coisa" => "Eine Aufgabe eingeben",
        "o endereço guardado não vale mais — ligue de novo" => {
            "Die gespeicherte Adresse ist ungültig. Die Verbindung erneut aktivieren."
        }
        "No ar, esperando a sua IA" => "Bereit für eine KI-Verbindung",
        "Quem tiver esse endereço edita este projeto: trate como senha." => {
            "Mit dieser Adresse lässt sich das Projekt bearbeiten. Die Adresse wie ein Passwort behandeln."
        }
        "cooktop" => "Kochfeld",
        "Controle de câmera" => "Kamerasteuerung",
        "Pavimento e altura do olho" => "Geschoss & Augenhöhe",
        "Térreo único (0.00 m)" => "Eingeschossig (0.00 m)",
        "Posição, elevação e lente" => "Position, Höhe & Objektiv",
        "Pontos de vista salvos" => "Gespeicherte Blickpunkte",
        _ => return None,
    })
}
