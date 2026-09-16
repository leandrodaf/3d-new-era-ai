//! Interface language. Texts are written in Portuguese in the code and
//! looked up here when English is chosen; unknown texts stay as written.

use std::sync::atomic::{AtomicBool, Ordering};

static ENGLISH: AtomicBool = AtomicBool::new(false);

/// Switches the interface language.
pub(crate) fn set_english(english: bool) {
    ENGLISH.store(english, Ordering::Relaxed);
}

pub(crate) fn is_english() -> bool {
    ENGLISH.load(Ordering::Relaxed)
}

/// The text in the current language.
/// Translates a line that only exists at run time: what a background job says
/// it is doing comes from the work itself, not from a literal in this crate.
pub(crate) fn dynamic(pt: &str) -> String {
    if !is_english() {
        return pt.to_owned();
    }
    english(pt).map_or_else(|| pt.to_owned(), str::to_owned)
}

pub(crate) fn tr(pt: &'static str) -> &'static str {
    if !is_english() {
        return pt;
    }
    english(pt).unwrap_or(pt)
}

#[allow(clippy::too_many_lines, clippy::match_same_arms)]
fn english(pt: &str) -> Option<&'static str> {
    Some(match pt {
        " · arco" => " · arc",
        "A tarefa termina sozinha em segundo plano; o resultado é descartado." => {
            "The task finishes on its own in the background; its result is thrown away."
        }
        "Abrindo imagens e modelos" => "Opening images and models",
        "Abrindo projeto" => "Opening project",
        "Desenhando a planta" => "Drawing the plan",
        "Exportando" => "Exporting",
        "Extraindo imagens e modelos" => "Extracting images and models",
        "Forçar e encerrar a espera" => "Force it and stop waiting",
        "Gravando o projeto" => "Writing the project",
        "Guardando imagens e modelos" => "Storing images and models",
        "Importando modelo" => "Importing model",
        "Lendo o arquivo" => "Reading the file",
        "Lendo o projeto" => "Reading the project",
        "Montando o modelo 3D" => "Building the 3D model",
        "Parando…" => "Stopping…",
        "Salvando projeto" => "Saving project",
        "Trabalhando…" => "Working…",
        "⚠ Espera encerrada — a tarefa termina em segundo plano." => {
            "⚠ Stopped waiting — the task finishes in the background."
        }
        "Ajustar ao telhado" => "Fit to the roof",
        "Código de obras" => "Building code",
        "Embutir peça no móvel selecionado" => "Embed the piece in the selected joinery",
        "Embutido:" => "Embedded:",
        "Armários na parede" => "Cabinets on the wall",
        "Armários na parede…" => "Cabinets on the wall…",
        "Balcões" => "Base cabinets",
        "Aéreos" => "Wall cabinets",
        "Entre a norma e a lei do município, prevalece o mais restritivo." => {
            "Between the standard and the city's law, the more restrictive one wins."
        }
        "Fontes desta revisão:" => "Sources of this review:",
        "Não informado" => "Not set",
        "Referências brasileiras onde existe norma; áreas e janelas variam com o código de obras do município." => {
            "Brazilian standards where one exists; areas and windows vary with the city's building code."
        }
        "Torres / guarda-roupa" => "Tall / wardrobe",
        "Pia centrada em" => "Sink centered at",
        "Cooktop centrado em" => "Cooktop centered at",
        "descreve" => "describes",
        "do início da parede" => "from the wall start",
        "Bancada de pedra por cima" => "Stone countertop on top",
        "Frentes" => "Fronts",
        "Gaveteiros" => "Drawer units",
        "automático" => "automatic",
        "Mede o trecho livre entre cantos, portas, janelas e eletrodomésticos e divide em módulos sem sobras; substitui os armários que já estão nessa parede." => {
            "Measures the free stretch between corners, doors, windows and appliances and splits it into modules with no leftovers; replaces the cabinets already on that wall."
        }
        "Pré-visualizar" => "Preview",
        "Criar armários" => "Build cabinets",
        "Criado (Ctrl+Z desfaz):" => "Built (Ctrl+Z undoes it):",
        "Prévia:" => "Preview:",
        "doutrina" => "doctrine",
        "mede" => "measures",
        "obriga" => "obliges",
        "portas" => "doors",
        "gaveteiro" => "drawers",
        "porta-temperos" => "pull-out",
        "canto cego" => "blind corner",
        "cabideiro" => "hanging",
        "referencia" => "references",
        "sobre a geladeira" => "over the fridge",
        "pia" => "sink",
        "tamponamento" => "filler",
        "bancada" => "countertop",
        "Ergonomia" => "Ergonomics",
        "Ergonomia…" => "Ergonomics…",
        "Moradores" => "Occupants",
        "Crianças" => "Children",
        "Idosos" => "Elderly",
        "Altura de quem cozinha" => "Cook's height",
        "Alguém usa cadeira de rodas (NBR 9050)" => "Someone uses a wheelchair (NBR 9050)",
        "nota de habitabilidade" => "habitability score",
        "lugares para dormir" => "beds",
        "banheiros" => "bathrooms",
        "à mesa" => "at the table",
        "na sala" => "in the living room",
        "de guarda-roupa" => "of wardrobe",
        "Nada a apontar para esses moradores." => "Nothing to point out for these occupants.",
        "Selecionar na planta" => "Select on the plan",
        "Aplicar correção" => "Apply fix",
        "Correção aplicada (Ctrl+Z desfaz)." => "Fix applied (Ctrl+Z undoes it).",
        "Referências: NBR 9050, NBR 15575-1 e IBGE; áreas e janelas variam com o código de obras do município." => {
            "References: NBR 9050, NBR 15575-1 and IBGE; areas and windows vary with the city's building code."
        }
        " · luz" => " · light",
        "(cópia)" => "(copy)",
        "Abrir" => "Open",
        "Abrir (Ctrl+O)" => "Open (Ctrl+O)",
        "Abrir link" => "Open link",
        "Abrir…" => "Open…",
        "Adicionar texto" => "Add text",
        "Adicionar um andar acima do mais alto" => "Add a storey above the highest one",
        "Afastamento" => "Offset",
        "Afastar (Ctrl -)" => "Zoom out (Ctrl -)",
        "Ajuda" => "Help",
        "Quadro de cargas e NBR 5410" => "Load schedule and NBR 5410",
        "Nenhum circuito: atribua os pontos a circuitos (MCP electrical assign)." => {
            "No circuits: assign the points to circuits (MCP electrical assign)."
        }
        "Circuito" => "Circuit",
        "Pontos" => "Points",
        "VA" => "VA",
        "V" => "V",
        "A" => "A",
        "Fio mm²" => "Wire mm²",
        "Disjuntor" => "Breaker",
        "DR" => "RCD",
        "Total instalado:" => "Installed total:",
        "Nada a apontar." => "Nothing to point out.",
        "Camadas" => "Layers",
        "Iluminação" => "Lighting",
        "Eletrodomésticos" => "Appliances",
        "Marcenaria" => "Joinery",
        "Mostrar ou esconder na planta; no 3D tudo continua aparecendo" => {
            "Show or hide on the plan; the 3D keeps showing everything"
        }
        "Enviar relatórios de erro" => "Send error reports",
        "Falhas e notas de uso vão para os desenvolvedores, sem o seu projeto, sem IP e sem nome da máquina." => {
            "Crashes and usage notes go to the developers, without your project, your IP or your machine's name."
        }
        "Ajustes…" => "Settings…",
        "Alinhamento" => "Alignment",
        "Altura" => "Height",
        "Andar" => "Storey",
        "Aproximar (Ctrl +)" => "Zoom in (Ctrl +)",
        "Arco" => "Arc",
        "Arquitetura" => "Architecture",
        "Arquivo" => "File",
        "Arraste para mover a vista" => "Drag to move the view",
        "Arraste: girar · Shift/botão do meio: mover · Scroll: zoom · F: enquadrar" => {
            "Drag: orbit · Shift/middle button: pan · Scroll: zoom · F: frame"
        }
        "Atalhos e ferramentas" => "Shortcuts and tools",
        "Automático (símbolos e vista de cima dos modelos)" => {
            "Automatic (symbols, top views for models)"
        }
        "Autor" => "Author",
        "Boa" => "Good",
        "Buscar: cama, janela, sofá…" => "Search: bed, window, sofa…",
        "Calibrar e posicionar" => "Calibrate and place",
        "Calibrar escala da imagem" => "Calibrate image scale",
        "Cancelar" => "Cancel",
        "Casa e bússola" => "Home and compass",
        "Casa e bússola…" => "Home and compass…",
        "Catálogo" => "Catalog",
        "Catálogo de origem" => "Source catalog",
        "Centro" => "Center",
        "Centro (x, y)" => "Center (x, y)",
        "Clique dois pontos de medida conhecida · arraste para posicionar a imagem" => {
            "Click two points of a known distance · drag to place the image"
        }
        "Clique e depois clique na planta para posicionar" => {
            "Click, then click on the plan to place"
        }
        "Clique encadeia paredes · digite o comprimento + Enter · Shift desliga o ímã · duplo clique encerra" => {
            "Clicks chain walls · type a length + Enter · Shift disables magnetism · double-click ends"
        }
        "Clique início e fim, mova para afastar e clique · duplo clique numa parede cota a parede" => {
            "Click start and end, move to offset and click · double-click a wall to dimension it"
        }
        "Clique onde o texto deve ficar" => "Click where the text goes",
        "Clique os cantos e duplo clique fecha · duplo clique dentro de paredes detecta o cômodo" => {
            "Click the corners and double-click to close · double-click inside walls detects the room"
        }
        "Clique os pontos e duplo clique encerra · na Elétrica/Hidráulica a linha vira eletroduto/tubulação" => {
            "Click the points and double-click to end · in Electrical/Plumbing lines become conduits/pipes"
        }
        "Clique para posicionar · portas e janelas encaixam na parede mais próxima · Esc cancela" => {
            "Click to place · doors and windows snap into the nearest wall · Esc cancels"
        }
        "Clique seleciona · Ctrl+clique soma · arraste move · alças editam · duplo clique modifica" => {
            "Click selects · Ctrl+click adds · drag moves · handles edit · double-click modifies"
        }
        "Colar" => "Paste",
        "Comece desenhando paredes (W), importe uma planta como imagem de fundo, ou peça para a IA via MCP." => {
            "Start by drawing walls (W), import a plan as background image, or ask the AI through MCP."
        }
        "Comparar" => "Compare",
        "Comparar as versões" => "Compare versions",
        "Comparar versões" => "Compare versions",
        "Comprimento" => "Length",
        "Comprimento exato da parede sendo desenhada" => "Exact length of the wall being drawn",
        "Contorno" => "Outline",
        "Contínuo" => "Solid",
        "Copiar" => "Copy",
        "Copiar · Recortar · Colar · Duplicar" => "Copy · Cut · Paste · Duplicate",
        "Cor" => "Color",
        "Cotar paredes selecionadas" => "Dimension selected walls",
        "Cotas automáticas (engenharia)" => "Automatic dimensions (engineering)",
        "Criar cotas" => "Create dimensions",
        "Criar cômodos" => "Create rooms",
        "Criar foto" => "Create photo",
        "Criar foto…" => "Create photo…",
        "Criar vídeo" => "Create video",
        "Criar vídeo…" => "Create video…",
        "Pontos do caminho" => "Path points",
        "Adicionar ponto atual" => "Add current point",
        "Ative o visitante na vista 3D" => "Turn on the visitor in the 3D view",
        "Órbita aérea" => "Aerial orbit",
        "Limpar" => "Clear",
        "Quadros por segundo" => "Frames per second",
        "Velocidade" => "Speed",
        "Gerar vídeo…" => "Render video…",
        "Vídeo salvo em" => "Video saved to",
        "quadros" => "frames",
        "Sol pela bússola e hora" => "Sun from compass and time",
        "Vista 3D" => "3D view",
        "Mostrar em 3D" => "Show in 3D",
        "Inclinação" => "Tilt",
        "Deitado" => "Lying",
        "Em pé" => "Standing",
        "Editor no navegador" => "Browser editor",
        "Disponível no aplicativo para desktop." => "Available in the desktop app.",
        "No navegador, exporte em .glb." => "In the browser, export as .glb.",
        "Nenhum plugin instalado" => "No plugins installed",
        "Pastas" => "Folders",
        "Rodando" => "Running",
        "concluído" => "done",
        "Acompanha as paredes" => "Follows the walls",
        "O contorno é detectado pelas paredes e divisores e se ajusta quando eles mudam" => {
            "The outline is detected from walls and dividers and adjusts when they change"
        }
        "Ambientes" => "Rooms",
        "Divisor de ambiente" => "Room divider",
        "Separa ambientes sem parede (ex.: sala e jantar integrados)" => {
            "Separates rooms without a wall (e.g. open living and dining)"
        }
        "Escala vertical" => "Vertical scale",
        "Diferente" => "Different",
        "Criar paredes" => "Create walls",
        "Curva suave" => "Smooth curve",
        "Cômodos" => "Rooms",
        "Descartar" => "Discard",
        "Descrição" => "Description",
        "Desenhar linhas" => "Draw lines",
        "Desfazer" => "Undo",
        "Desfazer (Ctrl+Z)" => "Undo (Ctrl+Z)",
        "Desfazer · Refazer (inclusive o que a IA fez)" => {
            "Undo · Redo (including what the AI did)"
        }
        "Desliga o ímã (ângulos de 15°, pontos e grade)" => {
            "Disables magnetism (15° angles, points and grid)"
        }
        "Detalhes: marca, modelo e link" => "Details: brand, model and link",
        "Digitar número + Enter" => "Type number + Enter",
        "Direita" => "Right",
        "Disco" => "Disc",
        "Distância real" => "Real distance",
        "Dividir parede ao meio" => "Split wall in half",
        "Diâmetro" => "Diameter",
        "Dobradiça à direita" => "Hinge on the right",
        "Duplicar" => "Duplicate",
        "Duplicar versão atual" => "Duplicate current version",
        "Duplicar versão · Próxima versão (guias)" => "Duplicate version · Next version (tabs)",
        "Duplo clique" => "Double-click",
        "Editar" => "Edit",
        "Editar andar…" => "Edit storey…",
        "Elevação" => "Elevation",
        "Elevação do piso" => "Floor elevation",
        "Elétrica" => "Electrical",
        "Encerra paredes · fecha/detecta cômodo · cota parede · modifica" => {
            "Ends walls · closes/detects room · dimensions wall · modifies"
        }
        "Enquadrar (Ctrl+0)" => "Frame (Ctrl+0)",
        "Enquadrar 3D" => "Frame 3D",
        "Enquadrar planta" => "Frame plan",
        "Escala" => "Scale",
        "Escala calibrada. Arraste para posicionar a imagem ou troque de ferramenta." => {
            "Scale calibrated. Drag to place the image or switch tools."
        }
        "Espelhado" => "Mirrored",
        "Espessura" => "Thickness",
        "Espessura da laje" => "Slab thickness",
        "Esquerda" => "Left",
        "Esta versão e o histórico dela serão removidos do projeto." => {
            "This version and its history will be removed from the project."
        }
        "Estilo" => "Style",
        "Excluir" => "Delete",
        "Excluir andar" => "Delete storey",
        "Exibir" => "Show",
        "Exportar 3D" => "Export 3D",
        "Exportar planta" => "Export plan",
        "Fechada" => "Closed",
        "Fechar" => "Close",
        "Fechar versão" => "Close version",
        "Fim" => "End",
        "Fim (x, y)" => "End (x, y)",
        "Grupo" => "Group",
        "Hidráulica" => "Plumbing",
        "Hora do dia" => "Time of day",
        "Imagem" => "Image",
        "Imagem de fundo" => "Background image",
        "Imagem importada. Clique em dois pontos de medida conhecida para calibrar; arraste para posicionar." => {
            "Image imported. Click two points of a known distance to calibrate; drag to place."
        }
        "Imagem…" => "Image…",
        "Imagens" => "Images",
        "Importar imagem de fundo" => "Import background image",
        "Importar modelo 3D…" => "Import 3D model…",
        "Importar…" => "Import…",
        "Informações" => "Information",
        "Início" => "Start",
        "Início (x, y)" => "Start (x, y)",
        "Itálico" => "Italic",
        "Lado direito" => "Right side",
        "Lado esquerdo" => "Left side",
        "Largura" => "Width",
        "Legenda de símbolos (elétrica e hidráulica)" => {
            "Symbol legend (electrical and plumbing)"
        }
        "Licença" => "License",
        "Linhas (tubulação / eletroduto)" => "Lines (pipes / conduits)",
        "Link" => "Link",
        "MCP desligado" => "MCP off",
        "Manter proporções" => "Keep proportions",
        "Marca" => "Brand",
        "Medida" => "Measure",
        "Modelo" => "Model",
        "Modelo importado. Ajuste medidas com Enter ou pelas alças." => {
            "Model imported. Adjust sizes with Enter or the handles."
        }
        "Modelos 3D" => "3D models",
        "Modificar andar" => "Modify storey",
        "Modificar cota" => "Modify dimension",
        "Modificar cômodo" => "Modify room",
        "Modificar linha" => "Modify line",
        "Modificar parede" => "Modify wall",
        "Modificar texto" => "Modify text",
        "Modificar · Excluir · Cancelar" => "Modify · Delete · Cancel",
        "Modificar…" => "Modify…",
        "Mostrar bússola" => "Show compass",
        "Move a seleção 1 cm (10 cm)" => "Moves the selection 1 cm (10 cm)",
        "Mover vista" => "Pan view",
        "Máxima" => "Best",
        "Móveis" => "Furniture",
        "Móveis na planta" => "Furniture on the plan",
        "Nada encontrado." => "Nothing found.",
        "Negrito" => "Bold",
        "Nenhum espaço fechado por paredes aqui" => "No space enclosed by walls here",
        "Nenhum ponto" => "No points",
        "Nenhum ponto de vista salvo" => "No saved points of view",
        "Nome" => "Name",
        "Nome do projeto" => "Project name",
        "Norte" => "North",
        "Nova versão da planta" => "New plan version",
        "Nova versão em branco" => "New blank version",
        "Novo" => "New",
        "Novo (Ctrl+N)" => "New (Ctrl+N)",
        "Níveis na mesma elevação funcionam como layouts alternativos" => {
            "Levels at the same elevation work as alternative layouts"
        }
        "O andar e tudo o que está nele serão removidos (dá para desfazer)." => {
            "The storey and everything on it will be removed (undoable)."
        }
        "O projeto tem alterações que ainda não foram salvas." => {
            "The project has unsaved changes."
        }
        "OBJ + MTL…" => "OBJ + MTL…",
        "OK" => "OK",
        "Opacidade" => "Opacity",
        "Ordem (mesma elevação)" => "Order (same elevation)",
        "PDF (A3, ajustado à folha)…" => "PDF (A3, fit to sheet)…",
        "PDF 1:100…" => "PDF 1:100…",
        "PDF 1:50…" => "PDF 1:50…",
        "PNG…" => "PNG…",
        "Paredes" => "Walls",
        "Personalizada" => "Custom",
        "Peça" => "Tile",
        "Pintura" => "Paint",
        "Piso" => "Floor",
        "Planta" => "Plan",
        "Pontilhado" => "Dotted",
        "Posição (x, y)" => "Position (x, y)",
        "Potência da luz" => "Light power",
        "Preço" => "Price",
        "Problemas" => "Issues",
        "Profundidade" => "Depth",
        "Projeto em edição: novos símbolos e linhas vão para ele" => {
            "Project being edited: new symbols and lines go there"
        }
        "Projetos (3D New Era AI, Sweet Home 3D)" => "Projects (3D New Era AI, Sweet Home 3D)",
        "Pé-direito" => "Ceiling height",
        "Qualidade" => "Quality",
        "Quantitativos" => "Quantities",
        "Rascunho" => "Draft",
        "Recentes" => "Recent",
        "Recortar" => "Cut",
        "Refazer" => "Redo",
        "Refazer (Ctrl+Shift+Z)" => "Redo (Ctrl+Shift+Z)",
        "Referências dos cômodos" => "Room references",
        "Remover imagem" => "Remove image",
        "Renderizar" => "Render",
        "Renomear" => "Rename",
        "Rotação" => "Rotation",
        "SVG em escala real…" => "SVG at true scale…",
        "Sair" => "Quit",
        "Salvar" => "Save",
        "Salvar (Ctrl+S)" => "Save (Ctrl+S)",
        "Salvar PNG…" => "Save PNG…",
        "Salvar alterações?" => "Save changes?",
        "Salvar como…" => "Save as…",
        "Salvar ponto de vista" => "Save point of view",
        "Scroll · botão do meio · F" => "Scroll · middle button · F",
        "Selecionar" => "Select",
        "Selecionar tudo" => "Select all",
        "Selecionar · Mover vista · Paredes · Cômodos · Cotas · Texto" => {
            "Select · Pan · Walls · Rooms · Dimensions · Text"
        }
        "Sem acabamento" => "No finish",
        "Sem nome" => "Unnamed",
        "Sem seta" => "No arrow",
        "Seta aberta" => "Open arrow",
        "Seta cheia" => "Filled arrow",
        "Setas (+Shift)" => "Arrows (+Shift)",
        "Shift (segurado)" => "Shift (held)",
        "Sweet Home 3D" => "Sweet Home 3D",
        "Símbolos arquitetônicos" => "Architectural symbols",
        "Tamanho" => "Size",
        "Teto" => "Ceiling",
        "Texto" => "Text",
        "Tipo" => "Type",
        "Tracejado" => "Dashed",
        "Traço" => "Dash",
        "Traço e dois pontos" => "Dash dot dot",
        "Traço e ponto" => "Dash dot",
        "Térreo" => "Ground floor",
        "Unidade" => "Unit",
        "Unir paredes selecionadas" => "Join selected walls",
        "Usa o ponto de vista atual da vista 3D (aérea ou visitante)." => {
            "Uses the current 3D point of view (aerial or visitor)."
        }
        "Ver" => "View",
        "Versão" => "Version",
        "Visitante" => "Visitor",
        "Visitante — arraste: olhar · W/A/S/D ou setas: andar · Scroll: avançar · Esc: visão aérea" => {
            "Visitor — drag: look · W/A/S/D or arrows: walk · Scroll: forward · Esc: aerial view"
        }
        "Vista de cima de todos" => "Top views for all",
        "Visualização 3D requer o backend wgpu." => "The 3D view needs the wgpu backend.",
        "Visão aérea" => "Aerial view",
        "Visível" => "Visible",
        "Visível no 3D" => "Visible in 3D",
        "Zoom · mover vista · enquadrar" => "Zoom · pan · frame",
        "glTF binário (.glb)…" => "Binary glTF (.glb)…",
        "peças" => "pieces",
        "Área" => "Area",
        "Área na planta" => "Area on the plan",
        "Ângulo" => "Angle",
        "Cotas" => "Dimensions",
        "Textos" => "Texts",
        "Linhas" => "Lines",
        "paredes" => "walls",
        "cômodos" => "rooms",
        "móveis" => "pieces",
        "grupo de" => "group of",
        "Sala de estar" => "Living room",
        "Sala de jantar" => "Dining room",
        "Cozinha" => "Kitchen",
        "Quarto" => "Bedroom",
        "Banheiro" => "Bathroom",
        "Lavanderia" => "Laundry",
        "Escritório" => "Office",
        "Portas e janelas" => "Doors and windows",
        "Estrutura" => "Structure",
        "Decoração" => "Decor",
        "Área externa" => "Outdoor",
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every literal handed to `tr` has an English side, or the window
    /// silently falls back to Portuguese for whoever picked English — which
    /// is exactly how a new panel ships half translated.
    #[test]
    fn every_string_on_screen_has_an_english_side() {
        fn sources(dir: &std::path::Path, into: &mut String) {
            for entry in std::fs::read_dir(dir).expect("src is readable") {
                let path = entry.expect("entry").path();
                if path.is_dir() {
                    sources(&path, into);
                } else if path.extension().is_some_and(|e| e == "rs") {
                    into.push_str(&std::fs::read_to_string(&path).expect("source"));
                }
            }
        }
        let mut code = String::new();
        sources(std::path::Path::new("src"), &mut code);
        // A literal handed to tr, whatever the wrapping and the trailing comma.
        let mut missing: Vec<String> = Vec::new();
        let mut rest = code.as_str();
        while let Some(at) = rest.find("tr(") {
            rest = &rest[at + 3..];
            let Some(open) = rest.find('"') else { break };
            if rest[..open].contains(|c: char| !c.is_whitespace()) {
                continue;
            }
            let body = &rest[open + 1..];
            let Some(close) = body.find('"') else { break };
            let (text, after) = (&body[..close], body[close + 1..].trim_start());
            if after.starts_with(',') || after.starts_with(')') {
                // The test's own probe, and words spelled the same in English.
                if english(text).is_none() && !["Algo novo", "Plugins", "cooktop"].contains(&text) {
                    missing.push(text.to_owned());
                }
            }
        }
        assert!(missing.is_empty(), "no English for: {missing:#?}");
    }

    #[test]
    fn translates_only_when_english() {
        assert_eq!(tr("Salvar"), "Salvar");
        set_english(true);
        assert_eq!(tr("Salvar"), "Save");
        assert_eq!(tr("Algo novo"), "Algo novo");
        set_english(false);
    }
}
