//! Surface finishes (paint, floors, tiles…) and standard wall types.
//!
//! A [`Material`] is a color, an optional procedural [`Pattern`] or an image
//! file, tiled at a real size in centimeters so a 60×60 cm porcelain tile
//! looks 60×60 cm in 3D. Materials have a short text form used by the MCP:
//!
//! ```text
//! #f2efe6              plain paint
//! wood                 pattern with its default color and size
//! tiles #ffffff 60x60  tinted, tile size in cm
//! parquet r45          rotated 45°
//! img:tex/piso.jpg 90x90
//! ```

use std::fmt;
use std::str::FromStr;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Built-in procedural patterns, drawn by the renderer at any resolution.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum Pattern {
    Wood,
    Parquet,
    Tiles,
    Subway,
    Brick,
    Stone,
    Concrete,
    Marble,
    Carpet,
    Grass,
}

impl Pattern {
    pub const ALL: [Self; 10] = [
        Self::Wood,
        Self::Parquet,
        Self::Tiles,
        Self::Subway,
        Self::Brick,
        Self::Stone,
        Self::Concrete,
        Self::Marble,
        Self::Carpet,
        Self::Grass,
    ];

    pub fn key(self) -> &'static str {
        match self {
            Self::Wood => "wood",
            Self::Parquet => "parquet",
            Self::Tiles => "tiles",
            Self::Subway => "subway",
            Self::Brick => "brick",
            Self::Stone => "stone",
            Self::Concrete => "concrete",
            Self::Marble => "marble",
            Self::Carpet => "carpet",
            Self::Grass => "grass",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Wood => "Madeira (réguas)",
            Self::Parquet => "Taco espinha de peixe",
            Self::Tiles => "Porcelanato / cerâmica",
            Self::Subway => "Azulejo metrô",
            Self::Brick => "Tijolo aparente",
            Self::Stone => "Pedra",
            Self::Concrete => "Cimento queimado",
            Self::Marble => "Mármore",
            Self::Carpet => "Carpete",
            Self::Grass => "Grama",
        }
    }

    /// Typical color, `[r, g, b]`.
    pub fn default_color(self) -> [u8; 3] {
        match self {
            Self::Wood => [176, 128, 84],
            Self::Parquet => [160, 112, 70],
            Self::Tiles => [226, 222, 214],
            Self::Subway => [244, 244, 240],
            Self::Brick => [168, 84, 58],
            Self::Stone => [150, 146, 138],
            Self::Concrete => [158, 158, 152],
            Self::Marble => [236, 234, 230],
            Self::Carpet => [120, 110, 100],
            Self::Grass => [96, 140, 70],
        }
    }

    /// Typical module size `[w, h]` in cm (one plank, tile, brick course…).
    pub fn default_tile(self) -> [f64; 2] {
        match self {
            Self::Wood => [120.0, 20.0],
            Self::Parquet | Self::Tiles => [60.0, 60.0],
            Self::Subway => [20.0, 10.0],
            Self::Brick => [24.0, 7.0],
            Self::Stone => [80.0, 80.0],
            Self::Concrete | Self::Carpet | Self::Grass => [200.0, 200.0],
            Self::Marble => [120.0, 120.0],
        }
    }

    /// Index used by the renderers (1-based; 0 is plain color).
    pub fn index(self) -> u32 {
        let position = Self::ALL.iter().position(|p| *p == self).unwrap_or(0);
        u32::try_from(position).unwrap_or(0) + 1
    }
}

impl FromStr for Pattern {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::ALL.into_iter().find(|p| p.key() == s).ok_or_else(|| {
            let keys: Vec<&str> = Self::ALL.iter().map(|p| p.key()).collect();
            format!("unknown pattern `{s}` (use {})", keys.join(", "))
        })
    }
}

#[allow(clippy::trivially_copy_pass_by_ref)] // serde's skip_serializing_if signature
fn is_zero(value: &f64) -> bool {
    *value == 0.0
}

/// A surface finish.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Material {
    /// Color, or the tint applied to the pattern/image.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<[u8; 3]>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pattern: Option<Pattern>,
    /// Image file, relative to the project file or absolute.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub image: Option<String>,
    /// Size of one repetition `[w, h]` in cm.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tile: Option<[f64; 2]>,
    /// Rotation of the pattern, degrees.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub angle: f64,
}

impl Material {
    pub fn paint(color: [u8; 3]) -> Self {
        Self {
            color: Some(color),
            ..Self::default()
        }
    }

    pub fn pattern(pattern: Pattern) -> Self {
        Self {
            pattern: Some(pattern),
            ..Self::default()
        }
    }

