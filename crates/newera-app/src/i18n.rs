//! Interface language. Texts are written in Portuguese in the code and
//! looked up here when another language is chosen; unknown texts stay as
//! written.

use std::sync::atomic::{AtomicU8, Ordering};

/// A language the window speaks.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub(crate) enum Lang {
    /// Portuguese (Brazil): the language the texts are written in, so it
    /// needs no table.
    #[default]
    Pt,
    En,
    Es,
    Fr,
    Uk,
}

impl Lang {
    /// Every language, in the order the menu lists them.
    pub(crate) const ALL: [Self; 5] = [Self::Pt, Self::En, Self::Es, Self::Fr, Self::Uk];

    /// How the language names itself — the one name whoever looks for it
    /// reads without knowing any of the others.
    pub(crate) const fn label(self) -> &'static str {
        match self {
            Self::Pt => "Português (Brasil)",
            Self::En => "English",
            Self::Es => "Español",
            Self::Fr => "Français",
            Self::Uk => "Українська",
        }
    }

    /// The two letters a locale name starts with for this language.
    pub(crate) const fn tag(self) -> &'static str {
        match self {
            Self::Pt => "pt",
            Self::En => "en",
            Self::Es => "es",
            Self::Fr => "fr",
            Self::Uk => "uk",
        }
    }

    /// Whether numbers are written with a comma for the decimals: everyone
    /// here but English.
    pub(crate) const fn decimal_comma(self) -> bool {
        !matches!(self, Self::En)
    }

    /// Which side of a table row the language is on; Portuguese is the key
    /// of the row, not a side of it.
    const fn column(self) -> Option<usize> {
        match self {
            Self::Pt => None,
            Self::En => Some(0),
            Self::Es => Some(1),
            Self::Fr => Some(2),
            Self::Uk => Some(3),
        }
    }

    const fn code(self) -> u8 {
        match self {
            Self::Pt => 0,
            Self::En => 1,
            Self::Es => 2,
            Self::Fr => 3,
            Self::Uk => 4,
        }
    }

    const fn from_code(code: u8) -> Self {
        match code {
            1 => Self::En,
            2 => Self::Es,
            3 => Self::Fr,
            4 => Self::Uk,
            _ => Self::Pt,
        }
    }

    /// The language a locale name asks for — `pt_BR.UTF-8`, `fr-CA`, `es` —
    /// or nothing when the window does not speak it.
    fn from_locale(locale: &str) -> Option<Self> {
        let tag = locale.get(..2)?.to_ascii_lowercase();
        Self::ALL.into_iter().find(|lang| lang.tag() == tag)
    }

    /// What the system asks for, when the window speaks it. `NEWERA_LANG`
    /// comes first, as it does in the installers.
    #[cfg(not(target_arch = "wasm32"))]
    pub(crate) fn from_system() -> Option<Self> {
        ["NEWERA_LANG", "LC_ALL", "LC_MESSAGES", "LANG"]
            .iter()
            .filter_map(|name| std::env::var(name).ok())
            .find_map(|locale| Self::from_locale(&locale))
    }

    /// In the browser there is no environment to read: the language is the
    /// one the browser itself is set to.
    #[cfg(target_arch = "wasm32")]
    pub(crate) fn from_system() -> Option<Self> {
        Self::from_locale(&web_sys::window()?.navigator().language()?)
    }
}

static LANG: AtomicU8 = AtomicU8::new(0);

/// Switches the interface language.
pub(crate) fn set(lang: Lang) {
    LANG.store(lang.code(), Ordering::Relaxed);
}

pub(crate) fn lang() -> Lang {
    Lang::from_code(LANG.load(Ordering::Relaxed))
}

/// The language a finding is said in. Findings carry Portuguese and English;
/// where the window speaks neither, English is the one more readers share.
pub(crate) fn findings_language() -> newera_core::vocabulary::Language {
    match lang() {
        Lang::Pt => newera_core::vocabulary::Language::Portuguese,
        Lang::En | Lang::Es | Lang::Fr | Lang::Uk => newera_core::vocabulary::Language::English,
    }
}

/// Whether numbers on screen take a comma for the decimals.
pub(crate) fn decimal_comma() -> bool {
    lang().decimal_comma()
}

/// The text in the current language.
/// Translates a line that only exists at run time: what a background job says
/// it is doing comes from the work itself, not from a literal in this crate.
pub(crate) fn dynamic(pt: &str) -> String {
    let Some(column) = lang().column() else {
        return pt.to_owned();
    };
    translations(pt).map_or_else(|| pt.to_owned(), |row| row[column].to_owned())
}

/// A name that lives in the project, as the window shows it. The defaults
/// the project starts with ("Nova casa", "Versão 1", "Versão 1 (cópia)") and
/// the catalog names pieces are placed under are Portuguese in the file; on
/// screen they follow the window's language. A name somebody typed is theirs
/// and stays as it is.
pub(crate) fn shown(name: &str) -> String {
    shown_in(name, lang())
}

/// [`shown`] in one named language. See [`say`].
fn shown_in(name: &str, lang: Lang) -> String {
    let Some(column) = lang.column() else {
        return name.to_owned();
    };
    if let Some(original) = name.strip_suffix(" (cópia)") {
        return format!("{}{}", shown_in(original, lang), say(" (cópia)", lang));
    }
    if let Some(n) = name.strip_prefix("Versão ")
        && !n.is_empty()
        && n.chars().all(|c| c.is_ascii_digit())
    {
        return fill_in("Versão {}", lang, &[&n]);
    }
    translations(name).map_or_else(|| name.to_owned(), |row| row[column].to_owned())
}

/// The text in the language the window is set to: `pt` is the Portuguese
/// the code is written in, and comes back as it is when there is no
/// translation for it.
pub(crate) fn tr(pt: &'static str) -> &'static str {
    say(pt, lang())
}

/// The text in every language the window speaks, Portuguese first: what a
/// name typed in any of them is matched against.
#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn every(pt: &'static str) -> impl Iterator<Item = &'static str> {
    Lang::ALL.into_iter().map(move |lang| say(pt, lang))
}

