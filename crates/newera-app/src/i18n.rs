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

    #[test]
    fn translates_only_when_english() {
        assert_eq!(tr("Salvar"), "Salvar");
        set_english(true);
        assert_eq!(tr("Salvar"), "Save");
        assert_eq!(tr("Algo novo"), "Algo novo");
        set_english(false);
    }
}