    /// Color to show when patterns can't be drawn (plan, thumbnails).
    pub fn base_color(&self, fallback: [u8; 3]) -> [u8; 3] {
        self.color
            .or_else(|| self.pattern.map(Pattern::default_color))
            .unwrap_or(fallback)
    }

    /// Tile size in cm; images default to one meter.
    pub fn tile_size(&self) -> [f64; 2] {
        self.tile
            .unwrap_or_else(|| self.pattern.map_or([100.0, 100.0], Pattern::default_tile))
    }

    pub fn validate(&self) -> Result<(), String> {
        if let Some([w, h]) = self.tile
            && !(w.is_finite() && h.is_finite() && w > 0.0 && h > 0.0)
        {
            return Err("material tile size must be positive".into());
        }
        if !self.angle.is_finite() {
            return Err("material angle must be finite".into());
        }
        Ok(())
    }
}

fn parse_hex(token: &str) -> Option<[u8; 3]> {
    let hex = token.strip_prefix('#')?;
    if hex.len() != 6 {
        return None;
    }
    let byte = |i: usize| u8::from_str_radix(&hex[i..i + 2], 16).ok();
    Some([byte(0)?, byte(2)?, byte(4)?])
}

fn parse_size(token: &str) -> Option<[f64; 2]> {
    let (w, h) = token.split_once('x').unwrap_or((token, token));
    let (w, h): (f64, f64) = (w.parse().ok()?, h.parse().ok()?);
    (w > 0.0 && h > 0.0).then_some([w, h])
}

impl FromStr for Material {
    type Err = String;

    /// Parses the short text form (see the module docs). Trailing tokens are
    /// read as color, size and rotation; what is left is the pattern or image.
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut tokens: Vec<&str> = s.split_whitespace().collect();
        let mut material = Self::default();
        while let Some(last) = tokens.last().copied() {
            if let Some(color) = parse_hex(last) {
                material.color = Some(color);
            } else if let Some(angle) = last.strip_prefix('r').and_then(|a| a.parse().ok()) {
                material.angle = angle;
            } else if last.starts_with(|c: char| c.is_ascii_digit())
                && let Some(size) = parse_size(last)
            {
                material.tile = Some(size);
            } else {
                break;
            }
            tokens.pop();
        }
        let kind = tokens.join(" ");
        if let Some(path) = kind.strip_prefix("img:") {
            if path.is_empty() {
                return Err("`img:` needs a file path".into());
            }
            material.image = Some(path.to_owned());
        } else if !kind.is_empty() {
            material.pattern = Some(kind.parse()?);
        }
        if material == Self::default() {
            return Err(format!("empty material `{s}`"));
        }
        material.validate()?;
        Ok(material)
    }
}

fn trim_number(value: f64) -> String {
    let rounded = (value * 100.0).round() / 100.0;
    format!("{rounded}")
}

impl fmt::Display for Material {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut parts = Vec::new();
        if let Some(image) = &self.image {
            parts.push(format!("img:{image}"));
        } else if let Some(pattern) = self.pattern {
            parts.push(pattern.key().to_owned());
        }
        if let Some([r, g, b]) = self.color {
            parts.push(format!("#{r:02x}{g:02x}{b:02x}"));
        }
        if let Some([w, h]) = self.tile {
            if (w - h).abs() < f64::EPSILON {
                parts.push(trim_number(w));
            } else {
                parts.push(format!("{}x{}", trim_number(w), trim_number(h)));
            }
        }
        if self.angle != 0.0 {
            parts.push(format!("r{}", trim_number(self.angle)));
        }
        f.write_str(&parts.join(" "))
    }
}

/// Family of a wall construction, for plan hatching and 3D defaults.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum WallFamily {
    Drywall,
    Masonry,
    Concrete,
    Glass,
    Wood,
}

/// A standard wall construction with its finished thickness.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WallType {
    pub id: &'static str,
    pub name: &'static str,
    pub family: WallFamily,
    /// Finished thickness, cm (core plus finishes on both sides).
    pub thickness: f64,
    pub description: &'static str,
}