/// The text in one named language, whatever the window is set to. The window
/// itself always wants [`tr`]; this is for the tests, which run beside each
/// other and must not shout across.
fn say(pt: &'static str, lang: Lang) -> &'static str {
    let Some(column) = lang.column() else {
        return pt;
    };
    translations(pt).map_or(pt, |row| row[column])
}

/// A sentence with things in it: `{}` in the text marks where each one goes,
/// in every language, so a translation is free to put them elsewhere.
pub(crate) fn fill(pt: &'static str, parts: &[&dyn std::fmt::Display]) -> String {
    fill_in(pt, lang(), parts)
}

/// [`fill`] in one named language. See [`say`].
fn fill_in(pt: &'static str, lang: Lang, parts: &[&dyn std::fmt::Display]) -> String {
    let mut text = String::new();
    let mut rest = say(pt, lang);
    for part in parts {
        let Some((before, after)) = rest.split_once("{}") else {
            break;
        };
        text.push_str(before);
        text.push_str(&part.to_string());
        rest = after;
    }
    text.push_str(rest);
    text
}

/// A number and the thing it counts, in the language the window is set to:
/// "12 paredes", "12 walls". `pt` is the word in the plural, as `tr` takes
/// it.
pub(crate) fn count(n: u64, pt: &'static str) -> String {
    count_in(n, pt, lang())
}

/// [`count`] in one named language. See [`say`]. Ukrainian changes the word
/// with the number — 1 стіна, 2 стіни, 5 стін — so it picks one of three
/// forms; the others put the plural after any number, as they always have.
fn count_in(n: u64, pt: &'static str, lang: Lang) -> String {
    if lang == Lang::Uk
        && let Some([one, few, many]) = ukrainian_forms(pt)
    {
        let word = match (n % 10, n % 100) {
            (1, tens) if tens != 11 => one,
            (2..=4, tens) if !(12..=14).contains(&tens) => few,
            _ => many,
        };
        return format!("{n} {word}");
    }
    format!("{n} {}", say(pt, lang))
}

/// The three Ukrainian forms of a word that follows a number: after 1 (and
/// 21, 31…), after 2–4 (and 22–24…), and after the rest.
fn ukrainian_forms(pt: &str) -> Option<[&'static str; 3]> {
    Some(match pt {
        "paredes" => ["стіна", "стіни", "стін"],
        "cômodos" => ["кімната", "кімнати", "кімнат"],
        "móveis" => ["предмет меблів", "предмети меблів", "предметів меблів"],
        "peças" => ["елемент", "елементи", "елементів"],
        "quadros" => ["кадр", "кадри", "кадрів"],
        "lugares para dormir" => ["спальне місце", "спальні місця", "спальних місць"],
        "banheiros" => ["ванна кімната", "ванні кімнати", "ванних кімнат"],
        _ => return None,
    })
}

/// What the text says in English, Spanish, French and Ukrainian, in this
/// order.
#[allow(clippy::too_many_lines, clippy::match_same_arms)]
fn translations(pt: &str) -> Option<[&'static str; 4]> {
    Some(match pt {
        " · arco" => [" · arc", " · arco", " · arc", " · дуга"],
        " (cópia)" => [" (copy)", " (copia)", " (copie)", " (копія)"],
        "Nova casa" => ["New home", "Casa nueva", "Nouvelle maison", "Новий будинок"],
        "Versão {}" => ["Version {}", "Versión {}", "Version {}", "Версія {}"],
        "A tarefa termina sozinha em segundo plano; o resultado é descartado." => [
            "The task finishes on its own in the background; its result is thrown away.",
            "La tarea termina sola en segundo plano; su resultado se descarta.",
            "La tâche se termine seule en arrière-plan ; son résultat est ignoré.",
            "Завдання завершиться саме у фоновому режимі; його результат буде відкинуто.",
        ],
        "Abrindo imagens e modelos" => [
            "Opening images and models",
            "Abriendo imágenes y modelos",
            "Ouverture des images et des modèles",
            "Відкриття зображень і моделей",
        ],
        "Abrindo projeto" => [
            "Opening project",
            "Abriendo proyecto",
            "Ouverture du projet",
            "Відкриття проєкту",
        ],
        "Desenhando a planta" => [
            "Drawing the plan",
            "Dibujando el plano",
            "Tracé du plan",
            "Креслення плану",
        ],
        "Exportando" => ["Exporting", "Exportando", "Exportation", "Експорт"],
        "Extraindo imagens e modelos" => [
            "Extracting images and models",
            "Extrayendo imágenes y modelos",
            "Extraction des images et des modèles",
            "Видобування зображень і моделей",
        ],
        "Forçar e encerrar a espera" => [
            "Force it and stop waiting",
            "Forzar y dejar de esperar",
            "Forcer et arrêter d'attendre",
            "Примусово припинити очікування",
        ],
        "Gravando o projeto" => [
            "Writing the project",
            "Grabando el proyecto",
            "Écriture du projet",
            "Запис проєкту",
        ],
        "Guardando imagens e modelos" => [
            "Storing images and models",
            "Guardando imágenes y modelos",
            "Enregistrement des images et des modèles",
            "Збереження зображень і моделей",
        ],
        "Importando modelo" => [
            "Importing model",
            "Importando modelo",
            "Importation du modèle",
            "Імпорт моделі",
        ],
        "Lendo o arquivo" => [
            "Reading the file",
            "Leyendo el archivo",
            "Lecture du fichier",
            "Читання файлу",
        ],
        "Lendo o projeto" => [
            "Reading the project",
            "Leyendo el proyecto",
            "Lecture du projet",
            "Читання проєкту",
        ],
        "Montando o modelo 3D" => [
            "Building the 3D model",
            "Montando el modelo 3D",
            "Construction du modèle 3D",
            "Побудова 3D-моделі",
        ],
        "Parando…" => ["Stopping…", "Deteniendo…", "Arrêt…", "Зупинка…"],
        "Salvando projeto" => [
            "Saving project",
            "Guardando proyecto",
            "Enregistrement du projet",
            "Збереження проєкту",
        ],
        "Trabalhando…" => [
            "Working…",
            "Trabajando…",
            "Travail en cours…",
            "Виконується…",
        ],
        "⚠ Espera encerrada — a tarefa termina em segundo plano." => [
            "⚠ Stopped waiting — the task finishes in the background.",
            "⚠ Espera terminada — la tarea sigue en segundo plano.",
            "⚠ Attente interrompue — la tâche se termine en arrière-plan.",
            "⚠ Очікування припинено — завдання завершиться у фоновому режимі.",
        ],
        "Ajustar ao telhado" => [
            "Fit to the roof",
            "Ajustar al tejado",
            "Ajuster au toit",
            "Підігнати під дах",
        ],
        "Código de obras" => [
            "Building code",
            "Código de obras",
            "Code de la construction",
            "Будівельні норми",
        ],
        "Embutir peça no móvel selecionado" => [
            "Embed the piece in the selected joinery",
            "Empotrar la pieza en el mueble seleccionado",
            "Encastrer la pièce dans le meuble sélectionné",
            "Вбудувати елемент у виділені меблі",
        ],
        "Embutido:" => ["Embedded:", "Empotrado:", "Encastré :", "Вбудовано:"],
        "Armários na parede" => [
            "Cabinets on the wall",
            "Armarios en la pared",
            "Meubles sur le mur",
            "Шафи вздовж стіни",
        ],
        "Armários na parede…" => [
            "Cabinets on the wall…",
            "Armarios en la pared…",
            "Meubles sur le mur…",
            "Шафи вздовж стіни…",
        ],
        "Balcões" => [
            "Base cabinets",
            "Muebles bajos",
            "Meubles bas",
            "Нижні шафи",
        ],
        "Aéreos" => [
            "Wall cabinets",
            "Muebles altos",
            "Meubles hauts",
            "Верхні шафи",
        ],
        "Entre a norma e a lei do município, prevalece o mais restritivo." => [
            "Between the standard and the city's law, the more restrictive one wins.",
            "Entre la norma y la ley del municipio, prevalece la más restrictiva.",
            "Entre la norme et la loi de la commune, la plus restrictive l'emporte.",
            "Якщо стандарт і місцеві правила розходяться, діє суворіша вимога.",
        ],
        "Fontes desta revisão:" => [
            "Sources of this review:",
            "Fuentes de esta revisión:",
            "Sources de cette révision :",
            "Джерела цієї перевірки:",
        ],
        "Não informado" => ["Not set", "Sin definir", "Non défini", "Не вказано"],
        "Referências brasileiras onde existe norma; áreas e janelas variam com o código de obras do município." => {
            [
                "Brazilian standards where one exists; areas and windows vary with the city's building code.",
                "Referencias brasileñas donde existe norma; áreas y ventanas varían según el código de obras del municipio.",
                "Normes brésiliennes lorsqu'elles existent ; surfaces et fenêtres varient selon le code de la construction de la commune.",
                "Бразильські норми, де вони є; площі та вікна залежать від місцевих будівельних норм.",
            ]
        }
        "Torres / guarda-roupa" => [
            "Tall / wardrobe",
            "Columnas / armario ropero",
            "Colonnes / dressing",
            "Пенали / шафа для одягу",
        ],
        "Pia centrada em" => [
            "Sink centered at",
            "Fregadero centrado en",
            "Évier centré à",
            "Центр мийки на відстані",
        ],
        "Cooktop centrado em" => [
            "Cooktop centered at",
            "Placa de cocción centrada en",
            "Table de cuisson centrée à",
            "Центр варильної поверхні на відстані",
        ],
        "descreve" => ["describes", "describe", "décrit", "описує"],
        "do início da parede" => [
            "from the wall start",
            "desde el inicio de la pared",
            "depuis le début du mur",
            "від початку стіни",
        ],
        "Bancada de pedra por cima" => [
            "Stone countertop on top",
            "Encimera de piedra encima",
            "Plan de travail en pierre au-dessus",
            "Кам'яна стільниця зверху",
        ],
        "Frentes" => ["Fronts", "Frentes", "Façades", "Фасади"],
        "Gaveteiros" => [
            "Drawer units",
            "Cajoneras",
            "Blocs-tiroirs",
            "Шухлядні блоки",
        ],
        "automático" => ["automatic", "automático", "automatique", "автоматично"],
        "Mede o trecho livre entre cantos, portas, janelas e eletrodomésticos e divide em módulos sem sobras; substitui os armários que já estão nessa parede." => {
            [
                "Measures the free stretch between corners, doors, windows and appliances and splits it into modules with no leftovers; replaces the cabinets already on that wall.",
                "Mide el tramo libre entre esquinas, puertas, ventanas y electrodomésticos y lo divide en módulos sin sobras; sustituye los armarios que ya están en esa pared.",
                "Mesure la portion libre entre angles, portes, fenêtres et appareils, puis la divise en modules sans chute ; remplace les meubles déjà posés sur ce mur.",
                "Вимірює вільну ділянку між кутами, дверима, вікнами й побутовою технікою та ділить її на модулі без залишків; замінює шафи, що вже стоять на цій стіні.",
            ]
        }
        "Pré-visualizar" => ["Preview", "Vista previa", "Aperçu", "Попередній перегляд"],
        "Criar armários" => [
            "Build cabinets",
            "Crear armarios",
            "Créer les meubles",
            "Створити шафи",
        ],
        "Criado (Ctrl+Z desfaz):" => [
            "Built (Ctrl+Z undoes it):",
            "Creado (Ctrl+Z deshace):",
            "Créé (Ctrl+Z annule) :",
            "Створено (Ctrl+Z скасовує):",
        ],
        "Prévia:" => [
            "Preview:",
            "Vista previa:",
            "Aperçu :",
            "Попередній перегляд:",
        ],
        "doutrina" => ["doctrine", "doctrina", "doctrine", "практика"],
        "mede" => ["measures", "mide", "mesure", "вимірює"],
        "obriga" => ["obliges", "obliga", "impose", "зобов'язує"],
        "portas" => ["doors", "puertas", "portes", "дверцята"],
        "gaveteiro" => ["drawers", "cajonera", "tiroirs", "шухляди"],
        "porta-temperos" => ["pull-out", "especiero", "coulissant", "висувна полиця"],
        "canto cego" => [
            "blind corner",
            "esquina ciega",
            "angle borgne",
            "сліпий кут",
        ],
        "cabideiro" => ["hanging", "barra de colgar", "penderie", "штанга для одягу"],
        "referencia" => ["references", "referencia", "référence", "посилається"],
        "sobre a geladeira" => [
            "over the fridge",
            "sobre el frigorífico",
            "au-dessus du réfrigérateur",
            "над холодильником",
        ],
        "pia" => ["sink", "fregadero", "évier", "мийка"],
        "tamponamento" => ["filler", "panel de relleno", "fileur", "доборна планка"],
        "bancada" => ["countertop", "encimera", "plan de travail", "стільниця"],
        "Ergonomia" => ["Ergonomics", "Ergonomía", "Ergonomie", "Ергономіка"],
        "Ergonomia…" => ["Ergonomics…", "Ergonomía…", "Ergonomie…", "Ергономіка…"],
        "Moradores" => ["Occupants", "Habitantes", "Occupants", "Мешканці"],
        "Crianças" => ["Children", "Niños", "Enfants", "Діти"],
        "Idosos" => ["Elderly", "Mayores", "Personnes âgées", "Літні люди"],
        "Altura de quem cozinha" => [
            "Cook's height",
            "Altura de quien cocina",
            "Taille de la personne qui cuisine",
            "Зріст того, хто готує",
        ],
        "Alguém usa cadeira de rodas (NBR 9050)" => [
            "Someone uses a wheelchair (NBR 9050)",
            "Alguien usa silla de ruedas (NBR 9050)",
            "Une personne en fauteuil roulant (NBR 9050)",
            "Хтось користується кріслом колісним (NBR 9050)",
        ],
        "Incluir elétrica na nota" => [
            "Include electrical in score",
            "Incluir electricidad en la nota",
            "Inclure l’électricité dans la note",
            "Враховувати електрику в оцінці",
        ],
        "Incluir hidráulica na nota" => [
            "Include plumbing in score",
            "Incluir fontanería en la nota",
            "Inclure la plomberie dans la note",
            "Враховувати сантехніку в оцінці",
        ],
        "Pendências fora do escopo continuam visíveis. A nota não certifica o projeto." => [
            "Findings outside the scope remain visible. The score does not certify the project.",
            "Los hallazgos fuera del alcance siguen visibles. La nota no certifica el proyecto.",
            "Les constats hors périmètre restent visibles. La note ne certifie pas le projet.",
            "Зауваження поза межами оцінки залишаються видимими. Оцінка не є сертифікацією проєкту.",
        ],
        "Fora do escopo da nota" => [
            "Outside score scope",
            "Fuera del alcance de la nota",
            "Hors périmètre de la note",
            "Поза межами оцінки",
        ],
        "nota de habitabilidade" => [
            "habitability score",
            "nota de habitabilidad",
            "note d'habitabilité",
            "оцінка придатності для життя",
        ],
        "lugares para dormir" => ["beds", "plazas para dormir", "couchages", "спальних місць"],
        "banheiros" => ["bathrooms", "baños", "salles de bain", "ванних кімнат"],
        "à mesa" => ["at the table", "en la mesa", "à table", "за столом"],
        "na sala" => [
            "in the living room",
            "en el salón",
            "au salon",
            "у вітальні",
        ],
        "de guarda-roupa" => [
            "of wardrobe",
            "de armario ropero",
            "de dressing",
            "шафи для одягу",
        ],
        "Nada a apontar para esses moradores." => [
            "Nothing to point out for these occupants.",
            "Nada que señalar para estos habitantes.",
            "Rien à signaler pour ces occupants.",
            "Для цих мешканців зауважень немає.",
        ],
        "Selecionar na planta" => [
            "Select on the plan",
            "Seleccionar en el plano",
            "Sélectionner sur le plan",
            "Виділити на плані",
        ],
        "Aplicar correção" => [
            "Apply fix",
            "Aplicar corrección",
            "Appliquer la correction",
            "Застосувати виправлення",
        ],
        "Correção aplicada (Ctrl+Z desfaz)." => [
            "Fix applied (Ctrl+Z undoes it).",
            "Corrección aplicada (Ctrl+Z deshace).",
            "Correction appliquée (Ctrl+Z annule).",
            "Виправлення застосовано (Ctrl+Z скасовує).",
        ],
        "Referências: NBR 9050, NBR 15575-1 e IBGE; áreas e janelas variam com o código de obras do município." => {
            [
                "References: NBR 9050, NBR 15575-1 and IBGE; areas and windows vary with the city's building code.",
                "Referencias: NBR 9050, NBR 15575-1 e IBGE; áreas y ventanas varían según el código de obras del municipio.",
                "Références : NBR 9050, NBR 15575-1 et IBGE ; surfaces et fenêtres varient selon le code de la construction de la commune.",
                "Джерела: NBR 9050, NBR 15575-1 та IBGE; площі та вікна залежать від місцевих будівельних норм.",
            ]
        }
        " · luz" => [" · light", " · luz", " · lumière", " · світло"],
        "(cópia)" => ["(copy)", "(copia)", "(copie)", "(копія)"],
        "Abrir" => ["Open", "Abrir", "Ouvrir", "Відкрити"],
        "Abrir (Ctrl+O)" => [
            "Open (Ctrl+O)",
            "Abrir (Ctrl+O)",
            "Ouvrir (Ctrl+O)",
            "Відкрити (Ctrl+O)",
        ],
        "Abrir link" => [
            "Open link",
            "Abrir enlace",
            "Ouvrir le lien",
            "Відкрити посилання",
        ],
        "Abrir…" => ["Open…", "Abrir…", "Ouvrir…", "Відкрити…"],
        "Adicionar texto" => [
            "Add text",
            "Añadir texto",
            "Ajouter du texte",
            "Додати текст",
        ],
        "Adicionar um andar acima do mais alto" => [
            "Add a storey above the highest one",
            "Añadir una planta encima de la más alta",
            "Ajouter un étage au-dessus du plus haut",
            "Додати поверх над найвищим",
        ],
        "Afastamento" => ["Offset", "Separación", "Décalage", "Відступ"],
        "Afastar (Ctrl -)" => [
            "Zoom out (Ctrl -)",
            "Alejar (Ctrl -)",
            "Zoom arrière (Ctrl -)",
            "Віддалити (Ctrl -)",
        ],
        "Ajuda" => ["Help", "Ayuda", "Aide", "Довідка"],
        "Quadro de cargas e NBR 5410" => [
            "Load schedule and NBR 5410",
            "Cuadro de cargas y NBR 5410",
            "Tableau des charges et NBR 5410",
            "Таблиця навантажень і NBR 5410",
        ],
        "Nenhum circuito: atribua os pontos a circuitos (MCP electrical assign)." => [
            "No circuits: assign the points to circuits (MCP electrical assign).",
            "Ningún circuito: asigne los puntos a circuitos (MCP electrical assign).",
            "Aucun circuit : affectez les points à des circuits (MCP electrical assign).",
            "Немає кіл: розподіліть точки по колах (MCP electrical assign).",
        ],
        "Circuito" => ["Circuit", "Circuito", "Circuit", "Коло"],
        "Pontos" => ["Points", "Puntos", "Points", "Точки"],
        "VA" => ["VA", "VA", "VA", "ВА"],
        "V" => ["V", "V", "V", "В"],
        "A" => ["A", "A", "A", "А"],
        "Fio mm²" => ["Wire mm²", "Hilo mm²", "Fil mm²", "Переріз, мм²"],
        "Disjuntor" => ["Breaker", "Magnetotérmico", "Disjoncteur", "Автомат"],
        "DR" => ["RCD", "Diferencial", "Différentiel", "ПЗВ"],
        "Total instalado:" => [
            "Installed total:",
            "Total instalado:",
            "Total installé :",
            "Загальна встановлена потужність:",
        ],
        "Nada a apontar." => [
            "Nothing to point out.",
            "Nada que señalar.",
            "Rien à signaler.",
            "Зауважень немає.",
        ],
        "Camadas" => ["Layers", "Capas", "Calques", "Шари"],
        "Iluminação" => ["Lighting", "Iluminación", "Éclairage", "Освітлення"],
        "Eletrodomésticos" => [
            "Appliances",
            "Electrodomésticos",
            "Appareils",
            "Побутова техніка",
        ],
        "Marcenaria" => ["Joinery", "Carpintería", "Menuiserie", "Корпусні меблі"],
        "Mostrar ou esconder na planta e no 3D" => [
            "Show or hide on the plan and in 3D",
            "Mostrar u ocultar en el plano y en 3D",
            "Afficher ou masquer sur le plan et en 3D",
            "Показати або сховати на плані та в 3D",
        ],
        "Mostrar tudo no 3D" => [
            "Show everything in 3D",
            "Mostrar todo en 3D",
            "Tout afficher en 3D",
            "Показувати все в 3D",
        ],
        "Ligado, o 3D mostra a obra inteira; desligado, esconde o que a planta esconde" => [
            "On, the 3D shows the whole building; off, it hides what the plan hides",
            "Activado, el 3D muestra la obra entera; desactivado, oculta lo que el plano oculta",
            "Activé, la 3D montre tout le bâtiment ; désactivé, elle masque ce que le plan masque",
            "Увімкнено — 3D показує всю будівлю; вимкнено — ховає те, що ховає план",
        ],
        "Enviar relatórios de erro" => [
            "Send error reports",
            "Enviar informes de error",
            "Envoyer les rapports d'erreur",
            "Надсилати звіти про помилки",
        ],
        "Falhas e notas de uso vão para os desenvolvedores, sem o seu projeto, sem IP e sem nome da máquina." => {
            [
                "Crashes and usage notes go to the developers, without your project, your IP or your machine's name.",
                "Los fallos y las notas de uso van a los desarrolladores, sin su proyecto, sin IP y sin el nombre de la máquina.",
                "Les plantages et les notes d'usage vont aux développeurs, sans votre projet, sans IP ni nom de machine.",
                "Дані про збої та використання надходять розробникам — без вашого проєкту, без IP і без назви комп'ютера.",
            ]
        }
        "Ajustes…" => ["Settings…", "Ajustes…", "Réglages…", "Налаштування…"],
        "Alinhamento" => ["Alignment", "Alineación", "Alignement", "Вирівнювання"],
        "Altura" => ["Height", "Altura", "Hauteur", "Висота"],
        "Andar" => ["Storey", "Planta", "Étage", "Поверх"],
        "Aproximar (Ctrl +)" => [
            "Zoom in (Ctrl +)",
            "Acercar (Ctrl +)",
            "Zoom avant (Ctrl +)",
            "Наблизити (Ctrl +)",
        ],
        "Apoiar o projeto" => [
            "Support the project",
            "Apoyar el proyecto",
            "Soutenir le projet",
            "Підтримати проєкт",
        ],
        "Apoiar no Buy Me a Coffee" => [
            "Support on Buy Me a Coffee",
            "Apoyar en Buy Me a Coffee",
            "Soutenir sur Buy Me a Coffee",
            "Підтримати на Buy Me a Coffee",
        ],
        "Abrir o GitHub" => [
            "Open GitHub",
            "Abrir GitHub",
            "Ouvrir GitHub",
            "Відкрити GitHub",
        ],
        "Agora não" => ["Not now", "Ahora no", "Pas maintenant", "Не зараз"],
        "Nova versão {} disponível" => [
            "New version {} available",
            "Nueva versión {} disponible",
            "Nouvelle version {} disponible",
            "Доступна нова версія {}",
        ],
        "Muita gente usa o 3D New Era AI todos os dias, mas quase ninguém apoia o projeto." => [
            "Many people use 3D New Era AI every day, but almost no one supports the project.",
            "Mucha gente usa 3D New Era AI todos los días, pero casi nadie apoya el proyecto.",
            "Beaucoup de gens utilisent 3D New Era AI chaque jour, mais presque personne ne soutient le projet.",
            "Багато людей щодня користуються 3D New Era AI, але майже ніхто не підтримує проєкт.",
        ],
        "As IAs que usam a ferramenta enviam sozinhas relatos de problemas e sugestões de melhoria, e eles chegam em grande número. Quero corrigir todos, mas me falta tempo e dinheiro para pagar as ferramentas de IA e os tokens que esse trabalho exige." => {
            [
                "The AIs that use the tool send problem reports and improvement suggestions on their own, and they arrive in large numbers. I want to fix them all, but I lack the time and the money to pay for the AI tools and tokens this work takes.",
                "Las IA que usan la herramienta envían solas reportes de problemas y sugerencias de mejora, y llegan en gran número. Quiero corregirlos todos, pero me falta tiempo y dinero para pagar las herramientas de IA y los tokens que este trabajo exige.",
                "Les IA qui utilisent l'outil envoient d'elles-mêmes des rapports de problèmes et des suggestions d'amélioration, et ils arrivent en grand nombre. Je veux tous les corriger, mais il me manque le temps et l'argent pour payer les outils d'IA et les tokens que ce travail demande.",
                "ШІ, що користуються інструментом, самі надсилають повідомлення про проблеми та пропозиції щодо покращень, і їх надходить дуже багато. Я хочу виправити все, але мені бракує часу й грошей, щоб платити за інструменти ШІ та токени, яких потребує ця робота.",
            ]
        }
        "Se a ferramenta é útil para você, considere apoiar no Buy Me a Coffee." => [
            "If the tool is useful to you, please consider supporting it on Buy Me a Coffee.",
            "Si la herramienta te resulta útil, considera apoyarla en Buy Me a Coffee.",
            "Si l'outil vous est utile, pensez à le soutenir sur Buy Me a Coffee.",
            "Якщо інструмент вам корисний, підтримайте його на Buy Me a Coffee.",
        ],
        "Também dá para ajudar com código: abra issues e pull requests no GitHub. Todos serão avaliados, e o que for aceito entra na próxima versão." => {
            [
                "You can also help with code: open issues and pull requests on GitHub. Every one will be reviewed, and what is accepted goes into the next release.",
                "También puedes ayudar con código: abre issues y pull requests en GitHub. Todos serán evaluados, y lo que se acepte entra en la próxima versión.",
                "Vous pouvez aussi aider avec du code : ouvrez des issues et des pull requests sur GitHub. Toutes seront examinées, et ce qui est accepté entre dans la prochaine version.",
                "Допомогти можна й кодом: відкривайте issues та pull requests на GitHub. Кожен буде розглянуто, а прийняте увійде до наступного випуску.",
            ]
        }
        "Arco" => ["Arc", "Arco", "Arc", "Дуга"],
        "Arquitetura" => [
            "Architecture",
            "Arquitectura",
            "Architecture",
            "Архітектура",
        ],
        "Arquivo" => ["File", "Archivo", "Fichier", "Файл"],
        "Arraste para mover a vista" => [
            "Drag to move the view",
            "Arrastre para mover la vista",
            "Faites glisser pour déplacer la vue",
            "Перетягніть, щоб посунути вигляд",
        ],
        "Arraste: girar · Shift/botão do meio: mover · Scroll: zoom · F: enquadrar" => [
            "Drag: orbit · Shift/middle button: pan · Scroll: zoom · F: frame",
            "Arrastrar: girar · Mayús/botón central: mover · Rueda: zoom · F: encuadrar",
            "Glisser : pivoter · Maj/bouton du milieu : déplacer · Molette : zoom · F : cadrer",
            "Перетягування: обертати · Shift/середня кнопка: зсув · Колесо: масштаб · F: вмістити",
        ],
        "Atalhos e ferramentas" => [
            "Shortcuts and tools",
            "Atajos y herramientas",
            "Raccourcis et outils",
            "Гарячі клавіші та інструменти",
        ],
        "Automático (símbolos e vista de cima dos modelos)" => [
            "Automatic (symbols, top views for models)",
            "Automático (símbolos y vista superior de los modelos)",
            "Automatique (symboles, vues de dessus pour les modèles)",
            "Автоматично (символи та вигляд моделей згори)",
        ],
        "Autor" => ["Author", "Autor", "Auteur", "Автор"],
        "Boa" => ["Good", "Buena", "Bonne", "Добра"],
        "Buscar: cama, janela, sofá…" => [
            "Search: bed, window, sofa…",
            "Buscar: cama, ventana, sofá…",
            "Rechercher : lit, fenêtre, canapé…",
            "Пошук: ліжко, вікно, диван…",
        ],
        "Calibrar e posicionar" => [
            "Calibrate and place",
            "Calibrar y posicionar",
            "Calibrer et placer",
            "Калібрувати й розмістити",
        ],
        "Calibrar escala da imagem" => [
            "Calibrate image scale",
            "Calibrar escala de la imagen",
            "Calibrer l'échelle de l'image",
            "Калібрувати масштаб зображення",
        ],
        "Cancelar" => ["Cancel", "Cancelar", "Annuler", "Скасувати"],
        "Casa e bússola" => [
            "Home and compass",
            "Casa y brújula",
            "Maison et boussole",
            "Будинок і компас",
        ],
        "Casa e bússola…" => [
            "Home and compass…",
            "Casa y brújula…",
            "Maison et boussole…",
            "Будинок і компас…",
        ],
        "Catálogo" => ["Catalog", "Catálogo", "Catalogue", "Каталог"],
        "Catálogo de origem" => [
            "Source catalog",
            "Catálogo de origen",
            "Catalogue d'origine",
            "Вихідний каталог",
        ],
        "Centro" => ["Center", "Centro", "Centre", "Центр"],
        "Centro (x, y)" => [
            "Center (x, y)",
            "Centro (x, y)",
            "Centre (x, y)",
            "Центр (x, y)",
        ],
        "Clique dois pontos de medida conhecida · arraste para posicionar a imagem" => [
            "Click two points of a known distance · drag to place the image",
            "Haga clic en dos puntos de distancia conocida · arrastre para posicionar la imagen",
            "Cliquez deux points d'une distance connue · faites glisser pour placer l'image",
            "Клацніть дві точки з відомою відстанню · перетягніть, щоб розмістити зображення",
        ],
        "Clique e depois clique na planta para posicionar" => [
            "Click, then click on the plan to place",
            "Haga clic y luego en el plano para posicionar",
            "Cliquez, puis cliquez sur le plan pour placer",
            "Клацніть, а потім клацніть на плані, щоб розмістити",
        ],
        "Clique encadeia paredes · digite o comprimento + Enter · Shift desliga o ímã · duplo clique encerra" => {
            [
                "Clicks chain walls · type a length + Enter · Shift disables magnetism · double-click ends",
                "El clic encadena paredes · escriba la longitud + Enter · Mayús desactiva el imán · doble clic termina",
                "Les clics enchaînent les murs · tapez une longueur + Entrée · Maj désactive l'aimant · double-clic termine",
                "Клацання з'єднують стіни · введіть довжину + Enter · Shift вимикає прилипання · подвійне клацання завершує",
            ]
        }
        "Clique início e fim, mova para afastar e clique · duplo clique numa parede cota a parede" => {
            [
                "Click start and end, move to offset and click · double-click a wall to dimension it",
                "Haga clic en inicio y fin, mueva para separar y haga clic · doble clic en una pared la acota",
                "Cliquez début et fin, déplacez pour décaler et cliquez · double-clic sur un mur pour le coter",
                "Клацніть початок і кінець, посуньте для відступу й клацніть · подвійне клацання на стіні проставляє її розмір",
            ]
        }
        "Clique onde o texto deve ficar" => [
            "Click where the text goes",
            "Haga clic donde debe quedar el texto",
            "Cliquez à l'endroit où placer le texte",
            "Клацніть там, де має бути текст",
        ],
        "Clique os cantos e duplo clique fecha · duplo clique dentro de paredes detecta o cômodo" => {
            [
                "Click the corners and double-click to close · double-click inside walls detects the room",
                "Haga clic en las esquinas y doble clic para cerrar · doble clic dentro de paredes detecta la habitación",
                "Cliquez les coins et double-cliquez pour fermer · double-clic à l'intérieur des murs détecte la pièce",
                "Клацніть кути, подвійне клацання замикає · подвійне клацання всередині стін знаходить кімнату",
            ]
        }
        "Clique os pontos e duplo clique encerra · na Elétrica/Hidráulica a linha vira eletroduto/tubulação" => {
            [
                "Click the points and double-click to end · in Electrical/Plumbing lines become conduits/pipes",
                "Haga clic en los puntos y doble clic termina · en Eléctrica/Fontanería la línea se vuelve tubo/canalización",
                "Cliquez les points et double-cliquez pour terminer · en Électricité/Plomberie la ligne devient conduit/tuyau",
                "Клацніть точки, подвійне клацання завершує · в Електриці/Сантехніці лінія стає кабелепроводом/трубою",
            ]
        }
        "Clique para posicionar · portas e janelas encaixam na parede mais próxima · Esc cancela" => {
            [
                "Click to place · doors and windows snap into the nearest wall · Esc cancels",
                "Haga clic para posicionar · puertas y ventanas encajan en la pared más cercana · Esc cancela",
                "Cliquez pour placer · portes et fenêtres s'encastrent dans le mur le plus proche · Échap annule",
                "Клацніть, щоб розмістити · двері та вікна вбудовуються в найближчу стіну · Esc скасовує",
            ]
        }
        "Clique seleciona · Ctrl+clique soma · arraste move · alças editam · duplo clique modifica" => {
            [
                "Click selects · Ctrl+click adds · drag moves · handles edit · double-click modifies",
                "El clic selecciona · Ctrl+clic suma · arrastrar mueve · los tiradores editan · doble clic modifica",
                "Le clic sélectionne · Ctrl+clic ajoute · glisser déplace · les poignées modifient · double-clic édite",
                "Клацання виділяє · Ctrl+клацання додає · перетягування переміщує · маркери редагують · подвійне клацання змінює",
            ]
        }
        "Colar" => ["Paste", "Pegar", "Coller", "Вставити"],
        "Comece desenhando paredes (W), importe uma planta como imagem de fundo, ou peça para a IA via MCP." => {
            [
                "Start by drawing walls (W), import a plan as background image, or ask the AI through MCP.",
                "Empiece dibujando paredes (W), importe un plano como imagen de fondo o pídalo a la IA vía MCP.",
                "Commencez par tracer des murs (W), importez un plan en image de fond, ou demandez-le à l'IA via MCP.",
                "Почніть із креслення стін (W), імпортуйте план як фонове зображення або попросіть ШІ через MCP.",
            ]
        }
        "Comparar" => ["Compare", "Comparar", "Comparer", "Порівняти"],
        "Comparar as versões" => [
            "Compare versions",
            "Comparar las versiones",
            "Comparer les versions",
            "Порівняти версії",
        ],
        "Comparar versões" => [
            "Compare versions",
            "Comparar versiones",
            "Comparer les versions",
            "Порівняти версії",
        ],
        "Comprimento" => ["Length", "Longitud", "Longueur", "Довжина"],
        "Comprimento exato da parede sendo desenhada" => [
            "Exact length of the wall being drawn",
            "Longitud exacta de la pared que se está dibujando",
            "Longueur exacte du mur en cours de tracé",
            "Точна довжина стіни, що креслиться",
        ],
        "Contorno" => ["Outline", "Contorno", "Contour", "Контур"],
        "Contínuo" => ["Solid", "Continuo", "Continu", "Суцільна"],
        "Copiar" => ["Copy", "Copiar", "Copier", "Копіювати"],
        "Copiar · Recortar · Colar · Duplicar" => [
            "Copy · Cut · Paste · Duplicate",
            "Copiar · Cortar · Pegar · Duplicar",
            "Copier · Couper · Coller · Dupliquer",
            "Копіювати · Вирізати · Вставити · Дублювати",
        ],
        "Cor" => ["Color", "Color", "Couleur", "Колір"],
        "Cotar paredes selecionadas" => [
            "Dimension selected walls",
            "Acotar paredes seleccionadas",
            "Coter les murs sélectionnés",
            "Проставити розміри виділених стін",
        ],
        "Cotas automáticas (engenharia)" => [
            "Automatic dimensions (engineering)",
            "Cotas automáticas (ingeniería)",
            "Cotes automatiques (ingénierie)",
            "Автоматичні розміри (інженерні)",
        ],
        "Criar cotas" => [
            "Create dimensions",
            "Crear cotas",
            "Créer des cotes",
            "Проставити розміри",
        ],
        "Criar cômodos" => [
            "Create rooms",
            "Crear habitaciones",
            "Créer des pièces",
            "Створити кімнати",
        ],
        "Criar foto" => [
            "Create photo",
            "Crear foto",
            "Créer une photo",
            "Створити фото",
        ],
        "Criar foto…" => [
            "Create photo…",
            "Crear foto…",
            "Créer une photo…",
            "Створити фото…",
        ],
        "Criar vídeo" => [
            "Create video",
            "Crear vídeo",
            "Créer une vidéo",
            "Створити відео",
        ],
        "Criar vídeo…" => [
            "Create video…",
            "Crear vídeo…",
            "Créer une vidéo…",
            "Створити відео…",
        ],
        "Pontos do caminho" => [
            "Path points",
            "Puntos del recorrido",
            "Points du parcours",
            "Точки маршруту",
        ],
        "Adicionar ponto atual" => [
            "Add current point",
            "Añadir punto actual",
            "Ajouter le point actuel",
            "Додати поточну точку",
        ],
        "Ative o visitante na vista 3D" => [
            "Turn on the visitor in the 3D view",
            "Active el visitante en la vista 3D",
            "Activez le visiteur dans la vue 3D",
            "Увімкніть відвідувача у 3D-вигляді",
        ],
        "Órbita aérea" => [
            "Aerial orbit",
            "Órbita aérea",
            "Orbite aérienne",
            "Політ по орбіті",
        ],
        "Limpar" => ["Clear", "Limpiar", "Effacer", "Очистити"],
        "Quadros por segundo" => [
            "Frames per second",
            "Cuadros por segundo",
            "Images par seconde",
            "Кадрів на секунду",
        ],
        "Velocidade" => ["Speed", "Velocidad", "Vitesse", "Швидкість"],
        "Gerar vídeo…" => [
            "Render video…",
            "Generar vídeo…",
            "Générer la vidéo…",
            "Згенерувати відео…",
        ],
        "Vídeo salvo em" => [
            "Video saved to",
            "Vídeo guardado en",
            "Vidéo enregistrée dans",
            "Відео збережено в",
        ],
        "quadros" => ["frames", "cuadros", "images", "кадрів"],
        "Sol pela bússola e hora" => [
            "Sun from compass and time",
            "Sol según brújula y hora",
            "Soleil selon la boussole et l'heure",
            "Сонце за компасом і часом",
        ],
        "Vista 3D" => ["3D view", "Vista 3D", "Vue 3D", "3D-вигляд"],
        "Mostrar em 3D" => [
            "Show in 3D",
            "Mostrar en 3D",
            "Afficher en 3D",
            "Показати в 3D",
        ],
        "Inclinação" => ["Tilt", "Inclinación", "Inclinaison", "Нахил"],
        "Deitado" => ["Lying", "Tumbado", "Couché", "Лежачи"],
        "Em pé" => ["Standing", "De pie", "Debout", "Стоячи"],
        "Editor no navegador" => [
            "Browser editor",
            "Editor en el navegador",
            "Éditeur dans le navigateur",
            "Редактор у браузері",
        ],
        "Disponível no aplicativo para desktop." => [
            "Available in the desktop app.",
            "Disponible en la aplicación de escritorio.",
            "Disponible dans l'application de bureau.",
            "Доступно в настільному застосунку.",
        ],
        "No navegador, exporte em .glb." => [
            "In the browser, export as .glb.",
            "En el navegador, exporte en .glb.",
            "Dans le navigateur, exportez en .glb.",
            "У браузері експортуйте в .glb.",
        ],
        "Nenhum plugin instalado" => [
            "No plugins installed",
            "Ningún plugin instalado",
            "Aucun plugin installé",
            "Не встановлено жодного плагіна",
        ],
        "Pastas" => ["Folders", "Carpetas", "Dossiers", "Теки"],
        "Rodando" => ["Running", "En ejecución", "En cours", "Працює"],
        "concluído" => ["done", "terminado", "terminé", "готово"],
        "Acompanha as paredes" => [
            "Follows the walls",
            "Sigue las paredes",
            "Suit les murs",
            "Повторює контур стін",
        ],
        "O contorno é detectado pelas paredes e divisores e se ajusta quando eles mudam" => [
            "The outline is detected from walls and dividers and adjusts when they change",
            "El contorno se detecta por las paredes y los divisores y se ajusta cuando cambian",
            "Le contour est détecté d'après les murs et les séparations et s'ajuste quand ils changent",
            "Контур визначається за стінами й роздільниками та підлаштовується, коли вони змінюються",
        ],
        "Ambientes" => ["Rooms", "Ambientes", "Espaces", "Приміщення"],
        "Divisor de ambiente" => [
            "Room divider",
            "Divisor de ambiente",
            "Séparation d'espace",
            "Роздільник приміщень",
        ],
        "Separa ambientes sem parede (ex.: sala e jantar integrados)" => [
            "Separates rooms without a wall (e.g. open living and dining)",
            "Separa ambientes sin pared (p. ej.: salón y comedor integrados)",
            "Sépare des espaces sans mur (ex. : salon et salle à manger ouverts)",
            "Розділяє приміщення без стіни (напр., суміщені вітальня та їдальня)",
        ],
        "Escala vertical" => [
            "Vertical scale",
            "Escala vertical",
            "Échelle verticale",
            "Вертикальний масштаб",
        ],
        "Diferente" => ["Different", "Diferente", "Différent", "Різні"],
        "Criar paredes" => [
            "Create walls",
            "Crear paredes",
            "Créer des murs",
            "Створити стіни",
        ],
        "Curva suave" => [
            "Smooth curve",
            "Curva suave",
            "Courbe douce",
            "Плавна крива",
        ],
        "Cômodos" => ["Rooms", "Habitaciones", "Pièces", "Кімнати"],
        "Da sua biblioteca de modelos. Clique para pôr no centro da planta" => [
            "From your model library. Click to put it in the middle of the plan",
            "De tu biblioteca de modelos. Haz clic para ponerlo en el centro del plano",
            "De votre bibliothèque de modèles. Cliquez pour le poser au centre du plan",
            "З вашої бібліотеки моделей. Клацніть, щоб поставити в центр плану",
        ],
        "Descartar" => ["Discard", "Descartar", "Abandonner", "Відкинути"],
        "Descrição" => ["Description", "Descripción", "Description", "Опис"],
        "Desenhar linhas" => [
            "Draw lines",
            "Dibujar líneas",
            "Tracer des lignes",
            "Креслити лінії",
        ],
        "Desfazer" => ["Undo", "Deshacer", "Annuler", "Скасувати"],
        "Desfazer (Ctrl+Z)" => [
            "Undo (Ctrl+Z)",
            "Deshacer (Ctrl+Z)",
            "Annuler (Ctrl+Z)",
            "Скасувати (Ctrl+Z)",
        ],
        "Desfazer · Refazer (inclusive o que a IA fez)" => [
            "Undo · Redo (including what the AI did)",
            "Deshacer · Rehacer (incluso lo que hizo la IA)",
            "Annuler · Rétablir (y compris ce qu'a fait l'IA)",
            "Скасувати · Повторити (включно з тим, що зробив ШІ)",
        ],
        "Desliga o ímã (ângulos de 15°, pontos e grade)" => [
            "Disables magnetism (15° angles, points and grid)",
            "Desactiva el imán (ángulos de 15°, puntos y cuadrícula)",
            "Désactive l'aimant (angles de 15°, points et grille)",
            "Вимикає прилипання (кути з кроком 15°, точки та сітка)",
        ],
        "Detalhes: marca, modelo e link" => [
            "Details: brand, model and link",
            "Detalles: marca, modelo y enlace",
            "Détails : marque, modèle et lien",
            "Подробиці: марка, модель і посилання",
        ],
        "Digitar número + Enter" => [
            "Type number + Enter",
            "Escribir número + Enter",
            "Taper un nombre + Entrée",
            "Ввести число + Enter",
        ],
        "Direita" => ["Right", "Derecha", "Droite", "Праворуч"],
        "Disco" => ["Disc", "Disco", "Disque", "Диск"],
        "Distância real" => [
            "Real distance",
            "Distancia real",
            "Distance réelle",
            "Реальна відстань",
        ],
        "Dividir parede ao meio" => [
            "Split wall in half",
            "Dividir la pared por la mitad",
            "Couper le mur en deux",
            "Розділити стіну навпіл",
        ],
        "Diâmetro" => ["Diameter", "Diámetro", "Diamètre", "Діаметр"],
        "Dobradiça à direita" => [
            "Hinge on the right",
            "Bisagra a la derecha",
            "Charnière à droite",
            "Петлі праворуч",
        ],
        "Duplicar" => ["Duplicate", "Duplicar", "Dupliquer", "Дублювати"],
        "Duplicar versão atual" => [
            "Duplicate current version",
            "Duplicar versión actual",
            "Dupliquer la version actuelle",
            "Дублювати поточну версію",
        ],
        "Duplicar versão · Próxima versão (guias)" => [
            "Duplicate version · Next version (tabs)",
            "Duplicar versión · Siguiente versión (pestañas)",
            "Dupliquer la version · Version suivante (onglets)",
            "Дублювати версію · Наступна версія (вкладки)",
        ],
        "Duplo clique" => [
            "Double-click",
            "Doble clic",
            "Double-clic",
            "Подвійне клацання",
        ],
        "Editar" => ["Edit", "Editar", "Modifier", "Редагування"],
        "Editar andar…" => [
            "Edit storey…",
            "Editar planta…",
            "Modifier l'étage…",
            "Редагувати поверх…",
        ],
        "Elevação" => ["Elevation", "Elevación", "Élévation", "Висота над підлогою"],
        "Elevação do piso" => [
            "Floor elevation",
            "Elevación del suelo",
            "Niveau du sol",
            "Рівень підлоги",
        ],
        "Elétrica" => ["Electrical", "Eléctrica", "Électricité", "Електрика"],
        "Elétrica (tomadas, luz, cabos)" => [
            "Electrical (outlets, lights, cables)",
            "Eléctrica (enchufes, luz, cables)",
            "Électricité (prises, éclairage, câbles)",
            "Електрика (розетки, світло, кабелі)",
        ],
        "Encerra paredes · fecha/detecta cômodo · cota parede · modifica" => [
            "Ends walls · closes/detects room · dimensions wall · modifies",
            "Termina paredes · cierra/detecta habitación · acota pared · modifica",
            "Termine les murs · ferme/détecte la pièce · cote le mur · modifie",
            "Завершує стіни · замикає/знаходить кімнату · проставляє розмір стіни · змінює",
        ],
        "Enquadrar (Ctrl+0)" => [
            "Frame (Ctrl+0)",
            "Encuadrar (Ctrl+0)",
            "Cadrer (Ctrl+0)",
            "Вмістити (Ctrl+0)",
        ],
        "Enquadrar 3D" => ["Frame 3D", "Encuadrar 3D", "Cadrer la 3D", "Вмістити 3D"],
        "Enquadrar planta" => [
            "Frame plan",
            "Encuadrar plano",
            "Cadrer le plan",
            "Вмістити план",
        ],
        "Escala" => ["Scale", "Escala", "Échelle", "Масштаб"],
        "Escala calibrada. Arraste para posicionar a imagem ou troque de ferramenta." => [
            "Scale calibrated. Drag to place the image or switch tools.",
            "Escala calibrada. Arrastre para posicionar la imagen o cambie de herramienta.",
            "Échelle calibrée. Faites glisser pour placer l'image ou changez d'outil.",
            "Масштаб відкалібровано. Перетягніть, щоб розмістити зображення, або змініть інструмент.",
        ],
        "Espelhado" => ["Mirrored", "Reflejado", "Miroir", "Віддзеркалено"],
        "Espessura" => ["Thickness", "Espesor", "Épaisseur", "Товщина"],
        "Espessura da laje" => [
            "Slab thickness",
            "Espesor de la losa",
            "Épaisseur de la dalle",
            "Товщина перекриття",
        ],
        "Esquerda" => ["Left", "Izquierda", "Gauche", "Ліворуч"],
        "Esta versão e o histórico dela serão removidos do projeto." => [
            "This version and its history will be removed from the project.",
            "Esta versión y su historial se quitarán del proyecto.",
            "Cette version et son historique seront retirés du projet.",
            "Цю версію та її історію буде вилучено з проєкту.",
        ],
        "Estilo" => ["Style", "Estilo", "Style", "Стиль"],
        "Excluir" => ["Delete", "Eliminar", "Supprimer", "Видалити"],
        "Excluir andar" => [
            "Delete storey",
            "Eliminar planta",
            "Supprimer l'étage",
            "Видалити поверх",
        ],
        "Exibir" => ["Show", "Ver", "Afficher", "Вигляд"],
        "Exportar 3D" => ["Export 3D", "Exportar 3D", "Exporter la 3D", "Експорт 3D"],
        "Exportar planta" => [
            "Export plan",
            "Exportar plano",
            "Exporter le plan",
            "Експорт плану",
        ],
        "Fechada" => ["Closed", "Cerrada", "Fermée", "Замкнена"],
        "Fechar" => ["Close", "Cerrar", "Fermer", "Закрити"],
        "Fechar versão" => [
            "Close version",
            "Cerrar versión",
            "Fermer la version",
            "Закрити версію",
        ],
        "Fim" => ["End", "Fin", "Fin", "Кінець"],
        "Fim (x, y)" => ["End (x, y)", "Fin (x, y)", "Fin (x, y)", "Кінець (x, y)"],
        "Grupo" => ["Group", "Grupo", "Groupe", "Група"],
        "Hidráulica" => ["Plumbing", "Fontanería", "Plomberie", "Сантехніка"],
        "Hora do dia" => [
            "Time of day",
            "Hora del día",
            "Heure de la journée",
            "Час доби",
        ],
        "Imagem" => ["Image", "Imagen", "Image", "Зображення"],
        "Imagem de fundo" => [
            "Background image",
            "Imagen de fondo",
            "Image de fond",
            "Фонове зображення",
        ],
        "Imagem importada. Clique em dois pontos de medida conhecida para calibrar; arraste para posicionar." => {
            [
                "Image imported. Click two points of a known distance to calibrate; drag to place.",
                "Imagen importada. Haga clic en dos puntos de distancia conocida para calibrar; arrastre para posicionar.",
                "Image importée. Cliquez deux points d'une distance connue pour calibrer ; faites glisser pour placer.",
                "Зображення імпортовано. Клацніть дві точки з відомою відстанню, щоб відкалібрувати; перетягніть, щоб розмістити.",
            ]
        }
        "Imagem…" => ["Image…", "Imagen…", "Image…", "Зображення…"],
        "Imagens" => ["Images", "Imágenes", "Images", "Зображення"],
        "Importar imagem de fundo" => [
            "Import background image",
            "Importar imagen de fondo",
            "Importer une image de fond",
            "Імпортувати фонове зображення",
        ],
        "Importar modelo 3D…" => [
            "Import 3D model…",
            "Importar modelo 3D…",
            "Importer un modèle 3D…",
            "Імпортувати 3D-модель…",
        ],
        "Importar…" => ["Import…", "Importar…", "Importer…", "Імпортувати…"],
        "Informações" => ["Information", "Información", "Informations", "Інформація"],
        "Início" => ["Start", "Inicio", "Début", "Початок"],
        "Início (x, y)" => [
            "Start (x, y)",
            "Inicio (x, y)",
            "Début (x, y)",
            "Початок (x, y)",
        ],
        "Itálico" => ["Italic", "Cursiva", "Italique", "Курсив"],
        "Lado direito" => ["Right side", "Lado derecho", "Côté droit", "Права сторона"],
        "Lado esquerdo" => ["Left side", "Lado izquierdo", "Côté gauche", "Ліва сторона"],
        "Largura" => ["Width", "Anchura", "Largeur", "Ширина"],
        "Legenda de símbolos (elétrica e hidráulica)" => [
            "Symbol legend (electrical and plumbing)",
            "Leyenda de símbolos (eléctrica y fontanería)",
            "Légende des symboles (électricité et plomberie)",
            "Умовні позначення (електрика та сантехніка)",
        ],
        "Licença" => ["License", "Licencia", "Licence", "Ліцензія"],
        "Linhas (tubulação / eletroduto)" => [
            "Lines (pipes / conduits)",
            "Líneas (tubería / tubo eléctrico)",
            "Lignes (tuyaux / conduits)",
            "Лінії (труби / кабелепроводи)",
        ],
        "Link" => ["Link", "Enlace", "Lien", "Посилання"],
        "MCP desligado" => ["MCP off", "MCP apagado", "MCP désactivé", "MCP вимкнено"],
        "Manter proporções" => [
            "Keep proportions",
            "Mantener proporciones",
            "Conserver les proportions",
            "Зберігати пропорції",
        ],
        "Marca" => ["Brand", "Marca", "Marque", "Марка"],
        "Me paga um café" => [
            "Buy me a coffee",
            "Invítame un café",
            "Offre-moi un café",
            "Пригостіть мене кавою",
        ],
        "Medida" => ["Measure", "Medida", "Mesure", "Розмір"],
        "Minha biblioteca" => [
            "My library",
            "Mi biblioteca",
            "Ma bibliothèque",
            "Моя бібліотека",
        ],
        "Modelo" => ["Model", "Modelo", "Modèle", "Модель"],
        "Modelo da biblioteca posicionado. Arraste para o lugar." => [
            "Library model placed. Drag it into place.",
            "Modelo de la biblioteca colocado. Arrástralo a su lugar.",
            "Modèle de la bibliothèque posé. Faites-le glisser à sa place.",
            "Модель із бібліотеки розміщено. Перетягніть її на місце.",
        ],
        "Modelo importado. Ajuste medidas com Enter ou pelas alças." => [
            "Model imported. Adjust sizes with Enter or the handles.",
            "Modelo importado. Ajuste las medidas con Enter o con los tiradores.",
            "Modèle importé. Ajustez les dimensions avec Entrée ou les poignées.",
            "Модель імпортовано. Налаштуйте розміри клавішею Enter або маркерами.",
        ],
        "Modelos 3D" => ["3D models", "Modelos 3D", "Modèles 3D", "3D-моделі"],
        "Modificar andar" => [
            "Modify storey",
            "Modificar planta",
            "Modifier l'étage",
            "Змінити поверх",
        ],
        "Modificar cota" => [
            "Modify dimension",
            "Modificar cota",
            "Modifier la cote",
            "Змінити розмірну лінію",
        ],
        "Modificar cômodo" => [
            "Modify room",
            "Modificar habitación",
            "Modifier la pièce",
            "Змінити кімнату",
        ],
        "Modificar linha" => [
            "Modify line",
            "Modificar línea",
            "Modifier la ligne",
            "Змінити лінію",
        ],
        "Modificar parede" => [
            "Modify wall",
            "Modificar pared",
            "Modifier le mur",
            "Змінити стіну",
        ],
        "Modificar texto" => [
            "Modify text",
            "Modificar texto",
            "Modifier le texte",
            "Змінити текст",
        ],
        "Modificar · Excluir · Cancelar" => [
            "Modify · Delete · Cancel",
            "Modificar · Eliminar · Cancelar",
            "Modifier · Supprimer · Annuler",
            "Змінити · Видалити · Скасувати",
        ],
        "Modificar…" => ["Modify…", "Modificar…", "Modifier…", "Змінити…"],
        "Mostrar bússola" => [
            "Show compass",
            "Mostrar brújula",
            "Afficher la boussole",
            "Показати компас",
        ],
        "Move a seleção 1 cm (10 cm)" => [
            "Moves the selection 1 cm (10 cm)",
            "Mueve la selección 1 cm (10 cm)",
            "Déplace la sélection de 1 cm (10 cm)",
            "Зсуває виділення на 1 см (10 см)",
        ],
        "Mover vista" => [
            "Pan view",
            "Mover vista",
            "Déplacer la vue",
            "Зсунути вигляд",
        ],
        "Máxima" => ["Best", "Máxima", "Maximale", "Найвища"],
        "Móveis" => ["Furniture", "Muebles", "Mobilier", "Меблі"],
        "Móveis na planta" => [
            "Furniture on the plan",
            "Muebles en el plano",
            "Mobilier sur le plan",
            "Меблі на плані",
        ],
        "Nada encontrado." => [
            "Nothing found.",
            "No se encontró nada.",
            "Aucun résultat.",
            "Нічого не знайдено.",
        ],
        "Negrito" => ["Bold", "Negrita", "Gras", "Жирний"],
        "Nenhum espaço fechado por paredes aqui" => [
            "No space enclosed by walls here",
            "Ningún espacio cerrado por paredes aquí",
            "Aucun espace fermé par des murs ici",
            "Тут немає простору, замкненого стінами",
        ],
        "Nenhum ponto" => ["No points", "Ningún punto", "Aucun point", "Немає точок"],
        "Nenhum ponto de vista salvo" => [
            "No saved points of view",
            "Ningún punto de vista guardado",
            "Aucun point de vue enregistré",
            "Немає збережених точок огляду",
        ],
        "Nome" => ["Name", "Nombre", "Nom", "Назва"],
        "Nome do projeto" => [
            "Project name",
            "Nombre del proyecto",
            "Nom du projet",
            "Назва проєкту",
        ],
        "Norte" => ["North", "Norte", "Nord", "Північ"],
        "Nova versão da planta" => [
            "New plan version",
            "Nueva versión del plano",
            "Nouvelle version du plan",
            "Нова версія плану",
        ],
        "Nova versão em branco" => [
            "New blank version",
            "Nueva versión en blanco",
            "Nouvelle version vierge",
            "Нова порожня версія",
        ],
        "Novo" => ["New", "Nuevo", "Nouveau", "Створити"],
        "Novo (Ctrl+N)" => [
            "New (Ctrl+N)",
            "Nuevo (Ctrl+N)",
            "Nouveau (Ctrl+N)",
            "Створити (Ctrl+N)",
        ],
        "Níveis na mesma elevação funcionam como layouts alternativos" => [
            "Levels at the same elevation work as alternative layouts",
            "Los niveles a la misma elevación funcionan como diseños alternativos",
            "Des niveaux à la même élévation servent de variantes d'aménagement",
            "Рівні на тій самій висоті слугують альтернативними варіантами планування",
        ],
        "O andar e tudo o que está nele serão removidos (dá para desfazer)." => [
            "The storey and everything on it will be removed (undoable).",
            "La planta y todo lo que hay en ella se quitarán (se puede deshacer).",
            "L'étage et tout ce qu'il contient seront supprimés (annulable).",
            "Поверх і все, що на ньому, буде вилучено (можна скасувати).",
        ],
        "O projeto tem alterações que ainda não foram salvas." => [
            "The project has unsaved changes.",
            "El proyecto tiene cambios sin guardar.",
            "Le projet a des modifications non enregistrées.",
            "У проєкті є незбережені зміни.",
        ],
        "OBJ + MTL…" => ["OBJ + MTL…", "OBJ + MTL…", "OBJ + MTL…", "OBJ + MTL…"],
        "OK" => ["OK", "OK", "OK", "OK"],
        "Opacidade" => ["Opacity", "Opacidad", "Opacité", "Непрозорість"],
        "Ordem (mesma elevação)" => [
            "Order (same elevation)",
            "Orden (misma elevación)",
            "Ordre (même élévation)",
            "Порядок (та сама висота)",
        ],
        "PDF (A3, ajustado à folha)…" => [
            "PDF (A3, fit to sheet)…",
            "PDF (A3, ajustado a la hoja)…",
            "PDF (A3, ajusté à la feuille)…",
            "PDF (A3, за розміром аркуша)…",
        ],
        "PDF 1:100…" => ["PDF 1:100…", "PDF 1:100…", "PDF 1:100…", "PDF 1:100…"],
        "PDF 1:50…" => ["PDF 1:50…", "PDF 1:50…", "PDF 1:50…", "PDF 1:50…"],
        "PNG…" => ["PNG…", "PNG…", "PNG…", "PNG…"],
        "Paredes" => ["Walls", "Paredes", "Murs", "Стіни"],
        "Inteiras" => ["Up", "Enteras", "Debout", "Цілі"],
        "Recortadas" => ["Cutaway", "Recortadas", "Coupés", "У розрізі"],
        "Baixas" => ["Down", "Bajas", "Baissés", "Опущені"],
        "Todas as paredes em pé" => [
            "Every wall standing",
            "Todas las paredes en pie",
            "Tous les murs debout",
            "Усі стіни стоять",
        ],
        "As paredes da frente descem para mostrar os cômodos" => [
            "The walls in front come down to show the rooms",
            "Las paredes de delante bajan para mostrar las habitaciones",
            "Les murs de devant s'abaissent pour montrer les pièces",
            "Передні стіни опускаються, щоб показати кімнати",
        ],
        "Todas as paredes descem" => [
            "Every wall comes down",
            "Todas las paredes bajan",
            "Tous les murs s'abaissent",
            "Усі стіни опускаються",
        ],
        "Personalizada" => ["Custom", "Personalizada", "Personnalisée", "Власна"],
        "Peça" => ["Tile", "Azulejo", "Carrelage", "Плитка"],
        "Pintura" => ["Paint", "Pintura", "Peinture", "Фарба"],
        "Piso" => ["Floor", "Suelo", "Sol", "Підлога"],
        "Planta" => ["Plan", "Plano", "Plan", "План"],
        "Pontilhado" => ["Dotted", "Punteado", "Pointillé", "Пунктир"],
        "Posição (x, y)" => [
            "Position (x, y)",
            "Posición (x, y)",
            "Position (x, y)",
            "Положення (x, y)",
        ],
        "Potência da luz" => [
            "Light power",
            "Potencia de la luz",
            "Puissance de la lumière",
            "Потужність світла",
        ],
        "Preço" => ["Price", "Precio", "Prix", "Ціна"],
        "Problemas" => ["Issues", "Problemas", "Problèmes", "Проблеми"],
        "Profundidade" => ["Depth", "Profundidad", "Profondeur", "Глибина"],
        "Projeto em edição: novos símbolos e linhas vão para ele" => [
            "Project being edited: new symbols and lines go there",
            "Proyecto en edición: los nuevos símbolos y líneas van a él",
            "Projet en cours d'édition : les nouveaux symboles et lignes y vont",
            "Проєкт, що редагується: нові символи й лінії потрапляють до нього",
        ],
        "Projetos (3D New Era AI, Sweet Home 3D)" => [
            "Projects (3D New Era AI, Sweet Home 3D)",
            "Proyectos (3D New Era AI, Sweet Home 3D)",
            "Projets (3D New Era AI, Sweet Home 3D)",
            "Проєкти (3D New Era AI, Sweet Home 3D)",
        ],
        "Pé-direito" => [
            "Ceiling height",
            "Altura libre",
            "Hauteur sous plafond",
            "Висота стелі",
        ],
        "Qualidade" => ["Quality", "Calidad", "Qualité", "Якість"],
        "Quantitativos" => [
            "Quantities",
            "Mediciones",
            "Quantitatifs",
            "Відомість обсягів",
        ],
        "Rascunho" => ["Draft", "Borrador", "Brouillon", "Чернетка"],
        "Recentes" => ["Recent", "Recientes", "Récents", "Нещодавні"],
        "Recortar" => ["Cut", "Cortar", "Couper", "Вирізати"],
        "Refazer" => ["Redo", "Rehacer", "Rétablir", "Повторити"],
        "Refazer (Ctrl+Shift+Z)" => [
            "Redo (Ctrl+Shift+Z)",
            "Rehacer (Ctrl+Shift+Z)",
            "Rétablir (Ctrl+Maj+Z)",
            "Повторити (Ctrl+Shift+Z)",
        ],
        "Referências dos cômodos" => [
            "Room references",
            "Referencias de las habitaciones",
            "Références des pièces",
            "Позначки кімнат",
        ],
        "Remover imagem" => [
            "Remove image",
            "Quitar imagen",
            "Retirer l'image",
            "Вилучити зображення",
        ],
        "Renderizar" => ["Render", "Renderizar", "Rendu", "Рендер"],
        "Renomear" => ["Rename", "Renombrar", "Renommer", "Перейменувати"],
        "Rotação" => ["Rotation", "Rotación", "Rotation", "Поворот"],
        "SVG em escala real…" => [
            "SVG at true scale…",
            "SVG a escala real…",
            "SVG à l'échelle réelle…",
            "SVG у реальному масштабі…",
        ],
        "Sair" => ["Quit", "Salir", "Quitter", "Вийти"],
        "Salvar" => ["Save", "Guardar", "Enregistrer", "Зберегти"],
        "Salvar (Ctrl+S)" => [
            "Save (Ctrl+S)",
            "Guardar (Ctrl+S)",
            "Enregistrer (Ctrl+S)",
            "Зберегти (Ctrl+S)",
        ],
        "Salvar PNG…" => [
            "Save PNG…",
            "Guardar PNG…",
            "Enregistrer en PNG…",
            "Зберегти PNG…",
        ],
        "Salvar alterações?" => [
            "Save changes?",
            "¿Guardar los cambios?",
            "Enregistrer les modifications ?",
            "Зберегти зміни?",
        ],
        "Salvar como…" => [
            "Save as…",
            "Guardar como…",
            "Enregistrer sous…",
            "Зберегти як…",
        ],
        "Salvar ponto de vista" => [
            "Save point of view",
            "Guardar punto de vista",
            "Enregistrer le point de vue",
            "Зберегти точку огляду",
        ],
        "Scroll · botão do meio · F" => [
            "Scroll · middle button · F",
            "Rueda · botón central · F",
            "Molette · bouton du milieu · F",
            "Колесо · середня кнопка · F",
        ],
        "Selecionar" => ["Select", "Seleccionar", "Sélectionner", "Виділити"],
        "Selecionar tudo" => [
            "Select all",
            "Seleccionar todo",
            "Tout sélectionner",
            "Виділити все",
        ],
        "Selecionar · Mover vista · Paredes · Cômodos · Cotas · Texto" => [
            "Select · Pan · Walls · Rooms · Dimensions · Text",
            "Seleccionar · Mover vista · Paredes · Habitaciones · Cotas · Texto",
            "Sélectionner · Déplacer · Murs · Pièces · Cotes · Texte",
            "Виділити · Зсунути вигляд · Стіни · Кімнати · Розміри · Текст",
        ],
        "Sem acabamento" => [
            "No finish",
            "Sin acabado",
            "Sans finition",
            "Без оздоблення",
        ],
        "Sem nome" => ["Unnamed", "Sin nombre", "Sans nom", "Без назви"],
        "Sem seta" => ["No arrow", "Sin flecha", "Sans flèche", "Без стрілки"],
        "Seta aberta" => [
            "Open arrow",
            "Flecha abierta",
            "Flèche ouverte",
            "Відкрита стрілка",
        ],
        "Seta cheia" => [
            "Filled arrow",
            "Flecha rellena",
            "Flèche pleine",
            "Суцільна стрілка",
        ],
        "Setas (+Shift)" => [
            "Arrows (+Shift)",
            "Flechas (+Mayús)",
            "Flèches (+Maj)",
            "Стрілки (+Shift)",
        ],
        "Shift (segurado)" => [
            "Shift (held)",
            "Mayús (mantenido)",
            "Maj (maintenue)",
            "Shift (утримувати)",
        ],
        "Sweet Home 3D" => [
            "Sweet Home 3D",
            "Sweet Home 3D",
            "Sweet Home 3D",
            "Sweet Home 3D",
        ],
        "Símbolos arquitetônicos" => [
            "Architectural symbols",
            "Símbolos arquitectónicos",
            "Symboles architecturaux",
            "Архітектурні символи",
        ],
        "Tamanho" => ["Size", "Tamaño", "Taille", "Розмір"],
        "Teto" => ["Ceiling", "Techo", "Plafond", "Стеля"],
        "Texto" => ["Text", "Texto", "Texte", "Текст"],
        "Tipo" => ["Type", "Tipo", "Type", "Тип"],
        "Tracejado" => ["Dashed", "Discontinuo", "Tirets", "Штрихова"],
        "Traço" => ["Dash", "Raya", "Tiret", "Штрих"],
        "Traço e dois pontos" => [
            "Dash dot dot",
            "Raya y dos puntos",
            "Tiret point point",
            "Штрихпунктир із двома крапками",
        ],
        "Traço e ponto" => ["Dash dot", "Raya y punto", "Tiret point", "Штрихпунктир"],
        "Térreo" => [
            "Ground floor",
            "Planta baja",
            "Rez-de-chaussée",
            "Перший поверх",
        ],
        "Unidade" => ["Unit", "Unidad", "Unité", "Одиниця"],
        "Unir paredes selecionadas" => [
            "Join selected walls",
            "Unir paredes seleccionadas",
            "Joindre les murs sélectionnés",
            "Об'єднати виділені стіни",
        ],
        "Usa o ponto de vista atual da vista 3D (aérea ou visitante)." => [
            "Uses the current 3D point of view (aerial or visitor).",
            "Usa el punto de vista actual de la vista 3D (aérea o visitante).",
            "Utilise le point de vue actuel de la vue 3D (aérienne ou visiteur).",
            "Використовує поточну точку огляду 3D-вигляду (з висоти або відвідувача).",
        ],
        "Ver" => ["View", "Ver", "Affichage", "Вигляд"],
        "Versão" => ["Version", "Versión", "Version", "Версія"],
        "Visitante" => ["Visitor", "Visitante", "Visiteur", "Відвідувач"],
        "Visitante — arraste: olhar · W/A/S/D ou setas: andar · Scroll: avançar · Esc: visão aérea" => {
            [
                "Visitor — drag: look · W/A/S/D or arrows: walk · Scroll: forward · Esc: aerial view",
                "Visitante — arrastrar: mirar · W/A/S/D o flechas: andar · Rueda: avanzar · Esc: vista aérea",
                "Visiteur — glisser : regarder · W/A/S/D ou flèches : marcher · Molette : avancer · Échap : vue aérienne",
                "Відвідувач — перетягування: дивитися · W/A/S/D або стрілки: іти · Колесо: вперед · Esc: вигляд з висоти",
            ]
        }
        "Vista de cima de todos" => [
            "Top views for all",
            "Vista superior de todos",
            "Vues de dessus pour tout",
            "Вигляд згори для всіх",
        ],
        "Visualização 3D requer o backend wgpu." => [
            "The 3D view needs the wgpu backend.",
            "La vista 3D necesita el backend wgpu.",
            "La vue 3D nécessite le backend wgpu.",
            "Для 3D-вигляду потрібен бекенд wgpu.",
        ],
        "Visão aérea" => [
            "Aerial view",
            "Vista aérea",
            "Vue aérienne",
            "Вигляд з висоти",
        ],
        "Frente" => ["Front", "Frente", "Face", "Спереду"],
        "Trás" => ["Back", "Detrás", "Arrière", "Ззаду"],
        "Topo" => ["Top", "Superior", "Dessus", "Згори"],
        "Visível" => ["Visible", "Visible", "Visible", "Видимий"],
        "Visível no 3D" => [
            "Visible in 3D",
            "Visible en 3D",
            "Visible en 3D",
            "Видимий у 3D",
        ],
        "Zoom · mover vista · enquadrar" => [
            "Zoom · pan · frame",
            "Zoom · mover vista · encuadrar",
            "Zoom · déplacer · cadrer",
            "Масштаб · зсув вигляду · вмістити",
        ],
        "glTF binário (.glb)…" => [
            "Binary glTF (.glb)…",
            "glTF binario (.glb)…",
            "glTF binaire (.glb)…",
            "Двійковий glTF (.glb)…",
        ],
        "peças" => ["pieces", "piezas", "pièces", "елементів"],
        "Área" => ["Area", "Área", "Surface", "Площа"],
        "Área na planta" => [
            "Area on the plan",
            "Área en el plano",
            "Surface sur le plan",
            "Площа на плані",
        ],
        "Ângulo" => ["Angle", "Ángulo", "Angle", "Кут"],
        "Cotas" => ["Dimensions", "Cotas", "Cotes", "Розміри"],
        "Textos" => ["Texts", "Textos", "Textes", "Тексти"],
        "Linhas" => ["Lines", "Líneas", "Lignes", "Лінії"],
        "paredes" => ["walls", "paredes", "murs", "стін"],
        "cômodos" => ["rooms", "habitaciones", "pièces", "кімнат"],
        "móveis" => ["pieces", "muebles", "meubles", "предметів меблів"],
        "grupo de" => ["group of", "grupo de", "groupe de", "група з"],
        "Sala de estar" => ["Living room", "Salón", "Salon", "Вітальня"],
        "Sala de jantar" => ["Dining room", "Comedor", "Salle à manger", "Їдальня"],
        "Cozinha" => ["Kitchen", "Cocina", "Cuisine", "Кухня"],
        "Quarto" => ["Bedroom", "Dormitorio", "Chambre", "Спальня"],
        "Banheiro" => ["Bathroom", "Baño", "Salle de bain", "Ванна кімната"],
        "Lavanderia" => ["Laundry", "Lavadero", "Buanderie", "Пральня"],
        "Escritório" => ["Office", "Despacho", "Bureau", "Кабінет"],
        "Portas e janelas" => [
            "Doors and windows",
            "Puertas y ventanas",
            "Portes et fenêtres",
            "Двері та вікна",
        ],
        "Estrutura" => ["Structure", "Estructura", "Structure", "Конструкції"],
        "Decoração" => ["Decor", "Decoración", "Décoration", "Декор"],
        "Área externa" => ["Outdoor", "Zona exterior", "Extérieur", "Подвір'я"],
        "atual" => ["current", "actual", "actuel", "поточна"],
        "Importado de {} — salve como projeto para manter tudo num arquivo" => [
            "Imported from {} — save it as a project to keep everything in one file",
            "Importado de {} — guárdelo como proyecto para tenerlo todo en un archivo",
            "Importé de {} — enregistrez-le en projet pour tout garder dans un seul fichier",
            "Імпортовано з {} — збережіть як проєкт, щоб усе було в одному файлі",
        ],
        "Aberto: {}" => ["Opened: {}", "Abierto: {}", "Ouvert : {}", "Відкрито: {}"],
        " · {} aviso(s)" => [
            " · {} warning(s)",
            " · {} aviso(s)",
            " · {} avertissement(s)",
            " · попереджень: {}",
        ],
        "⚠ Não foi possível abrir {}: {}" => [
            "⚠ Could not open {}: {}",
            "⚠ No se pudo abrir {}: {}",
            "⚠ Impossible d'ouvrir {} : {}",
            "⚠ Не вдалося відкрити {}: {}",
        ],
        "Pontos de vista ({})" => [
            "Points of view ({})",
            "Puntos de vista ({})",
            "Points de vue ({})",
            "Точки огляду ({})",
        ],
        "Ponto de vista {}" => [
            "Point of view {}",
            "Punto de vista {}",
            "Point de vue {}",
            "Точка огляду {}",
        ],
        "Salvo em {}" => [
            "Saved to {}",
            "Guardado en {}",
            "Enregistré dans {}",
            "Збережено в {}",
        ],
        "⚠ Não foi possível salvar: {}" => [
            "⚠ Could not save: {}",
            "⚠ No se pudo guardar: {}",
            "⚠ Impossible d'enregistrer : {}",
            "⚠ Не вдалося зберегти: {}",
        ],
        "Modelo 3D exportado para {}" => [
            "3D model exported to {}",
            "Modelo 3D exportado a {}",
            "Modèle 3D exporté vers {}",
            "3D-модель експортовано в {}",
        ],
        "⚠ Falha ao exportar: {}" => [
            "⚠ Export failed: {}",
            "⚠ Fallo al exportar: {}",
            "⚠ Échec de l'export : {}",
            "⚠ Помилка експорту: {}",
        ],
        "Planta exportada para {}" => [
            "Plan exported to {}",
            "Plano exportado a {}",
            "Plan exporté vers {}",
            "План експортовано в {}",
        ],
        "⚠ Imagem inválida: {}" => [
            "⚠ Invalid image: {}",
            "⚠ Imagen no válida: {}",
            "⚠ Image non valide : {}",
            "⚠ Недійсне зображення: {}",
        ],
        "Modificar {} paredes" => [
            "Modify {} walls",
            "Modificar {} paredes",
            "Modifier {} murs",
            "Змінити стіни ({})",
        ],
        "Modificar {}" => ["Modify {}", "Modificar {}", "Modifier {}", "Змінити {}"],
        "{} peças" => ["{} pieces", "{} piezas", "{} pièces", "Елементів: {}"],
        "Os pontos marcados estão a {} na escala atual." => [
            "The marked points are {} apart at the current scale.",
            "Los puntos marcados están a {} en la escala actual.",
            "Les points marqués sont distants de {} à l'échelle actuelle.",
            "Відстань між позначеними точками в поточному масштабі: {}.",
        ],
        "Fechar “{}”?" => [
            "Close “{}”?",
            "¿Cerrar «{}»?",
            "Fermer « {} » ?",
            "Закрити «{}»?",
        ],
        "Excluir “{}”?" => [
            "Delete “{}”?",
            "¿Eliminar «{}»?",
            "Supprimer « {} » ?",
            "Видалити «{}»?",
        ],
        "Renderizando… {} s" => [
            "Rendering… {} s",
            "Renderizando… {} s",
            "Rendu… {} s",
            "Рендеринг… {} с",
        ],
        "Pronta em {} s" => [
            "Ready in {} s",
            "Lista en {} s",
            "Prête en {} s",
            "Готово за {} с",
        ],
        "Foto salva em {}" => [
            "Photo saved to {}",
            "Foto guardada en {}",
            "Photo enregistrée dans {}",
            "Фото збережено в {}",
        ],
        "Mostrar {}" => ["Show {}", "Mostrar {}", "Afficher {}", "Показати {}"],
        "Tema" => ["Theme", "Tema", "Thème", "Тема"],
        "Do sistema" => [
            "From the system",
            "Del sistema",
            "Du système",
            "Як у системі",
        ],
        "Noite" => ["Night", "Noche", "Nuit", "Ніч"],
        "Dia" => ["Day", "Día", "Jour", "День"],
        "luz" => ["light", "luz", "lumière", "світло"],
        "Arraste ou dois dedos: girar · Shift: mover · Pinça ou scroll: zoom · F: enquadrar" => {
            [
                "Drag or two fingers: orbit · Shift: pan · Pinch or scroll: zoom · F: frame",
                "Arrastrar o dos dedos: girar · Mayús: mover · Pellizco o rueda: zoom · F: encuadrar",
                "Glisser ou deux doigts : pivoter · Maj : déplacer · Pincer ou molette : zoom · F : cadrer",
                "Перетягування або два пальці: обертати · Shift: зсув · Щипок або колесо: масштаб · F: вмістити",
            ]
        }
        "Aparência" => ["Look", "Aspecto", "Apparence", "Зовнішній вигляд"],
        // Every piece of the catalog: what the window calls it, and what it
        // is named when someone drops it on the plan.
        "Sofá 3 lugares" => [
            "3-seat sofa",
            "Sofá de 3 plazas",
            "Canapé 3 places",
            "Тримісний диван",
        ],
        "Sofá 2 lugares" => [
            "2-seat sofa",
            "Sofá de 2 plazas",
            "Canapé 2 places",
            "Двомісний диван",
        ],
        "Sofá em L (chaise)" => [
            "L-shaped sofa (chaise)",
            "Sofá en L (chaise longue)",
            "Canapé d'angle (méridienne)",
            "Кутовий диван (з оттоманкою)",
        ],
        "Poltrona" => ["Armchair", "Sillón", "Fauteuil", "Крісло"],
        "Poltrona Win" => ["Win armchair", "Sillón Win", "Fauteuil Win", "Крісло Win"],
        "Cadeira Ares" => ["Ares chair", "Silla Ares", "Chaise Ares", "Стілець Ares"],
        "Mesa de centro" => [
            "Coffee table",
            "Mesa de centro",
            "Table basse",
            "Журнальний столик",
        ],
        "Mesa lateral" => [
            "Side table",
            "Mesa auxiliar",
            "Table d'appoint",
            "Приставний столик",
        ],
        "Rack de TV" => [
            "TV unit",
            "Mueble de TV",
            "Meuble TV",
            "Тумба під телевізор",
        ],
        "Televisão 55\"" => [
            "55\" television",
            "Televisor de 55\"",
            "Téléviseur 55\"",
            "Телевізор 55\"",
        ],
        "Estante" => ["Shelving unit", "Estantería", "Étagère", "Стелаж"],
        "Tapete" => ["Rug", "Alfombra", "Tapis", "Килим"],
        "Radiador de painel 60×10×50" => [
            "Panel radiator 60×10×50",
            "Radiador de panel 60×10×50",
            "Radiateur à panneau 60×10×50",
            "Панельний радіатор 60×10×50",
        ],
        "Radiador de painel 100×10×50" => [
            "Panel radiator 100×10×50",
            "Radiador de panel 100×10×50",
            "Radiateur à panneau 100×10×50",
            "Панельний радіатор 100×10×50",
        ],
        "Radiador de painel 120×10×50" => [
            "Panel radiator 120×10×50",
            "Radiador de panel 120×10×50",
            "Radiateur à panneau 120×10×50",
            "Панельний радіатор 120×10×50",
        ],
        "Convector elétrico 100×10×45" => [
            "Electric convector 100×10×45",
            "Convector eléctrico 100×10×45",
            "Convecteur électrique 100×10×45",
            "Електричний конвектор 100×10×45",
        ],
        "Luminária de piso" => ["Floor lamp", "Lámpara de pie", "Lampadaire", "Торшер"],
        "Abajur de mesa" => [
            "Table lamp",
            "Lámpara de mesa",
            "Lampe de table",
            "Настільна лампа",
        ],
        "Spot embutido LED 7 W" => [
            "Recessed LED spot 7 W",
            "Foco LED empotrado 7 W",
            "Spot LED encastré 7 W",
            "Вбудований світлодіодний спот 7 Вт",
        ],
        "Pendente" => [
            "Pendant light",
            "Lámpara colgante",
            "Suspension",
            "Підвісний світильник",
        ],
        "Painel LED 60×60 36 W" => [
            "LED panel 60×60 36 W",
            "Panel LED 60×60 36 W",
            "Dalle LED 60×60 36 W",
            "Світлодіодна панель 60×60 36 Вт",
        ],
        "Fita LED (perfil)" => [
            "LED strip (profile)",
            "Tira LED (perfil)",
            "Ruban LED (profilé)",
            "Світлодіодна стрічка (у профілі)",
        ],
        "Mesa de jantar 4 lugares" => [
            "Dining table for 4",
            "Mesa de comedor de 4",
            "Table à manger 4 places",
            "Обідній стіл на 4 особи",
        ],
        "Mesa de jantar 6 lugares" => [
            "Dining table for 6",
            "Mesa de comedor de 6",
            "Table à manger 6 places",
            "Обідній стіл на 6 осіб",
        ],
        "Mesa com 4 cadeiras" => [
            "Table with 4 chairs",
            "Mesa con 4 sillas",
            "Table avec 4 chaises",
            "Стіл із 4 стільцями",
        ],
        "Mesa com 6 cadeiras" => [
            "Table with 6 chairs",
            "Mesa con 6 sillas",
            "Table avec 6 chaises",
            "Стіл із 6 стільцями",
        ],
        "Mesa redonda" => ["Round table", "Mesa redonda", "Table ronde", "Круглий стіл"],
        "Cadeira" => ["Chair", "Silla", "Chaise", "Стілець"],
        "Aparador" => ["Sideboard", "Aparador", "Buffet", "Буфет"],
        "Geladeira" => ["Fridge", "Frigorífico", "Réfrigérateur", "Холодильник"],
        "Fogão 4 bocas" => [
            "4-burner range",
            "Cocina de 4 fuegos",
            "Cuisinière 4 feux",
            "Плита на 4 конфорки",
        ],
        "Bancada com pia" => [
            "Countertop with sink",
            "Encimera con fregadero",
            "Plan de travail avec évier",
            "Стільниця з мийкою",
        ],
        "Armário de cozinha (baixo)" => [
            "Base cabinet",
            "Mueble bajo de cocina",
            "Meuble bas de cuisine",
            "Нижня шафа",
        ],
        "Armário aéreo" => ["Wall cabinet", "Mueble alto", "Meuble haut", "Верхня шафа"],
        "Coifa" => ["Range hood", "Campana extractora", "Hotte", "Витяжка"],
        "Lava-louças" => [
            "Dishwasher",
            "Lavavajillas",
            "Lave-vaisselle",
            "Посудомийна машина",
        ],
        "Micro-ondas" => [
            "Microwave",
            "Microondas",
            "Micro-ondes",
            "Мікрохвильова піч",
        ],
        "Cooktop 4 bocas (embutir)" => [
            "4-burner cooktop (built-in)",
            "Placa de 4 fuegos (encastrar)",
            "Table de cuisson 4 feux (encastrable)",
            "Варильна поверхня на 4 конфорки (вбудована)",
        ],
        "Cuba de inox (embutir)" => [
            "Stainless steel sink (built-in)",
            "Fregadero de acero inoxidable (encastrar)",
            "Évier inox (encastrable)",
            "Мийка з неіржавної сталі (вбудована)",
        ],
        "Forno de embutir" => [
            "Built-in oven",
            "Horno empotrable",
            "Four encastrable",
            "Вбудована духова шафа",
        ],
        "Ilha de cozinha" => [
            "Kitchen island",
            "Isla de cocina",
            "Îlot de cuisine",
            "Кухонний острів",
        ],
        "Banqueta alta" => [
            "Bar stool",
            "Taburete alto",
            "Tabouret de bar",
            "Барний стілець",
        ],
        "Cama de casal" => [
            "Double bed",
            "Cama de matrimonio",
            "Lit double",
            "Двоспальне ліжко",
        ],
        "Cama queen" => ["Queen bed", "Cama queen", "Lit queen", "Ліжко Queen"],
        "Cama king" => ["King bed", "Cama king", "Lit king", "Ліжко King"],
        "Cama de solteiro" => [
            "Single bed",
            "Cama individual",
            "Lit simple",
            "Односпальне ліжко",
        ],
        "Criado-mudo" => [
            "Bedside table",
            "Mesita de noche",
            "Table de chevet",
            "Приліжкова тумба",
        ],
        "Guarda-roupa" => ["Wardrobe", "Armario ropero", "Armoire", "Шафа для одягу"],
        "Cômoda" => ["Chest of drawers", "Cómoda", "Commode", "Комод"],
        "Berço" => ["Cot", "Cuna", "Lit bébé", "Дитяче ліжечко"],
        "Vaso sanitário" => ["Toilet", "Inodoro", "WC", "Унітаз"],
        "Gabinete com lavatório" => [
            "Vanity unit with basin",
            "Mueble con lavabo",
            "Meuble avec vasque",
            "Тумба з умивальником",
        ],
        "Box de chuveiro" => [
            "Shower tray",
            "Plato de ducha",
            "Receveur de douche",
            "Душовий піддон",
        ],
        "Box de vidro" => [
            "Glass shower screen",
            "Mampara de vidrio",
            "Paroi de douche en verre",
            "Скляна душова перегородка",
        ],
        "Banheira" => ["Bathtub", "Bañera", "Baignoire", "Ванна"],
        "Máquina de lavar" => [
            "Washing machine",
            "Lavadora",
            "Lave-linge",
            "Пральна машина",
        ],
        "Secadora" => ["Tumble dryer", "Secadora", "Sèche-linge", "Сушильна машина"],
        "Tanque" => [
            "Laundry sink",
            "Lavadero",
            "Bac à laver",
            "Господарська мийка",
        ],
        "Escrivaninha" => ["Desk", "Escritorio", "Bureau", "Письмовий стіл"],
        "Cadeira de escritório" => [
            "Office chair",
            "Silla de oficina",
            "Chaise de bureau",
            "Офісне крісло",
        ],
        "Porta" => ["Door", "Puerta", "Porte", "Двері"],
        "Porta dupla" => [
            "Double door",
            "Puerta doble",
            "Porte double",
            "Двостулкові двері",
        ],
        "Porta de correr" => [
            "Sliding door",
            "Puerta corredera",
            "Porte coulissante",
            "Розсувні двері",
        ],
        "Portão de garagem" => [
            "Garage door",
            "Puerta de garaje",
            "Porte de garage",
            "Гаражні ворота",
        ],
        "Vão livre" => ["Opening", "Vano libre", "Ouverture libre", "Отвір"],
        "Janela" => ["Window", "Ventana", "Fenêtre", "Вікно"],
        "Janela basculante" => [
            "Awning window",
            "Ventana abatible",
            "Fenêtre à soufflet",
            "Відкидне вікно",
        ],
        "Porta-janela" => [
            "French door",
            "Puerta ventana",
            "Porte-fenêtre",
            "Балконні двері",
        ],
        "Escada reta" => [
            "Straight stair",
            "Escalera recta",
            "Escalier droit",
            "Прямі сходи",
        ],
        "Pilar" => ["Column", "Pilar", "Poteau", "Колона"],
        "Coluna redonda" => [
            "Round column",
            "Columna redonda",
            "Colonne ronde",
            "Кругла колона",
        ],
        "Sapata" => ["Footing", "Zapata", "Semelle", "Фундаментна подушка"],
        "Viga / caibro" => [
            "Beam / rafter",
            "Viga / cabio",
            "Poutre / chevron",
            "Балка / кроква",
        ],
        "Painel" => ["Panel", "Panel", "Panneau", "Панель"],
        "Telha ondulada" => [
            "Corrugated sheet",
            "Chapa ondulada",
            "Tôle ondulée",
            "Хвилястий лист",
        ],
        "Gradil de ferro (guarda-corpo de barras verticais)" => [
            "Iron railing (vertical bar guard)",
            "Barandilla de hierro (barrotes verticales)",
            "Garde-corps en fer (barreaux verticaux)",
            "Залізне огородження (вертикальні прути)",
        ],
        "Guarda-corpo de vidro laminado com corrimão metálico" => [
            "Laminated glass guard with a metal handrail",
            "Barandilla de vidrio laminado con pasamanos metálico",
            "Garde-corps en verre feuilleté avec main courante métallique",
            "Огородження з багатошарового скла з металевим поручнем",
        ],
        "Fechamento de vidro da sacada (envidraçamento retrátil)" => [
            "Balcony glazing (retractable)",
            "Cerramiento de vidrio del balcón (acristalamiento retráctil)",
            "Fermeture vitrée du balcon (vitrage rétractable)",
            "Скління балкона (розсувне)",
        ],
        "Caixa" => ["Box", "Caja", "Caisse", "Ящик"],
        "Planta em vaso" => [
            "Potted plant",
            "Planta en maceta",
            "Plante en pot",
            "Рослина в горщику",
        ],
        "Floreira" => ["Planter", "Jardinera", "Jardinière", "Квітник"],
        "Muro / cerca" => [
            "Wall / fence",
            "Muro / valla",
            "Mur / clôture",
            "Мур / паркан",
        ],
        "Piscina" => ["Swimming pool", "Piscina", "Piscine", "Басейн"],
        "Piscina oval" => [
            "Oval pool",
            "Piscina ovalada",
            "Piscine ovale",
            "Овальний басейн",
        ],
        "Espreguiçadeira" => ["Sun lounger", "Tumbona", "Transat", "Шезлонг"],
        "Churrasqueira" => ["Barbecue", "Barbacoa", "Barbecue", "Мангал"],
        "Banco" => ["Bench", "Banco", "Banc", "Лавка"],
        "Árvore" => ["Tree", "Árbol", "Arbre", "Дерево"],
        "Carro" => ["Car", "Coche", "Voiture", "Автомобіль"],
        "Tomada baixa (30 cm)" => [
            "Low socket (30 cm)",
            "Enchufe bajo (30 cm)",
            "Prise basse (30 cm)",
            "Низька розетка (30 см)",
        ],
        "Tomada média (1,10 m)" => [
            "Mid socket (1.10 m)",
            "Enchufe medio (1,10 m)",
            "Prise à mi-hauteur (1,10 m)",
            "Середня розетка (1,10 м)",
        ],
        "Tomada alta (2,20 m)" => [
            "High socket (2.20 m)",
            "Enchufe alto (2,20 m)",
            "Prise haute (2,20 m)",
            "Висока розетка (2,20 м)",
        ],
        "Interruptor simples" => [
            "Single switch",
            "Interruptor simple",
            "Interrupteur simple",
            "Одноклавішний вимикач",
        ],
        "Interruptor duplo" => [
            "Double switch",
            "Interruptor doble",
            "Interrupteur double",
            "Двоклавішний вимикач",
        ],
        "Interruptor paralelo (three-way)" => [
            "Two-way switch (three-way)",
            "Conmutador (three-way)",
            "Va-et-vient (three-way)",
            "Прохідний вимикач",
        ],
        "Ponto de luz no teto" => [
            "Ceiling light point",
            "Punto de luz en el techo",
            "Point lumineux au plafond",
            "Точка освітлення на стелі",
        ],
        "Arandela (ponto de luz na parede)" => [
            "Wall light (light point on the wall)",
            "Aplique (punto de luz en la pared)",
            "Applique (point lumineux mural)",
            "Бра (точка освітлення на стіні)",
        ],
        "Quadro de distribuição" => [
            "Consumer unit",
            "Cuadro eléctrico",
            "Tableau électrique",
            "Розподільний щит",
        ],
        "Ponto de ar-condicionado" => [
            "Air conditioning point",
            "Punto de aire acondicionado",
            "Point de climatisation",
            "Точка для кондиціонера",
        ],
        "Ponto para chuveiro elétrico" => [
            "Electric shower point",
            "Punto para calentador de ducha",
            "Point pour chauffe-eau de douche",
            "Точка для електричного душу",
        ],
        "Tomada de dados / TV" => [
            "Data / TV socket",
            "Toma de datos / TV",
            "Prise données / TV",
            "Розетка даних / ТБ",
        ],
        "Ponto de rede RJ45 (Cat 6)" => [
            "RJ45 network point (Cat 6)",
            "Punto de red RJ45 (Cat 6)",
            "Prise réseau RJ45 (Cat 6)",
            "Мережева точка RJ45 (Cat 6)",
        ],
        "Ponto de TV (coaxial)" => [
            "TV point (coaxial)",
            "Punto de TV (coaxial)",
            "Prise TV (coaxiale)",
            "Точка ТБ (коаксіальна)",
        ],
        "Ponto de Wi-Fi no teto (access point)" => [
            "Ceiling Wi-Fi point (access point)",
            "Punto de Wi-Fi en el techo (punto de acceso)",
            "Point Wi-Fi au plafond (point d'accès)",
            "Точка Wi-Fi на стелі (точка доступу)",
        ],
        "Quadro de telecomunicações (rack / DG)" => [
            "Telecom panel (rack / MDF)",
            "Cuadro de telecomunicaciones (rack / RITI)",
            "Coffret de communication (baie / DTI)",
            "Телекомунікаційна шафа (стійка / кросовий щит)",
        ],
        "Campainha" => ["Doorbell", "Timbre", "Sonnette", "Дверний дзвінок"],
        "Torre de tomadas retrátil de embutir (furo 60 mm, 3 tomadas, plugue)" => [
            "Retractable socket tower, built in (60 mm hole, 3 sockets, plug)",
            "Torre de enchufes retráctil empotrable (agujero 60 mm, 3 enchufes, clavija)",
            "Bloc de prises escamotable encastré (perçage 60 mm, 3 prises, fiche)",
            "Висувний вбудований блок розеток (отвір 60 мм, 3 розетки, вилка)",
        ],
        "Torre de tomadas automática com indução (furo 85 mm, ligada à instalação)" => [
            "Automatic socket tower with induction (85 mm hole, wired in)",
            "Torre de enchufes automática con inducción (agujero 85 mm, conectada a la instalación)",
            "Bloc de prises automatique avec induction (perçage 85 mm, raccordé)",
            "Автоматичний блок розеток з індукцією (отвір 85 мм, стаціонарне підключення)",
        ],
        "Torre de 4 tomadas + USB e indução (furo 100 mm)" => [
            "4-socket tower + USB and induction (100 mm hole)",
            "Torre de 4 enchufes + USB e inducción (agujero 100 mm)",
            "Bloc 4 prises + USB et induction (perçage 100 mm)",
            "Блок на 4 розетки + USB та індукція (отвір 100 мм)",
        ],
        "Caixa de tomadas de mesa (4 tomadas, RJ45, USB, HDMI; recorte 120×335 mm)" => [
            "Desk socket box (4 sockets, RJ45, USB, HDMI; 120×335 mm cutout)",
            "Caja de enchufes de mesa (4 enchufes, RJ45, USB, HDMI; recorte 120×335 mm)",
            "Boîtier de prises de table (4 prises, RJ45, USB, HDMI ; découpe 120×335 mm)",
            "Настільний блок розеток (4 розетки, RJ45, USB, HDMI; виріз 120×335 мм)",
        ],
        "Tomada de embutir em móvel (furo 35 mm)" => [
            "Socket built into furniture (35 mm hole)",
            "Enchufe empotrado en el mueble (agujero 35 mm)",
            "Prise encastrée dans le meuble (perçage 35 mm)",
            "Розетка, вбудована в меблі (отвір 35 мм)",
        ],
        "Relé de automação (atrás do interruptor ou da luminária)" => [
            "Automation relay (behind the switch or the fixture)",
            "Relé de automatización (detrás del interruptor o de la luminaria)",
            "Relais domotique (derrière l'interrupteur ou le luminaire)",
            "Реле автоматизації (за вимикачем або світильником)",
        ],
        "Interruptor inteligente" => [
            "Smart switch",
            "Interruptor inteligente",
            "Interrupteur connecté",
            "Розумний вимикач",
        ],
        "Dimmer" => ["Dimmer", "Regulador", "Variateur", "Світлорегулятор"],
        "Sensor de presença de teto" => [
            "Ceiling presence sensor",
            "Sensor de presencia de techo",
            "Détecteur de présence au plafond",
            "Стельовий датчик присутності",
        ],
        "Fechadura eletrônica" => [
            "Electronic lock",
            "Cerradura electrónica",
            "Serrure électronique",
            "Електронний замок",
        ],
        "Ponto de água fria" => [
            "Cold water point",
            "Punto de agua fría",
            "Point d'eau froide",
            "Точка холодної води",
        ],
        "Ponto de água quente" => [
            "Hot water point",
            "Punto de agua caliente",
            "Point d'eau chaude",
            "Точка гарячої води",
        ],
        "Ponto de esgoto" => [
            "Drain point",
            "Punto de desagüe",
            "Point d'évacuation",
            "Точка каналізації",
        ],
        "Caixa sifonada 150×150×50 (ralo)" => [
            "Trapped gully 150×150×50 (drain)",
            "Sumidero sifónico 150×150×50",
            "Siphon de sol 150×150×50",
            "Сифонний трап 150×150×50 (злив)",
        ],
        "Caixa sifonada 100×150×50 (ralo)" => [
            "Trapped gully 100×150×50 (drain)",
            "Sumidero sifónico 100×150×50",
            "Siphon de sol 100×150×50",
            "Сифонний трап 100×150×50 (злив)",
        ],
        "Caixa sifonada 150×185×75 (ralo)" => [
            "Trapped gully 150×185×75 (drain)",
            "Sumidero sifónico 150×185×75",
            "Siphon de sol 150×185×75",
            "Сифонний трап 150×185×75 (злив)",
        ],
        "Ralo sifonado 100 mm (fecho hídrico curto)" => [
            "Trapped floor drain 100 mm (shallow seal)",
            "Sumidero sifónico 100 mm (cierre hídrico corto)",
            "Bonde siphoïde 100 mm (garde d'eau courte)",
            "Сифонний трап 100 мм (неглибокий гідрозатвор)",
        ],
        "Ralo seco 100 mm" => [
            "Dry floor drain 100 mm",
            "Sumidero seco 100 mm",
            "Bonde sèche 100 mm",
            "Сухий трап 100 мм",
        ],
        "Ralo linear 70 cm (sem sifão)" => [
            "Linear drain 70 cm (untrapped)",
            "Canaleta lineal 70 cm (sin sifón)",
            "Caniveau 70 cm (sans siphon)",
            "Лінійний трап 70 см (без сифона)",
        ],
        "Ralo linear sifonado 70 cm" => [
            "Trapped linear drain 70 cm",
            "Canaleta lineal sifónica 70 cm",
            "Caniveau siphoïde 70 cm",
            "Лінійний трап із сифоном 70 см",
        ],
        "Ralo de águas pluviais (varanda descoberta, terraço)" => [
            "Rainwater drain (open balcony, terrace)",
            "Sumidero de pluviales (balcón descubierto, terraza)",
            "Bonde d'eaux pluviales (balcon découvert, terrasse)",
            "Дощоприймач (відкритий балкон, тераса)",
        ],
        "Registro" => [
            "Stopcock",
            "Llave de paso",
            "Robinet d'arrêt",
            "Запірний кран",
        ],
        "Caixa de gordura" => [
            "Grease trap",
            "Trampa de grasas",
            "Bac à graisse",
            "Жировловлювач",
        ],
        "Grelha de ventilação permanente (gás)" => [
            "Permanent ventilation grille (gas)",
            "Rejilla de ventilación permanente (gas)",
            "Grille de ventilation permanente (gaz)",
            "Постійна вентиляційна решітка (газ)",
        ],
        "Tubo de ventilação" => [
            "Vent pipe",
            "Tubo de ventilación",
            "Colonne de ventilation",
            "Фанова труба",
        ],
        "Caixa de inspeção" => [
            "Inspection chamber",
            "Arqueta de inspección",
            "Regard de visite",
            "Оглядовий колодязь",
        ],
        "Hidrômetro" => [
            "Water meter",
            "Contador de agua",
            "Compteur d'eau",
            "Лічильник води",
        ],
        "Ponto de gás" => ["Gas point", "Punto de gas", "Point de gaz", "Точка газу"],
        // What walls are built of and what surfaces are finished with, as
        // the pickers list them.
        "Drywall 73 mm" => [
            "Drywall 73 mm",
            "Yeso laminado 73 mm",
            "Plaque de plâtre 73 mm",
            "Гіпсокартон 73 мм",
        ],
        "Drywall 95 mm" => [
            "Drywall 95 mm",
            "Yeso laminado 95 mm",
            "Plaque de plâtre 95 mm",
            "Гіпсокартон 95 мм",
        ],
        "Drywall 115 mm" => [
            "Drywall 115 mm",
            "Yeso laminado 115 mm",
            "Plaque de plâtre 115 mm",
            "Гіпсокартон 115 мм",
        ],
        "Drywall 120 mm chapa dupla" => [
            "Drywall 120 mm, double board",
            "Yeso laminado 120 mm, placa doble",
            "Plaque de plâtre 120 mm, double parement",
            "Гіпсокартон 120 мм, обшивка у два шари",
        ],
        "Tijolo cerâmico 9 cm" => [
            "Clay brick 9 cm",
            "Ladrillo cerámico 9 cm",
            "Brique en terre cuite 9 cm",
            "Керамічна цегла 9 см",
        ],
        "Tijolo cerâmico 14 cm" => [
            "Clay brick 14 cm",
            "Ladrillo cerámico 14 cm",
            "Brique en terre cuite 14 cm",
            "Керамічна цегла 14 см",
        ],
        "Tijolo cerâmico 19 cm" => [
            "Clay brick 19 cm",
            "Ladrillo cerámico 19 cm",
            "Brique en terre cuite 19 cm",
            "Керамічна цегла 19 см",
        ],
        "Bloco de concreto 14 cm" => [
            "Concrete block 14 cm",
            "Bloque de hormigón 14 cm",
            "Bloc de béton 14 cm",
            "Бетонний блок 14 см",
        ],
        "Bloco de concreto 19 cm" => [
            "Concrete block 19 cm",
            "Bloque de hormigón 19 cm",
            "Bloc de béton 19 cm",
            "Бетонний блок 19 см",
        ],
        "Parede de concreto 10 cm" => [
            "Concrete wall 10 cm",
            "Muro de hormigón 10 cm",
            "Mur en béton 10 cm",
            "Бетонна стіна 10 см",
        ],
        "Concreto armado 15 cm" => [
            "Reinforced concrete 15 cm",
            "Hormigón armado 15 cm",
            "Béton armé 15 cm",
            "Залізобетон 15 см",
        ],
        "Steel frame 14 cm" => [
            "Steel frame 14 cm",
            "Steel frame 14 cm",
            "Ossature métallique 14 cm",
            "Сталевий каркас 14 см",
        ],
        "Wood frame 10 cm" => [
            "Wood frame 10 cm",
            "Entramado de madera 10 cm",
            "Ossature bois 10 cm",
            "Дерев'яний каркас 10 см",
        ],
        "Divisória de vidro" => [
            "Glass partition",
            "Mampara de vidrio",
            "Cloison vitrée",
            "Скляна перегородка",
        ],
        "Montante 48 mm + 1 chapa de 12,5 mm em cada face" => [
            "48 mm stud + one 12.5 mm board each side",
            "Montante de 48 mm + 1 placa de 12,5 mm por cara",
            "Montant 48 mm + 1 plaque de 12,5 mm par face",
            "Стійка 48 мм + 1 лист 12,5 мм з кожного боку",
        ],
        "Montante 70 mm + 1 chapa de 12,5 mm em cada face" => [
            "70 mm stud + one 12.5 mm board each side",
            "Montante de 70 mm + 1 placa de 12,5 mm por cara",
            "Montant 70 mm + 1 plaque de 12,5 mm par face",
            "Стійка 70 мм + 1 лист 12,5 мм з кожного боку",
        ],
        "Montante 90 mm + 1 chapa de 12,5 mm em cada face" => [
            "90 mm stud + one 12.5 mm board each side",
            "Montante de 90 mm + 1 placa de 12,5 mm por cara",
            "Montant 90 mm + 1 plaque de 12,5 mm par face",
            "Стійка 90 мм + 1 лист 12,5 мм з кожного боку",
        ],
        "Montante 70 mm + 2 chapas de 12,5 mm em cada face (acústica)" => [
            "70 mm stud + two 12.5 mm boards each side (acoustic)",
            "Montante de 70 mm + 2 placas de 12,5 mm por cara (acústica)",
            "Montant 70 mm + 2 plaques de 12,5 mm par face (acoustique)",
            "Стійка 70 мм + 2 листи 12,5 мм з кожного боку (акустична)",
        ],
        "Bloco de 9 cm + reboco de 2,5 cm em cada face" => [
            "9 cm block + 2.5 cm render each side",
            "Bloque de 9 cm + enfoscado de 2,5 cm por cara",
            "Bloc de 9 cm + enduit de 2,5 cm par face",
            "Блок 9 см + штукатурка 2,5 см з кожного боку",
        ],
        "Bloco de 14 cm + reboco de 2,5 cm em cada face" => [
            "14 cm block + 2.5 cm render each side",
            "Bloque de 14 cm + enfoscado de 2,5 cm por cara",
            "Bloc de 14 cm + enduit de 2,5 cm par face",
            "Блок 14 см + штукатурка 2,5 см з кожного боку",
        ],
        "Bloco de 19 cm + reboco de 2,5 cm em cada face" => [
            "19 cm block + 2.5 cm render each side",
            "Bloque de 19 cm + enfoscado de 2,5 cm por cara",
            "Bloc de 19 cm + enduit de 2,5 cm par face",
            "Блок 19 см + штукатурка 2,5 см з кожного боку",
        ],
        "Bloco estrutural de 14 cm + revestimento de 1,5 cm em cada face" => [
            "14 cm structural block + 1.5 cm finish each side",
            "Bloque estructural de 14 cm + revestimiento de 1,5 cm por cara",
            "Bloc structurel de 14 cm + revêtement de 1,5 cm par face",
            "Конструкційний блок 14 см + оздоблення 1,5 см з кожного боку",
        ],
        "Bloco estrutural de 19 cm + revestimento de 1,5 cm em cada face" => [
            "19 cm structural block + 1.5 cm finish each side",
            "Bloque estructural de 19 cm + revestimiento de 1,5 cm por cara",
            "Bloc structurel de 19 cm + revêtement de 1,5 cm par face",
            "Конструкційний блок 19 см + оздоблення 1,5 см з кожного боку",
        ],
        "Concreto moldado in loco, sistema parede de concreto" => [
            "Cast-in-place concrete, concrete wall system",
            "Hormigón moldeado in situ, sistema de muro de hormigón",
            "Béton coulé en place, système de mur en béton",
            "Монолітний бетон, система бетонних стін",
        ],
        "Parede estrutural de concreto armado" => [
            "Structural reinforced concrete wall",
            "Muro estructural de hormigón armado",
            "Mur structurel en béton armé",
            "Несуча залізобетонна стіна",
        ],
        "Perfil 90 mm + OSB e placa cimentícia/gesso" => [
            "90 mm profile + OSB and cement/plaster board",
            "Perfil de 90 mm + OSB y placa de cemento/yeso",
            "Profilé 90 mm + OSB et plaque ciment/plâtre",
            "Профіль 90 мм + OSB і цементна/гіпсова плита",
        ],
        "Estrutura de madeira com fechamento em chapas" => [
            "Timber frame closed with boards",
            "Estructura de madera cerrada con placas",
            "Ossature bois fermée par des panneaux",
            "Дерев'яний каркас з обшивкою листами",
        ],
        "Vidro temperado de 10 mm" => [
            "10 mm toughened glass",
            "Vidrio templado de 10 mm",
            "Verre trempé de 10 mm",
            "Загартоване скло 10 мм",
        ],
        "Madeira (réguas)" => [
            "Wood (planks)",
            "Madera (tablas)",
            "Bois (lames)",
            "Дерево (дошки)",
        ],
        "Taco espinha de peixe" => [
            "Herringbone parquet",
            "Parqué en espiga",
            "Parquet à chevrons",
            "Паркет «ялинка»",
        ],
        "Porcelanato / cerâmica" => [
            "Porcelain / ceramic tile",
            "Porcelánico / cerámica",
            "Grès cérame / carrelage",
            "Керамограніт / керамічна плитка",
        ],
        "Azulejo metrô" => [
            "Subway tile",
            "Azulejo metro",
            "Carrelage métro",
            "Плитка «кабанчик»",
        ],
        "Tijolo aparente" => [
            "Exposed brick",
            "Ladrillo visto",
            "Brique apparente",
            "Відкрита цегла",
        ],
        "Pedra" => ["Stone", "Piedra", "Pierre", "Камінь"],
        "Cimento queimado" => [
            "Polished cement",
            "Cemento pulido",
            "Béton ciré",
            "Полірований бетон",
        ],
        "Mármore" => ["Marble", "Mármol", "Marbre", "Мармур"],
        "Carpete" => ["Carpet", "Moqueta", "Moquette", "Ковролін"],
        "Grama" => ["Grass", "Césped", "Gazon", "Трава"],
        "Água (piscina)" => [
            "Water (pool)",
            "Agua (piscina)",
            "Eau (piscine)",
            "Вода (басейн)",
        ],
        "Deck de madeira" => [
            "Wood deck",
            "Tarima de madera",
            "Terrasse en bois",
            "Дерев'яний настил",
        ],
        "Projeto recuperado desta sessão." => [
            "Project recovered from this session.",
            "Proyecto recuperado de esta sesión.",
            "Projet récupéré de cette session.",
            "Проєкт відновлено з цього сеансу.",
        ],
        "No terminal:" => [
            "In a terminal:",
            "En el terminal:",
            "Dans un terminal :",
            "У терміналі:",
        ],
        "Os instaladores já registram sozinhos se o Claude Code estiver instalado." => [
            "The installers register it for you when Claude Code is installed.",
            "Los instaladores lo registran solos si Claude Code está instalado.",
            "Les installateurs l'enregistrent tout seuls si Claude Code est installé.",
            "Інсталятори реєструють його самі, якщо встановлено Claude Code.",
        ],
        "Configurações → Desenvolvedor → Editar configuração" => [
            "Settings → Developer → Edit Config",
            "Configuración → Desarrollador → Editar configuración",
            "Réglages → Développeur → Modifier la configuration",
            "Налаштування → Розробник → Редагувати конфігурацію",
        ],
        "Precisa do Node.js instalado para a ponte mcp-remote. Reinicie o Claude Desktop." => [
            "Needs Node.js for the mcp-remote bridge. Restart Claude Desktop.",
            "Necesita Node.js para el puente mcp-remote. Reinicie Claude Desktop.",
            "Nécessite Node.js pour le pont mcp-remote. Redémarrez Claude Desktop.",
            "Для мосту mcp-remote потрібен встановлений Node.js. Перезапустіть Claude Desktop.",
        ],
        "Os instaladores já registram sozinhos se o Codex estiver instalado." => [
            "The installers register it for you when Codex is installed.",
            "Los instaladores lo registran solos si Codex está instalado.",
            "Les installateurs l'enregistrent tout seuls si Codex est installé.",
            "Інсталятори реєструють його самі, якщо встановлено Codex.",
        ],
        "Depois abra o Gemini CLI e peça o projeto." => [
            "Then open Gemini CLI and ask for your project.",
            "Después abra Gemini CLI y pídale el proyecto.",
            "Ouvrez ensuite Gemini CLI et demandez-lui votre projet.",
            "Потім відкрийте Gemini CLI і попросіть зробити щось у проєкті.",
        ],
        "Use no modo agente do Copilot." => [
            "Use it in Copilot's agent mode.",
            "Úselo en el modo agente de Copilot.",
            "À utiliser dans le mode agent de Copilot.",
            "Використовуйте в режимі агента Copilot.",
        ],
        "Arquivo ~/.cursor/mcp.json" => [
            "File ~/.cursor/mcp.json",
            "Archivo ~/.cursor/mcp.json",
            "Fichier ~/.cursor/mcp.json",
            "Файл ~/.cursor/mcp.json",
        ],
        "Reinicie o Cursor depois de salvar." => [
            "Restart Cursor after saving.",
            "Reinicie Cursor después de guardar.",
            "Redémarrez Cursor après avoir enregistré.",
            "Перезапустіть Cursor після збереження.",
        ],
        "Outro app" => [
            "Another app",
            "Otra app",
            "Une autre app",
            "Інший застосунок",
        ],
        "Nas configurações de MCP do app (Cline, Roo Code, Cherry Studio, LM Studio…)" => [
            "In the app's MCP settings (Cline, Roo Code, Cherry Studio, LM Studio…)",
            "En los ajustes de MCP de la app (Cline, Roo Code, Cherry Studio, LM Studio…)",
            "Dans les réglages MCP de l'app (Cline, Roo Code, Cherry Studio, LM Studio…)",
            "У налаштуваннях MCP застосунку (Cline, Roo Code, Cherry Studio, LM Studio…)",
        ],
        "O que importa é o app falar MCP: o modelo pode ser qualquer um." => [
            "What matters is that the app speaks MCP: the model can be any of them.",
            "Lo que importa es que la app hable MCP: el modelo puede ser cualquiera.",
            "Ce qui compte, c'est que l'app parle MCP : le modèle peut être n'importe lequel.",
            "Головне, щоб застосунок підтримував MCP: модель може бути будь-якою.",
        ],
        "agora" => ["just now", "ahora", "à l'instant", "щойно"],
        "há {} s" => ["{} s ago", "hace {} s", "il y a {} s", "{} с тому"],
        "há {} min" => ["{} min ago", "hace {} min", "il y a {} min", "{} хв тому"],
        "há {} h" => ["{} h ago", "hace {} h", "il y a {} h", "{} год тому"],
        "{} · {} chamadas" => [
            "{} · {} calls",
            "{} · {} llamadas",
            "{} · {} appels",
            "{} · викликів: {}",
        ],
        "MCP · esperando sua IA" => [
            "MCP · waiting for your AI",
            "MCP · esperando su IA",
            "MCP · en attente de votre IA",
            "MCP · чекає на ваш ШІ",
        ],
        "IA: só no aplicativo" => [
            "AI: in the app only",
            "IA: solo en la aplicación",
            "IA : seulement dans l'application",
            "ШІ: лише в застосунку",
        ],
        "Conectar sua IA — clique para ver como" => [
            "Connect your AI — click to see how",
            "Conecte su IA — haga clic para ver cómo",
            "Connectez votre IA — cliquez pour voir comment",
            "Підключіть свій ШІ — клацніть, щоб дізнатися як",
        ],
        "IA" => ["AI", "IA", "IA", "ШІ"],
        "{} conectado · {}" => [
            "{} connected · {}",
            "{} conectado · {}",
            "{} connecté · {}",
            "{} підключено · {}",
        ],
        "Nenhuma IA conectada ainda" => [
            "No AI connected yet",
            "Ninguna IA conectada todavía",
            "Aucune IA connectée pour l'instant",
            "Ще не підключено жодного ШІ",
        ],
        "Conectar sua IA…" => [
            "Connect your AI…",
            "Conecte su IA…",
            "Connecter votre IA…",
            "Підключити свій ШІ…",
        ],
        "Copiar o endereço do MCP" => [
            "Copy the MCP address",
            "Copiar la dirección del MCP",
            "Copier l'adresse MCP",
            "Копіювати адресу MCP",
        ],
        "Endereço copiado" => [
            "Address copied",
            "Dirección copiada",
            "Adresse copiée",
            "Адресу скопійовано",
        ],
        "Conectar sua IA" => [
            "Connect your AI",
            "Conecte su IA",
            "Connecter votre IA",
            "Підключіть свій ШІ",
        ],
        "O editor abre uma porta MCP: a sua IA desenha aqui dentro, em centímetros, e você vê acontecer." => {
            [
                "The editor opens an MCP port: your AI draws in here, in centimetres, and you watch it happen.",
                "El editor abre un puerto MCP: su IA dibuja aquí dentro, en centímetros, y usted lo ve ocurrir.",
                "L'éditeur ouvre un port MCP : votre IA dessine ici, en centimètres, et vous la regardez faire.",
                "Редактор відкриває порт MCP: ваш ШІ креслить тут, у сантиметрах, а ви бачите, як це відбувається.",
            ]
        }
        "Agora" => ["Right now", "Ahora mismo", "En ce moment", "Зараз"],
        "Servidor MCP ligado" => [
            "MCP server on",
            "Servidor MCP encendido",
            "Serveur MCP allumé",
            "Сервер MCP увімкнено",
        ],
        "No navegador o editor roda sozinho: o MCP vive no aplicativo do computador." => [
            "In a browser the editor runs on its own: the MCP lives in the desktop app.",
            "En el navegador el editor funciona solo: el MCP vive en la aplicación de escritorio.",
            "Dans un navigateur, l'éditeur tourne seul : le MCP vit dans l'application de bureau.",
            "У браузері редактор працює сам по собі: MCP є лише в настільному застосунку.",
        ],
        "O servidor está desligado (--no-server): reabra o aplicativo sem essa opção." => [
            "The server is off (--no-server): reopen the app without that option.",
            "El servidor está apagado (--no-server): vuelva a abrir la aplicación sin esa opción.",
            "Le serveur est éteint (--no-server) : rouvrez l'application sans cette option.",
            "Сервер вимкнено (--no-server): відкрийте застосунок знову без цього параметра.",
        ],
        "Nenhuma IA conectada ainda — siga os três passos abaixo." => [
            "No AI connected yet — the three steps below do it.",
            "Ninguna IA conectada todavía: los tres pasos de abajo lo resuelven.",
            "Aucune IA connectée — les trois étapes ci-dessous s'en chargent.",
            "Ще не підключено жодного ШІ — виконайте три кроки нижче.",
        ],
        "1. Escolha o aplicativo de IA que você usa" => [
            "1. Pick the AI app you use",
            "1. Elija la app de IA que usa",
            "1. Choisissez l'app d'IA que vous utilisez",
            "1. Виберіть застосунок ШІ, яким користуєтеся",
        ],
        "2. Cole isto onde ele pede" => [
            "2. Paste this where it asks",
            "2. Pegue esto donde lo pide",
            "2. Collez ceci là où il le demande",
            "2. Вставте це туди, де він просить",
        ],
        "3. Peça alguma coisa" => [
            "3. Ask it for something",
            "3. Pídale algo",
            "3. Demandez-lui quelque chose",
            "3. Попросіть його щось зробити",
        ],
        "Quantas paredes tem este projeto? Depois coloque uma janela de 120 cm na sala." => [
            "How many walls does this project have? Then put a 120 cm window in the living room.",
            "¿Cuántas paredes tiene este proyecto? Después ponga una ventana de 120 cm en el salón.",
            "Combien de murs a ce projet ? Ensuite, posez une fenêtre de 120 cm dans le salon.",
            "Скільки стін у цьому проєкті? Потім поставте вікно 120 см у вітальні.",
        ],
        "Copiado" => ["Copied", "Copiado", "Copié", "Скопійовано"],
        "Últimas chamadas" => [
            "Latest calls",
            "Últimas llamadas",
            "Derniers appels",
            "Останні виклики",
        ],
        "Nada ainda. Assim que sua IA usar uma ferramenta, ela aparece aqui." => [
            "Nothing yet. The moment your AI uses a tool, it shows up here.",
            "Nada todavía. En cuanto su IA use una herramienta, aparece aquí.",
            "Rien encore. Dès que votre IA utilise un outil, cela apparaît ici.",
            "Поки нічого. Щойно ваш ШІ скористається інструментом, виклик з'явиться тут.",
        ],
        "Baixar o aplicativo" => [
            "Download the app",
            "Descargar la aplicación",
            "Télécharger l'application",
            "Завантажити застосунок",
        ],
        "{} conectado · {} chamadas · {}" => [
            "{} connected · {} calls · {}",
            "{} conectado · {} llamadas · {}",
            "{} connecté · {} appels · {}",
            "{} підключено · викликів: {} · {}",
        ],
        "{} conectou — sua IA já pode desenhar aqui" => [
            "{} connected — your AI can draw in here now",
            "{} se conectó — su IA ya puede dibujar aquí",
            "{} s'est connecté — votre IA peut dessiner ici",
            "{} підключився — ваш ШІ вже може креслити тут",
        ],
        "Registrar agora" => [
            "Register now",
            "Registrar ahora",
            "Enregistrer maintenant",
            "Зареєструвати зараз",
        ],
        "Registrando…" => [
            "Registering…",
            "Registrando…",
            "Enregistrement…",
            "Реєстрація…",
        ],
        "Roda esse mesmo comando aqui, sem abrir o terminal." => [
            "Runs that same command from here, no terminal needed.",
            "Ejecuta ese mismo comando desde aquí, sin abrir el terminal.",
            "Exécute cette même commande d'ici, sans ouvrir de terminal.",
            "Виконує ту саму команду звідси, без термінала.",
        ],
        "Registrado no {} — agora é só pedir" => [
            "Registered with {} — now just ask",
            "Registrado en {} — ahora solo pida",
            "Enregistré dans {} — il n'y a plus qu'à demander",
            "Зареєстровано в {} — тепер просто попросіть",
        ],
        "Não deu para registrar: {}" => [
            "Could not register: {}",
            "No se pudo registrar: {}",
            "Impossible d'enregistrer : {}",
            "Не вдалося зареєструвати: {}",
        ],
        "Ligue e esta aba ganha um endereço que a sua IA alcança. O projeto não sai daqui: o servidor no meio só passa recados." => {
            [
                "Switch it on and this tab gets an address your AI can reach. The project stays here: the server in between only passes messages.",
                "Actívelo y esta pestaña obtiene una dirección a la que su IA llega. El proyecto no sale de aquí: el servidor del medio solo pasa mensajes.",
                "Activez-le et cet onglet reçoit une adresse que votre IA peut joindre. Le projet reste ici : le serveur au milieu ne fait que passer les messages.",
                "Увімкніть, і ця вкладка отримає адресу, до якої може дістатися ваш ШІ. Проєкт залишається тут: сервер посередині лише передає повідомлення.",
            ]
        }
        "Ligar o MCP nesta aba" => [
            "Switch the MCP on in this tab",
            "Activar el MCP en esta pestaña",
            "Activer le MCP dans cet onglet",
            "Увімкнути MCP у цій вкладці",
        ],
        "Ligando…" => ["Switching on…", "Activando…", "Activation…", "Вмикання…"],
        "Esta aba está no ar para a sua IA" => [
            "This tab is reachable by your AI",
            "Esta pestaña está al alcance de su IA",
            "Cet onglet est joignable par votre IA",
            "Ця вкладка доступна для вашого ШІ",
        ],
        "Quem tiver o endereço abaixo pode editar este projeto: trate como senha." => [
            "Anyone holding the address below can edit this project: treat it like a password.",
            "Quien tenga la dirección de abajo puede editar este proyecto: trátela como una contraseña.",
            "Quiconque détient l'adresse ci-dessous peut modifier ce projet : traitez-la comme un mot de passe.",
            "Будь-хто з адресою нижче може редагувати цей проєкт: бережіть її як пароль.",
        ],
        "Não deu para ligar: {}" => [
            "Could not switch it on: {}",
            "No se pudo activar: {}",
            "Impossible de l'activer : {}",
            "Не вдалося увімкнути: {}",
        ],
        "Tentar de novo" => [
            "Try again",
            "Intentar de nuevo",
            "Réessayer",
            "Спробувати ще раз",
        ],
        "{} só no aplicativo do computador: aqui ele renderiza na CPU e travaria a aba" => [
            "{} only in the desktop app: here it renders on the CPU and would freeze the tab",
            "{} solo en la aplicación de escritorio: aquí renderiza en la CPU y congelaría la pestaña",
            "{} seulement dans l'application de bureau : ici il rend sur le CPU et figerait l'onglet",
            "{} лише в настільному застосунку: тут рендеринг іде на CPU і завісить вкладку",
        ],
        "MCP desligado · ligar" => [
            "MCP off · switch on",
            "MCP apagado · activar",
            "MCP éteint · activer",
            "MCP вимкнено · увімкнути",
        ],
        "Desligar" => ["Switch off", "Apagar", "Éteindre", "Вимкнути"],
        "o relay recusou esta aba — ligue de novo" => [
            "the relay refused this tab — switch it on again",
            "el relay rechazó esta pestaña — actívelo de nuevo",
            "le relais a refusé cet onglet — rallumez-le",
            "ретранслятор відхилив цю вкладку — увімкніть знову",
        ],
        "Conectado como {}" => [
            "Signed in as {}",
            "Conectado como {}",
            "Connecté en tant que {}",
            "Ви ввійшли як {}",
        ],
        "Com este endereço, o Claude, o ChatGPT e outras IAs chegam nesta aba entrando com a sua conta." => {
            [
                "With this address, Claude, ChatGPT and other AIs reach this tab by signing in with your account.",
                "Con esta dirección, Claude, ChatGPT y otras IA llegan a esta pestaña iniciando sesión con su cuenta.",
                "Avec cette adresse, Claude, ChatGPT et d'autres IA atteignent cet onglet en se connectant avec votre compte.",
                "З цією адресою Claude, ChatGPT та інші ШІ потрапляють у цю вкладку, входячи у ваш обліковий запис.",
            ]
        }
        "Entrar para usar no Claude e no ChatGPT" => [
            "Sign in to use it in Claude and ChatGPT",
            "Iniciar sesión para usarlo en Claude y ChatGPT",
            "Se connecter pour l'utiliser dans Claude et ChatGPT",
            "Увійдіть, щоб користуватися в Claude і ChatGPT",
        ],
        "Com uma conta, a sua IA chega nesta aba por um endereço fixo, sem colar nada." => [
            "With an account, your AI reaches this tab at one fixed address, with nothing to paste.",
            "Con una cuenta, su IA llega a esta pestaña por una dirección fija, sin pegar nada.",
            "Avec un compte, votre IA atteint cet onglet à une adresse fixe, sans rien coller.",
            "З обліковим записом ваш ШІ дістається цієї вкладки за постійною адресою, нічого не вставляючи.",
        ],
        "Fecha o endereço. A sua IA perde o acesso na hora; o projeto continua aqui." => [
            "Closes the address. Your AI loses access at once; the project stays here.",
            "Cierra la dirección. Su IA pierde el acceso al instante; el proyecto sigue aquí.",
            "Ferme l'adresse. Votre IA perd l'accès aussitôt ; le projet reste ici.",
            "Закриває адресу. Ваш ШІ одразу втрачає доступ; проєкт залишається тут.",
        ],
        "Conectar sua IA (no navegador, liga e desliga)" => [
            "Connect your AI (in a browser, switches it on and off)",
            "Conecte su IA (en el navegador, activa y desactiva)",
            "Connecter votre IA (dans un navigateur, active et désactive)",
            "Підключити свій ШІ (у браузері вмикає й вимикає)",
        ],
        "3D" => ["3D", "3D", "3D", "3D"],
        "Catálogo · Planta · 3D (telas estreitas)" => [
            "Catalogue · Plan · 3D (narrow screens)",
            "Catálogo · Plano · 3D (pantallas estrechas)",
            "Catalogue · Plan · 3D (écrans étroits)",
            "Каталог · План · 3D (вузькі екрани)",
        ],
        "o servidor que leva a sua IA até esta aba não respondeu — no aplicativo do computador o MCP não precisa dele" => {
            [
                "the server that carries your AI to this tab did not answer — in the desktop app the MCP needs no server",
                "el servidor que lleva su IA hasta esta pestaña no respondió — en la aplicación de escritorio el MCP no lo necesita",
                "le serveur qui relie votre IA à cet onglet n'a pas répondu — dans l'application de bureau, le MCP n'en a pas besoin",
                "сервер, що з'єднує ваш ШІ з цією вкладкою, не відповів — у настільному застосунку MCP він не потрібен",
            ]
        }
        "Disciplina, camadas e andares" => [
            "Discipline, layers and storeys",
            "Disciplina, capas y plantas",
            "Discipline, calques et étages",
            "Розділ, шари та поверхи",
        ],
        "IA · esperando" => [
            "AI · waiting",
            "IA · esperando",
            "IA · en attente",
            "ШІ · очікування",
        ],
        "Conectar IA" => [
            "Connect AI",
            "Conectar IA",
            "Connecter l'IA",
            "Підключити ШІ",
        ],
        "{} conectado · {} chamadas" => [
            "{} connected · {} calls",
            "{} conectado · {} llamadas",
            "{} connecté · {} appels",
            "{} підключено · викликів: {}",
        ],
        "Esperando a sua IA chegar" => [
            "Waiting for your AI to arrive",
            "Esperando a que llegue su IA",
            "En attente de votre IA",
            "Чекаємо на ваш ШІ",
        ],
        "Ainda não dá para alcançar esta janela" => [
            "Nothing can reach this window yet",
            "Todavía nada puede alcanzar esta ventana",
            "Rien ne peut encore joindre cette fenêtre",
            "Поки ніщо не може дістатися цього вікна",
        ],
        "Escolha o aplicativo de IA que você usa" => [
            "Pick the AI app you use",
            "Elija la app de IA que usa",
            "Choisissez l'app d'IA que vous utilisez",
            "Виберіть застосунок ШІ, яким користуєтеся",
        ],
        "Cole isto onde ele pede" => [
            "Paste this where it asks",
            "Pegue esto donde lo pide",
            "Collez ceci là où il le demande",
            "Вставте це туди, де він просить",
        ],
        "Peça alguma coisa" => [
            "Ask it for something",
            "Pídale algo",
            "Demandez-lui quelque chose",
            "Попросіть його щось зробити",
        ],
        "o endereço guardado não vale mais — ligue de novo" => [
            "the address kept here is no longer yours — switch it on again",
            "la dirección guardada ya no es suya — actívela de nuevo",
            "l'adresse gardée ici n'est plus la vôtre — activez-la à nouveau",
            "збережена адреса більше не дійсна — увімкніть знову",
        ],
        "No ar, esperando a sua IA" => [
            "Up, waiting for your AI",
            "En el aire, esperando su IA",
            "En ligne, en attente de votre IA",
            "Онлайн, чекаємо на ваш ШІ",
        ],
        "Quem tiver esse endereço edita este projeto: trate como senha." => [
            "Anyone with that address edits this project: treat it like a password.",
            "Quien tenga esa dirección edita este proyecto: trátela como una contraseña.",
            "Quiconque a cette adresse modifie ce projet : traitez-la comme un mot de passe.",
            "Будь-хто з цією адресою редагує цей проєкт: бережіть її як пароль.",
        ],
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The names a new project starts with, and the catalog's, follow the
    /// window's language; one somebody typed does not. The English editor
    /// said "Versão 1", "Nova casa" and "Porta-janela".
    #[test]
    fn default_names_follow_the_language() {
        assert_eq!(shown_in("Versão 1", Lang::En), "Version 1");
        assert_eq!(shown_in("Versão 12", Lang::Es), "Versión 12");
        assert_eq!(shown_in("Versão 1 (cópia)", Lang::Fr), "Version 1 (copie)");
        assert_eq!(shown_in("Nova casa", Lang::En), "New home");
        assert_eq!(shown_in("Versão 2 (cópia)", Lang::Uk), "Версія 2 (копія)");
        assert_eq!(shown_in("Nova casa", Lang::Uk), "Новий будинок");
        assert_eq!(shown_in("Porta-janela", Lang::En), "French door");
        assert_eq!(
            shown_in("Cadeira Ares (cópia)", Lang::Es),
            "Silla Ares (copia)"
        );
        // Portuguese is what the file holds, and a typed name is left alone.
        assert_eq!(shown_in("Versão 1", Lang::Pt), "Versão 1");
        assert_eq!(shown_in("Bloom Coffee", Lang::En), "Bloom Coffee");
        assert_eq!(shown_in("Versão final", Lang::En), "Versão final");
    }

    /// Every literal handed to `tr` has every other language, or the window
    /// silently falls back to Portuguese for whoever picked one of them —
    /// which is exactly how a new panel ships half translated.
    #[test]
    fn every_string_on_screen_is_translated() {
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
        for needle in ["tr(", "fill("] {
            missing.extend(untranslated(&code, needle));
        }
        assert!(missing.is_empty(), "not translated: {missing:#?}");
    }

    /// Every literal handed to `call` — `tr(`, `fill(` — that no language
    /// speaks, or that some language leaves blank or without the `{}` the
    /// sentence needs.
    fn untranslated(code: &str, call: &str) -> Vec<String> {
        let mut missing: Vec<String> = Vec::new();
        let mut rest = code;
        while let Some(at) = rest.find(call) {
            // Match a function name, not the suffix of from_str or refill.
            let identifier_prefix = rest[..at]
                .chars()
                .next_back()
                .is_some_and(|c| c.is_alphanumeric() || c == '_');
            rest = &rest[at + call.len()..];
            if identifier_prefix {
                continue;
            }
            let Some(open) = rest.find('"') else { break };
            if rest[..open].contains(|c: char| !c.is_whitespace()) {
                continue;
            }
            let body = &rest[open + 1..];
            let Some(close) = body.find('"') else { break };
            let (text, after) = (&body[..close], body[close + 1..].trim_start());
            if after.starts_with(',') || after.starts_with(')') {
                // The test's own probe, and words spelled the same everywhere.
                if ["Algo novo", "Plugins", "cooktop"].contains(&text) {
                    continue;
                }
                // A blank side is worse than no row: the label disappears.
                // A side without the `{}` of the key drops what goes in it.
                let places = text.matches("{}").count();
                match translations(text) {
                    Some(row)
                        if row
                            .iter()
                            .all(|t| !t.trim().is_empty() && t.matches("{}").count() == places) => {
                    }
                    _ => missing.push(text.to_owned()),
                }
            }
        }
        missing
    }

    #[test]
    fn translation_scan_matches_function_names_not_identifier_suffixes() {
        let code = "JsValue::from_str(\"neweraOperation\"); refill(\"buffer\"); i18n::tr(\"Missing UI text\"); tr(\"Salvar\"); fill(\"Missing format {}\", &[]);";
        assert_eq!(untranslated(code, "tr("), vec!["Missing UI text"]);
        assert_eq!(untranslated(code, "fill("), vec!["Missing format {}"]);
    }

    /// These run beside the tests that drive whole windows, which read the
    /// labels in Portuguese — so none of them touch the language the window
    /// is set to, they ask for a language by name.
    #[test]
    fn translates_to_the_chosen_language() {
        for (lang, saved) in [
            (Lang::Pt, "Salvar"),
            (Lang::En, "Save"),
            (Lang::Es, "Guardar"),
            (Lang::Fr, "Enregistrer"),
            (Lang::Uk, "Зберегти"),
        ] {
            assert_eq!(say("Salvar", lang), saved);
            // A text nobody translated stays as written, in any language.
            assert_eq!(say("Algo novo", lang), "Algo novo");
        }
    }

    /// Ukrainian changes the counted word with the number; the others keep
    /// the plural the table gives.
    #[test]
    fn counts_agree_with_the_number() {
        for (n, said) in [
            (0, "0 стін"),
            (1, "1 стіна"),
            (2, "2 стіни"),
            (4, "4 стіни"),
            (5, "5 стін"),
            (11, "11 стін"),
            (12, "12 стін"),
            (21, "21 стіна"),
            (22, "22 стіни"),
            (111, "111 стін"),
            (114, "114 стін"),
        ] {
            assert_eq!(count_in(n, "paredes", Lang::Uk), said);
        }
        assert_eq!(count_in(1, "paredes", Lang::En), "1 walls");
        assert_eq!(count_in(3, "paredes", Lang::Pt), "3 paredes");
    }

    /// A word `count` knows the Ukrainian forms of is a word the table
    /// translates too, so the other languages never fall back to Portuguese.
    #[test]
    fn every_counted_word_is_translated() {
        for pt in [
            "paredes",
            "cômodos",
            "móveis",
            "peças",
            "quadros",
            "lugares para dormir",
            "banheiros",
        ] {
            assert!(ukrainian_forms(pt).is_some(), "{pt}");
            assert!(translations(pt).is_some(), "{pt}");
        }
    }

    #[test]
    fn the_window_keeps_the_language_it_is_set_to() {
        for lang in Lang::ALL {
            assert_eq!(Lang::from_code(lang.code()), lang);
        }
    }

    #[test]
    fn fills_the_sentence_in_the_chosen_language() {
        assert_eq!(
            fill_in("Aberto: {}", Lang::Fr, &[&"casa.n3d"]),
            "Ouvert : casa.n3d"
        );
        assert_eq!(
            fill_in(
                "⚠ Não foi possível abrir {}: {}",
                Lang::Fr,
                &[&"casa.n3d", &"nada aqui"]
            ),
            "⚠ Impossible d'ouvrir casa.n3d : nada aqui"
        );
        assert_eq!(
            fill_in("Aberto: {}", Lang::Pt, &[&"casa.n3d"]),
            "Aberto: casa.n3d"
        );
        // Nothing to put in it, and nothing lost either.
        assert_eq!(fill_in("Salvo em {}", Lang::Pt, &[]), "Salvo em {}");
    }

    /// Every piece in the catalog is named in every language. Change the
    /// language and the sofa has to stop being a sofá — the catalog is the
    /// first thing anyone reads in this window.
    #[test]
    fn every_piece_of_the_catalog_is_named_in_every_language() {
        let mut missing: Vec<&str> = Vec::new();
        for item in newera_catalog::CATALOG {
            match translations(item.name) {
                Some(row) if row.iter().all(|t| !t.trim().is_empty()) => {}
                _ => missing.push(item.name),
            }
        }
        assert!(missing.is_empty(), "not translated: {missing:#?}");
    }

    #[test]
    fn reads_the_language_the_system_asks_for() {
        assert_eq!(Lang::from_locale("pt_BR.UTF-8"), Some(Lang::Pt));
        assert_eq!(Lang::from_locale("en_US.UTF-8"), Some(Lang::En));
        assert_eq!(Lang::from_locale("es-419"), Some(Lang::Es));
        assert_eq!(Lang::from_locale("fr_CA"), Some(Lang::Fr));
        assert_eq!(Lang::from_locale("uk_UA.UTF-8"), Some(Lang::Uk));
        assert_eq!(Lang::from_locale("de_DE.UTF-8"), None);
        assert_eq!(Lang::from_locale("C"), None);
        assert_eq!(Lang::from_locale(""), None);
    }

    #[test]
    fn only_english_writes_decimals_with_a_dot() {
        for lang in Lang::ALL {
            assert_eq!(lang.decimal_comma(), lang != Lang::En);
        }
    }
}
