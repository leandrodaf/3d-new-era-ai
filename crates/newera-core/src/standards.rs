//! The standards the software leans on, as data instead of prose.
//!
//! A rule that cites its source in the middle of a sentence cannot be
//! clicked, cannot say which edition it followed, and cannot tell how much
//! it matters. So every reference lives here once — code, title, edition,
//! reliability tier and link — and rules carry only the short [`Standard::code`].
//!
//! The tiers are a ladder from what obliges to what merely describes, and
//! they are what turns a reference into a severity: a Brazilian standard is
//! not the same kind of claim as a market survey, and treating them alike is
//! how measurements without an owner end up in software.
//!
//! Numbers that could not be checked against the primary publication are
//! marked [`Confidence::ConfirmBeforeUse`] and never raise a finding to an
//! error. Paid standards whose figures we do not hold are represented by
//! presence checks — the requirement exists — never by an invented number.

use serde::Serialize;

/// How much a source obliges.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub enum Tier {
    /// Brazilian standard or municipal code: breaking it is a legal or
    /// safety problem.
    A,
    /// Foreign standard or association guideline: good engineering, no legal
    /// force here.
    B,
    /// Architects and manufacturers: what makes a kitchen good, not what
    /// makes it legal.
    C,
    /// Laboratory research: empirical, with a published method.
    D,
    /// Market survey: what people do, never what they should do.
    E,
}

impl Tier {
    /// The letter used on the wire and in the interface.
    pub const fn letter(self) -> &'static str {
        match self {
            Self::A => "A",
            Self::B => "B",
            Self::C => "C",
            Self::D => "D",
            Self::E => "E",
        }
    }

    /// What this tier does, in one word.
    pub const fn what(self) -> &'static str {
        match self {
            Self::A => "obriga",
            Self::B => "referencia",
            Self::C => "doutrina",
            Self::D => "mede",
            Self::E => "descreve",
        }
    }
}

/// Whether the figures behind a reference were checked at the source.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum Confidence {
    /// Read in the primary publication, or in a free official version of it.
    Verified,
    /// Plausible and widely repeated, not confirmed at the source: it may
    /// advise, it may not accuse.
    ConfirmBeforeUse,
}

/// One source a rule can stand on.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct Standard {
    /// Short key carried by findings, e.g. `nbr13103`.
    pub code: &'static str,
    /// Full name as it should be cited, with the year.
    pub title: &'static str,
    /// Edition or year, alone, for the interface.
    pub edition: &'static str,
    pub tier: Tier,
    /// What it governs, in one line.
    pub scope: &'static str,
    /// A legitimate public version: official PDF, sector guide or booklet.
    pub url: Option<&'static str>,
    pub confidence: Confidence,
}

use Confidence::{ConfirmBeforeUse, Verified};

const fn s(
    code: &'static str,
    title: &'static str,
    edition: &'static str,
    tier: Tier,
    scope: &'static str,
    url: Option<&'static str>,
    confidence: Confidence,
) -> Standard {
    Standard {
        code,
        title,
        edition,
        tier,
        scope,
        url,
        confidence,
    }
}

