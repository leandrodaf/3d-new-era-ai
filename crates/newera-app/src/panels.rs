//! Left side panel: furniture catalog on top, home contents below.

use eframe::egui::{self, Color32, RichText, Sense, Stroke, Vec2};
use egui_phosphor::regular as icon;
use newera_catalog::{CatalogItem, Category, SymbolShape};
use newera_core::{ElementId, FurnitureId, Point2};

use crate::app::NewEraApp;
use crate::view::plan::Tool;

fn category_icon(category: Category) -> &'static str {
    match category {
        Category::Living => icon::COUCH,
        Category::Dining => icon::CHAIR,
        Category::Kitchen => icon::OVEN,
        Category::Bedroom => icon::BED,
        Category::Bathroom => icon::BATHTUB,
        Category::Laundry => icon::WASHING_MACHINE,
        Category::Office => icon::DESK,
        Category::DoorsWindows => icon::DOOR,
        Category::Structure => icon::STAIRS,
        Category::Decor => icon::PLANT,
        Category::Outdoor => icon::TREE,
    }
}

/// Small top-view thumbnail drawn from the item's plan symbol.
fn thumbnail(ui: &mut egui::Ui, item: &CatalogItem, size: f32) {
    let (rect, _) = ui.allocate_exact_size(Vec2::splat(size), Sense::hover());
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 4.0, Color32::from_rgb(250, 250, 248));
    let piece = item.instantiate(FurnitureId(0), Point2::new(0.0, 0.0));
    // Door swings extend in front of the opening; frame them too.
    let extent_y = if piece.is_opening() {
        piece.depth / 2.0 + piece.width
    } else {
        piece.depth
    };
    #[allow(clippy::cast_possible_truncation)]
    let scale = (f64::from(size - 6.0) / piece.width.max(extent_y)) as f32;
    let offset_y = if piece.is_opening() {
        -(piece.width / 2.0)
    } else {
        0.0
    };
    #[allow(clippy::cast_possible_truncation)]
    let to =
        |(x, y): (f64, f64)| rect.center() + Vec2::new(x as f32, (y + offset_y) as f32) * scale;
    for shape in newera_catalog::plan_symbol(&piece) {
        match shape {
            SymbolShape::Fill { points, detail } => {
                let pts: Vec<Point2> = points.iter().map(|&(x, y)| Point2::new(x, y)).collect();
                let mut mesh = egui::Mesh::default();
                let color = if detail {
                    Color32::from_rgb(222, 225, 229)
                } else {
                    Color32::WHITE
                };
                for p in &points {
                    mesh.colored_vertex(to(*p), color);
                }
                for [a, b, c] in newera_core::triangulate(&pts) {
                    #[allow(clippy::cast_possible_truncation)]
                    mesh.add_triangle(a as u32, b as u32, c as u32);
                }
                painter.add(egui::Shape::mesh(mesh));
            }
            SymbolShape::Line {
                points,
                closed,
                strong,
            } => {
                let mut pts: Vec<egui::Pos2> = points.iter().map(|p| to(*p)).collect();
                if closed && let Some(&first) = pts.first() {
                    pts.push(first);
                }
                let width = if strong { 1.2 } else { 0.7 };
                painter.add(egui::Shape::line(
                    pts,
                    Stroke::new(width, Color32::from_gray(70)),
                ));
            }
        }
    }
}

fn catalog_row(app: &mut NewEraApp, ui: &mut egui::Ui, item: &'static CatalogItem) {
    let selected = app.tool == Tool::Place(item.id);
    let unit = app.unit();
    let response = ui
        .horizontal(|ui| {
            thumbnail(ui, item, 34.0);
            ui.vertical(|ui| {
                ui.add_space(1.0);
                let name = RichText::new(item.name);
                ui.label(if selected {
                    name.color(crate::theme::ACCENT).strong()
                } else {
                    name
                });
                ui.weak(RichText::new(unit.format_size(item.size)).small());
            });
        })
        .response
        .interact(Sense::click());
    if response.hovered() {
        ui.painter().rect_stroke(
            response.rect.expand(1.0),
            4.0,
            Stroke::new(1.0, crate::theme::ACCENT.gamma_multiply(0.5)),
            egui::StrokeKind::Outside,
        );
    }
    if response
        .on_hover_text("Clique e depois clique na planta para posicionar")
        .clicked()
    {
        app.set_tool(Tool::Place(item.id));
    }
}

pub(crate) fn left(app: &mut NewEraApp, ui: &mut egui::Ui) {
    egui::Panel::top("catalog")
        .resizable(true)
        .default_size(ui.available_height() * 0.55)
        .show(ui, |ui| {
            ui.add_space(4.0);
            ui.label(RichText::new(format!("{} Catálogo", icon::ARMCHAIR)).heading());
            ui.horizontal(|ui| {
                ui.label(icon::MAGNIFYING_GLASS);
                ui.add(
                    egui::TextEdit::singleline(&mut app.catalog_query)
                        .hint_text("Buscar: cama, janela, sofá…")
                        .desired_width(f32::INFINITY),
                );
            });
            egui::ScrollArea::vertical()
                .id_salt("catalog_scroll")
                .show(ui, |ui| {
                    let query = app.catalog_query.trim().to_owned();
                    if query.is_empty() {
                        for category in Category::ALL {
                            let items: Vec<&'static CatalogItem> = newera_catalog::CATALOG
                                .iter()
                                .filter(|i| i.category == category)
                                .collect();
                            egui::CollapsingHeader::new(format!(
                                "{}  {} ({})",
                                category_icon(category),
                                category.name(),
                                items.len()
                            ))
                            .id_salt(category.id())
                            .show(ui, |ui| {
                                for item in items {
                                    catalog_row(app, ui, item);
                                }
                            });
                        }
                    } else {
                        let found = newera_catalog::search(&query);
                        if found.is_empty() {
                            ui.weak("Nada encontrado.");
                        }
                        for item in found {
                            catalog_row(app, ui, item);
                        }
                    }
                });
        });

    egui::CentralPanel::default().show(ui, |ui| {
        let home = {
            let doc = app.document.read();
            doc.home().level_view(doc.home().current_level())
        };
        ui.add_space(4.0);
        ui.label(RichText::new(format!("{} {}", icon::HOUSE_LINE, home.name)).heading());
        let total: f64 = home.rooms.iter().map(newera_core::Room::area).sum();
        ui.weak(format!(
            "{} paredes · {} cômodos · {} móveis · {}",
            home.walls.len(),
            home.rooms.len(),
            home.furniture.len(),
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
                format!("{} Móveis ({})", icon::ARMCHAIR, home.furniture.len()),
                home.furniture
                    .iter()
                    .map(|f| {
                        let size = unit.format_size([f.width, f.depth, f.height]);
                        (f.id.into(), format!("{} · {size}", f.name))
                    })
                    .collect(),
            );
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
                home.dimensions.iter().map(|d| (d.id.into(), unit.format_length(d.length()))).collect(),
            );
            section(
                ui,
                format!("{} Textos ({})", icon::TEXT_T, home.labels.len()),
                home.labels
                    .iter()
                    .map(|l| (l.id.into(), l.text.lines().next().unwrap_or_default().to_owned()))
                    .collect(),
            );
            if home.walls.is_empty() && home.rooms.is_empty() && home.furniture.is_empty() {
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