/// Common Brazilian wall constructions.
pub const WALL_TYPES: &[WallType] = &[
    WallType {
        id: "drywall-73",
        name: "Drywall 73 mm",
        family: WallFamily::Drywall,
        thickness: 7.3,
        description: "Montante 48 mm + 1 chapa de 12,5 mm em cada face",
    },
    WallType {
        id: "drywall-95",
        name: "Drywall 95 mm",
        family: WallFamily::Drywall,
        thickness: 9.5,
        description: "Montante 70 mm + 1 chapa de 12,5 mm em cada face",
    },
    WallType {
        id: "drywall-115",
        name: "Drywall 115 mm",
        family: WallFamily::Drywall,
        thickness: 11.5,
        description: "Montante 90 mm + 1 chapa de 12,5 mm em cada face",
    },
    WallType {
        id: "drywall-120-duplo",
        name: "Drywall 120 mm chapa dupla",
        family: WallFamily::Drywall,
        thickness: 12.0,
        description: "Montante 70 mm + 2 chapas de 12,5 mm em cada face (acústica)",
    },
    WallType {
        id: "tijolo-9",
        name: "Tijolo cerâmico 9 cm",
        family: WallFamily::Masonry,
        thickness: 14.0,
        description: "Bloco de 9 cm + reboco de 2,5 cm em cada face",
    },
    WallType {
        id: "tijolo-14",
        name: "Tijolo cerâmico 14 cm",
        family: WallFamily::Masonry,
        thickness: 19.0,
        description: "Bloco de 14 cm + reboco de 2,5 cm em cada face",
    },
    WallType {
        id: "tijolo-19",
        name: "Tijolo cerâmico 19 cm",
        family: WallFamily::Masonry,
        thickness: 24.0,
        description: "Bloco de 19 cm + reboco de 2,5 cm em cada face",
    },
    WallType {
        id: "bloco-concreto-14",
        name: "Bloco de concreto 14 cm",
        family: WallFamily::Masonry,
        thickness: 17.0,
        description: "Bloco estrutural de 14 cm + revestimento de 1,5 cm em cada face",
    },
    WallType {
        id: "bloco-concreto-19",
        name: "Bloco de concreto 19 cm",
        family: WallFamily::Masonry,
        thickness: 22.0,
        description: "Bloco estrutural de 19 cm + revestimento de 1,5 cm em cada face",
    },
    WallType {
        id: "concreto-10",
        name: "Parede de concreto 10 cm",
        family: WallFamily::Concrete,
        thickness: 10.0,
        description: "Concreto moldado in loco, sistema parede de concreto",
    },
    WallType {
        id: "concreto-15",
        name: "Concreto armado 15 cm",
        family: WallFamily::Concrete,
        thickness: 15.0,
        description: "Parede estrutural de concreto armado",
    },
    WallType {
        id: "steel-frame-14",
        name: "Steel frame 14 cm",
        family: WallFamily::Drywall,
        thickness: 14.0,
        description: "Perfil 90 mm + OSB e placa cimentícia/gesso",
    },
    WallType {
        id: "madeira-10",
        name: "Wood frame 10 cm",
        family: WallFamily::Wood,
        thickness: 10.0,
        description: "Estrutura de madeira com fechamento em chapas",
    },
    WallType {
        id: "vidro-10",
        name: "Divisória de vidro",
        family: WallFamily::Glass,
        thickness: 1.0,
        description: "Vidro temperado de 10 mm",
    },
];

pub fn wall_type(id: &str) -> Option<&'static WallType> {
    WALL_TYPES.iter().find(|t| t.id == id)
}

#[cfg(test)]
#[allow(clippy::float_cmp)] // exact constants
mod tests {
    use super::*;

    #[test]
    fn short_form_round_trips() {
        for text in [
            "#f2efe6",
            "wood",
            "tiles #ffffff 60",
            "subway 20x10",
            "parquet r45",
            "img:tex/piso claro.jpg 90x45 r30",
        ] {
            let material: Material = text.parse().unwrap();
            assert_eq!(material.to_string(), text);
        }
    }

    #[test]
    fn short_form_rejects_nonsense() {
        assert!("".parse::<Material>().is_err());
        assert!("velvet".parse::<Material>().is_err());
        assert!("img:".parse::<Material>().is_err());
        assert!("tiles 0x10".parse::<Material>().is_err());
    }

    #[test]
    fn defaults_come_from_the_pattern() {
        let wood = Material::pattern(Pattern::Wood);
        assert_eq!(wood.tile_size(), [120.0, 20.0]);
        assert_eq!(wood.base_color([0, 0, 0]), Pattern::Wood.default_color());
        assert_eq!(Material::default().base_color([1, 2, 3]), [1, 2, 3]);
        assert_eq!(Pattern::Wood.index(), 1);
    }

    #[test]
    fn wall_types_are_unique_and_sane() {
        for (i, t) in WALL_TYPES.iter().enumerate() {
            assert!(t.thickness > 0.0 && t.thickness < 60.0, "{}", t.id);
            assert!(WALL_TYPES[i + 1..].iter().all(|o| o.id != t.id), "{}", t.id);
        }
        assert_eq!(wall_type("drywall-95").unwrap().thickness, 9.5);
    }
}