/// Every source, grouped by tier.
pub static STANDARDS: &[Standard] = &[
    // ---- A: obliges ----
    s(
        "nbr15575g",
        "ABNT NBR 15575-1:2021 — Desempenho, Anexo G",
        "2021",
        Tier::A,
        "Mobiliário e equipamento mínimo por cômodo e a circulação em torno deles.",
        Some(
            "https://autogestao.unmp.org.br/wp-content/uploads/2014/09/Anexo-G_NBR-15575_1_Requisitos-Gerais.pdf",
        ),
        ConfirmBeforeUse,
    ),
    s(
        "nbr15575",
        "ABNT NBR 15575-1:2021 — Norma de Desempenho",
        "2021",
        Tier::A,
        "Pé-direito mínimo e desempenho dos sistemas de uma habitação.",
        Some("https://cbic.org.br/wp-content/uploads/2017/11/Guia_da_Norma_de_Desempenho_2013.pdf"),
        Verified,
    ),
    s(
        "nbr9050",
        "ABNT NBR 9050:2020 — Acessibilidade",
        "2020",
        Tier::A,
        "Área de aproximação e uso, alcances manuais, alturas de comando e giro de cadeira de rodas.",
        Some("https://www.confea.org.br/midias/acessibilidade_abnt_2022.pdf"),
        Verified,
    ),
    s(
        "nbr13103",
        "ABNT NBR 13103:2024 — Instalação de aparelhos a gás — Requisitos",
        "2024",
        Tier::A,
        "Ventilação permanente e exaustão onde há aparelho a gás, até 80 kW somados no mesmo local.",
        Some(
            "https://gasescombustiveis.com.br/seminario/159/palestras/ABNT%20NBR%2013103%20Bras%C3%ADlia%202024.pdf",
        ),
        Verified,
    ),
    s(
        "nbr5410",
        "ABNT NBR 5410 — Instalações elétricas de baixa tensão",
        "2004 (versão corrigida 2008)",
        Tier::A,
        "Em cozinhas e copas, um ponto de tomada a cada 3,5 m de perímetro e no mínimo dois acima da bancada.",
        Some("https://www.saladaeletrica.com.br/pontos-de-tomada-por-comodo/"),
        Verified,
    ),
    s(
        "nbr5626",
        "ABNT NBR 5626:2020 — Sistemas prediais de água fria e água quente — Projeto, execução, operação e manutenção",
        "2020",
        Tier::A,
        "Substitui a NBR 7198 (água quente): pressão estática até 400 kPa no ponto, dinâmica mínima de 10 kPa, registro antes dos sub-ramais de ao menos um ambiente sanitário.",
        None,
        Verified,
    ),
    s(
        "nbr8160",
        "ABNT NBR 8160:1999 — Sistemas prediais de esgoto sanitário — Projeto e execução",
        "1999",
        Tier::A,
        "Ramais de descarga por aparelho (tabela 3), caimento recomendado de 2 % até 75 mm e 1 % a partir de 100, ventilação (tabela 1), caixas sifonadas, de gordura e de inspeção.",
        None,
        Verified,
    ),
    s(
        "nbr14565",
        "ABNT NBR 14565 — Cabeamento estruturado para edifícios comerciais",
        "2025",
        Tier::A,
        "Cabeamento de telecomunicações de edifícios comerciais: enlace permanente até 90 m, canal até 100 m.",
        None,
        ConfirmBeforeUse,
    ),
    s(
        "nbr16264",
        "ABNT NBR 16264 — Cabeamento estruturado residencial",
        "2016 (confirmada em 2025)",
        Tier::A,
        "Tomadas de telecomunicações por cômodo (tabela 1), cabo de 4 pares até o distribuidor de residência, canal até 100 m, tomada de energia junto a cada ponto e ao distribuidor.",
        None,
        ConfirmBeforeUse,
    ),
    s(
        "nbr16415",
        "ABNT NBR 16415 — Caminhos e espaços para cabeamento estruturado",
        "2021",
        Tier::A,
        "Eletrodutos e caminhos dos cabos de telecomunicações, separados dos de energia.",
        None,
        ConfirmBeforeUse,
    ),
    s(
        "nbr5444",
        "ABNT NBR 5444 — Símbolos gráficos para instalações elétricas prediais",
        "1989 (cancelada em 2014, ainda a convenção de desenho)",
        Tier::C,
        "Símbolos dos pontos de luz, tomadas, interruptores e quadros na planta.",
        None,
        ConfirmBeforeUse,
    ),
    s(
        "nbr8995",
        "ABNT NBR ISO/CIE 8995-1:2013 — Iluminação de ambientes de trabalho",
        "2013",
        Tier::A,
        "Iluminância mínima em lux por atividade, de 50 lx na circulação a 2.000 lx em tarefa de detalhe.",
        None,
        Verified,
    ),
    s(
        "nbr16280",
        "ABNT NBR 16280 — Reforma em edificações",
        "2020",
        Tier::A,
        "Reforma em condomínio exige plano com cronograma, segurança e responsável técnico.",
        None,
        Verified,
    ),
    s(
        "nbr14037",
        "ABNT NBR 14037 e NBR 5674 — manual do proprietário e manutenção",
        "2011 / 2012",
        Tier::A,
        "O que a construtora entrega: manual de uso, sistemas instalados e programa de manutenção.",
        None,
        Verified,
    ),
    s(
        "nbr14810",
        "ABNT NBR 14810 — Painéis de partículas de média densidade (MDP)",
        "2018",
        Tier::A,
        "Define o MDP: painel de partículas com densidade entre 551 e 750 kg/m³, bom em arranque de parafuso.",
        Some("https://iba.org/psq-paineis"),
        Verified,
    ),
    s(
        "nbr15316",
        "ABNT NBR 15316 — Painéis de fibras de média densidade (MDF)",
        "2021",
        Tier::A,
        "Define o MDF: painel de fibras de processo seco, com usinabilidade de face e de topo.",
        None,
        Verified,
    ),
    s(
        "caixa-mcmv",
        "Caixa — Especificações Mínimas da Unidade Habitacional (MCMV)",
        "2023",
        Tier::A,
        "Cozinha de 1,80 m de largura mínima, com previsão de pia 120×50, fogão 55×60 e geladeira 70×70 cm.",
        Some("https://www.gov.br/cidades/pt-br"),
        ConfirmBeforeUse,
    ),
    s(
        "coe-municipal",
        "Código de obras municipal e Código Sanitário estadual",
        "varia por município",
        Tier::A,
        "Áreas, ventilação e o círculo inscrito no piso mudam de cidade; contra norma, prevalece o mais restritivo.",
        Some(
            "https://www.al.sp.gov.br/repositorio/legislacao/decreto/1978/decreto-12342-27.09.1978.html",
        ),
        Verified,
    ),
    s(
        "rdc216",
        "ANVISA RDC nº 216/2004 — serviços de alimentação",
        "2004",
        Tier::A,
        "Cozinha profissional: estrutura física, revestimentos, higienização, resíduos e pragas.",
        None,
        Verified,
    ),
    // ---- D: measures ----
    s(
        "ibge-adensamento",
        "IBGE — critério de adensamento excessivo",
        "atual",
        Tier::D,
        "Acima de três moradores por dormitório o domicílio é considerado adensado.",
        None,
        Verified,
    ),
    // ---- B: references ----
    s(
        "en1116",
        "EN 1116:2018 — Coordinating sizes for kitchen furniture and appliances",
        "2018",
        Tier::B,
        "Larguras nominais de 400, 500, 600 e 900 mm para módulos inferiores, e os nichos de embutir.",
        Some("https://standards.cencenelec.eu/dyn/www/f?p=CEN:110:0::::FSP_PROJECT:41638&cs=1"),
        Verified,
    ),
    s(
        "nkba",
        "NKBA Kitchen & Bath Planning Guidelines",
        "5ª edição",
        Tier::B,
        "31 diretrizes de cozinha, cada uma com projeto, exigência de código e Access Standards.",
        Some("https://nkba.org/professional-resources/kitchen-bath-planning-guidelines/"),
        ConfirmBeforeUse,
    ),
    s(
        "irc2024",
        "IRC 2024 / NEC 2023 — elétrica de cozinha (EUA)",
        "2024 / 2023",
        Tier::B,
        "Bancada a partir de 305 mm exige tomada, e ela não fica a mais de 20 in acima da bancada.",
        None,
        Verified,
    ),
    // ---- C: doctrine ----
    s(
        "alexander184",
        "Christopher Alexander — A Pattern Language, padrão 184 «Cooking Layout»",
        "1977",
        Tier::C,
        "Bancada total ≥ 366 cm fora de pia, fogão e geladeira; nenhum trecho < 122 cm; nenhum par > 305 cm.",
        None,
        Verified,
    ),
    s(
        "blum-zonas",
        "Blum e Hettich — as cinco zonas de trabalho",
        "atual",
        Tier::C,
        "Mantimentos, armazenagem, lavagem, preparo e cocção na ordem do fluxo; bancada 15–20 cm abaixo do cotovelo.",
        Some("https://www.blum.com/br/pt/company/dynamic-space/"),
        Verified,
    ),
    s(
        "gilbreth-triangulo",
        "Lillian Gilbreth e o Small Homes Council — o triângulo de trabalho",
        "1929 / anos 1940",
        Tier::C,
        "Pia, fogão e geladeira próximos; calibrado para cozinha de uma pessoa, sem micro-ondas nem lava-louças.",
        None,
        Verified,
    ),
    s(
        "bulthaup-b1",
        "bulthaup b1 e b3 — a cozinha em três elementos",
        "atual",
        Tier::C,
        "Ilha, linha de parede e bloco de torres: uma tipologia limpa de layout. Material de marca, sem método publicado.",
        None,
        ConfirmBeforeUse,
    ),
    s(
        "neufert",
        "Ernst Neufert — A Arte de Projetar em Arquitetura",
        "desde 1936",
        Tier::C,
        "Origem da bancada de referência de 90 cm de altura por 60 cm de profundidade.",
        None,
        Verified,
    ),
    s(
        "panero-zelnik",
        "Panero & Zelnik — Dimensionamento Humano para Espaços Interiores",
        "1979",
        Tier::C,
        "Alcances, folgas e zonas de trabalho derivados de percentis populacionais.",
        None,
        Verified,
    ),
    // ---- D: measures ----
    s(
        "lbnl-coifa",
        "Lawrence Berkeley National Laboratory — eficiência de captura de coifas",
        "2013",
        Tier::D,
        "Captura de 15 % a 98 % entre modelos, sem relação com preço; ~80 % nas bocas traseiras contra ~50 % nas frontais.",
        Some("https://newscenter.lbl.gov/2013/09/24/cooking-up-cleaner-air/"),
        Verified,
    ),
    // ---- E: describes ----
    s(
        "houzz2026",
        "2026 U.S. Houzz Kitchen Trends Study",
        "2026",
        Tier::E,
        "1.780 respondentes americanos em reforma de cozinha, campo em julho de 2025. Não transferível ao Brasil.",
        Some(
            "https://www.houzz.com/magazine/2026-u-s-houzz-kitchen-trends-study-stsetivw-vs~137665428",
        ),
        Verified,
    ),
    s(
        "abimovel",
        "Abimóvel — Anuário Brasil Móveis",
        "2024",
        Tier::E,
        "A indústria moveleira nacional: 22 mil empresas e receita acima de R$ 91,5 bilhões em 2024.",
        None,
        Verified,
    ),
];

