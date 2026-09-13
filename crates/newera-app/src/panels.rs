//! Left side panel: furniture catalog on top, home contents below.

use eframe::egui::{self, RichText};
use egui_phosphor::regular as icon;
use newera_core::ElementId;

use crate::app::NewEraApp;

/// Furniture categories. Items arrive with the furniture milestone; the tree
/// is here so the layout matches the target experience.
const CATALOG: &[(&str, &str, &[&str])] = &[
    (
        icon::COUCH,
        "Sala de estar",
        &["Sofá", "Poltrona", "Mesa de centro", "Estante"],
    ),
    (
        icon::OVEN,
        "Cozinha",
        &["Geladeira", "Fogão", "Pia", "Armário"],
    ),
    (
        icon::BED,
        "Quarto",
        &["Cama de casal", "Cama de solteiro", "Guarda-roupa"],
    ),
    (
        icon::BATHTUB,
        "Banheiro",
        &["Vaso sanitário", "Chuveiro", "Lavatório"],
    ),
    (
        icon::DOOR,
        "Portas e janelas",
        &["Porta", "Janela", "Porta de correr"],
    ),
];

pub(crate) fn left(app: &mut NewEraApp, ui: &mut egui::Ui) {
    egui::Panel::top("catalog")
        .resizable(true)
        .default_size(ui.available_height() * 0.45)
        .show(ui, |ui| {
            ui.add_space(4.0);
            ui.label(RichText::new(format!("{} Catálogo", icon::ARMCHAIR)).heading());
            egui::ScrollArea::vertical()
                .id_salt("catalog_scroll")
                .show(ui, |ui| {
                    for (glyph, category, items) in CATALOG {
                        egui::CollapsingHeader::new(format!("{glyph}  {category}")).show(
                            ui,
                            |ui| {
                                for item in *items {
                                    ui.add_enabled(false, egui::Label::new(*item))
                                        .on_disabled_hover_text("Móveis chegam no marco M2");
                                }
                            },
                        );
                    }
                });
        });

    egui::CentralPanel::default().show(ui, |ui| {
        let home = app.document.read().home().clone();
        ui.add_space(4.0);
        ui.label(RichText::new(format!("{} {}", icon::HOUSE_LINE, home.name)).heading());
        let total: f64 = home.rooms.iter().map(newera_core::Room::area).sum();
        ui.weak(format!(
            "{} paredes · {} cômodos · {}",
            home.walls.len(),
            home.rooms.len(),
            app.unit().format_area(total)
        ));
        ui.separator();

        let unit = app.unit();
        let mut clicked: Option<(ElementId, bool)> = None;
        let mut modify: Option<ElementId> = None;
        egui::ScrollArea::vertical().id_salt("home_scroll").show(ui, |ui| {
            let mut section = |ui: &mut egui::Ui, title: String, rows: Vec<(ElementId, String)>| {
                if rows.is_empty() {
                    return;
                }
                egui::CollapsingHeader::new(title).default_open(true).show(ui, |ui| {
                    for (id, text) in rows {
                        let selected = app.selection.contains(&id);
                        let response = ui.selectable_label(selected, text);
                        if response.clicked() {
                            clicked = Some((id, ui.input(|i| i.modifiers.command)));
                        }
                        if response.double_clicked() {
                            modify = Some(id);
                        }
                    }
                });
            };
            section(
                ui,
                format!("{} Cômodos ({})", icon::POLYGON, home.rooms.len()),
                home.rooms
                    .iter()
                    .map(|r| {
                        let name = if r.name.is_empty() { "Sem nome" } else { &r.name };
                        (r.id.into(), format!("{name} · {}", unit.format_area(r.area())))
                    })
                    .collect(),
            );
            section(
                ui,
                format!("{} Paredes ({})", icon::WALL, home.walls.len()),
                home.walls
                    .iter()
                    .map(|w| {
                        let arc = if w.is_arc() { " · arco" } else { "" };
                        (w.id.into(), format!("{} · {}{arc}", w.id, unit.format_length(w.length())))
                    })
                    .collect(),
            );
            section(
                ui,
                format!("{} Cotas ({})", icon::RULER, home.dimensions.len()),
                home.dimensions
                    .iter()
                    .map(|d| (d.id.into(), unit.format_length(d.length())))
                    .collect(),
            );
            section(
                ui,
                format!("{} Textos ({})", icon::TEXT_T, home.labels.len()),
                home.labels
                    .iter()
                    .map(|l| (l.id.into(), l.text.lines().next().unwrap_or_default().to_owned()))
                    .collect(),
            );
            if home.walls.is_empty() && home.rooms.is_empty() {
                ui.add_space(12.0);
                ui.weak("Comece desenhando paredes (W), importe uma planta como imagem de fundo, ou peça para a IA via MCP.");
            }
        });

        if let Some((id, additive)) = clicked {
            if !additive {
                app.selection.clear();
            }
            if !app.selection.remove(&id) || !additive {
                app.selection.insert(id);
            }
        }
        if let Some(id) = modify {
            app.open_modify(&[id]);
        }
    });
}