/// The source behind `code`, if it is one we hold.
pub fn standard(code: &str) -> Option<&'static Standard> {
    STANDARDS.iter().find(|s| s.code == code)
}

/// What a municipal building code demands of a kitchen, where we hold it.
///
/// These numbers change from city to city and, against a standard, the more
/// restrictive one wins — so they are never constants inside a rule. A city
/// we do not hold turns every municipal finding into advice to confirm.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct MunicipalCode {
    /// Slug used in tool arguments, e.g. `sao-paulo`.
    pub city: &'static str,
    /// Name to show, e.g. `São Paulo — SP`.
    pub label: &'static str,
    /// Where the figures come from.
    pub source: &'static str,
    /// Circle that must fit on the kitchen floor, cm.
    pub kitchen_circle_cm: Option<f64>,
}

/// Cities whose code we hold.
pub static MUNICIPAL_CODES: &[MunicipalCode] = &[
    MunicipalCode {
        city: "sao-paulo",
        label: "São Paulo — SP",
        source: "COE-SP (Lei 16.642/2017) e Código Sanitário estadual (Decreto 12.342/1978)",
        kitchen_circle_cm: Some(120.0),
    },
    MunicipalCode {
        city: "estado-sp",
        label: "Estado de São Paulo (Código Sanitário)",
        source: "Decreto estadual 12.342/1978",
        kitchen_circle_cm: Some(120.0),
    },
];

/// The code of `city`, if we hold it. Accepts the slug or the label.
pub fn municipal(city: &str) -> Option<&'static MunicipalCode> {
    let want = city.trim();
    MUNICIPAL_CODES
        .iter()
        .find(|m| m.city.eq_ignore_ascii_case(want) || m.label.eq_ignore_ascii_case(want))
}

/// Every city we hold, for a picker.
pub const fn cities() -> &'static [MunicipalCode] {
    MUNICIPAL_CODES
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codes_are_unique_and_resolvable() {
        for s in STANDARDS {
            assert_eq!(standard(s.code), Some(s), "{} resolves to itself", s.code);
            assert_eq!(
                STANDARDS.iter().filter(|o| o.code == s.code).count(),
                1,
                "{} appears once",
                s.code
            );
            assert!(!s.title.is_empty() && !s.edition.is_empty(), "{}", s.code);
        }
        assert!(standard("inventada").is_none());
    }

    #[test]
    fn paid_standards_we_did_not_read_ask_to_be_confirmed() {
        // The dossier's own rule: a figure we could not check at the source
        // may advise, never accuse.
        for code in ["nbr15575g", "caixa-mcmv", "nkba", "bulthaup-b1"] {
            assert_eq!(
                standard(code).unwrap().confidence,
                Confidence::ConfirmBeforeUse,
                "{code}"
            );
        }
    }

    #[test]
    fn cities_are_found_by_slug_or_label() {
        let sp = municipal("sao-paulo").unwrap();
        assert_eq!(sp.kitchen_circle_cm, Some(120.0));
        assert_eq!(municipal("São Paulo — SP"), Some(sp));
        assert!(municipal("atlantis").is_none());
        assert!(!cities().is_empty());
    }
}
