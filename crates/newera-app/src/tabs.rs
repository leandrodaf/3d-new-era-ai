//! Plan variants as tabs: several versions of the same project, each with
//! its own history, to try alternatives and compare them side by side.

use eframe::egui::{self, RichText};
use egui_phosphor::regular as icon;
use newera_core::Home;

use crate::app::NewEraApp;
use crate::dialogs::Dialog;

/// Numbers that make versions comparable at a glance.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct VariantStats {
    pub(crate) name: String,
    pub(crate) active: bool,
    pub(crate) walls: usize,
    pub(crate) rooms: usize,
    /// Total room area, cm².
    pub(crate) area: f64,
    pub(crate) furniture: usize,
    pub(crate) issues: usize,
}

pub(crate) fn stats(name: &str, active: bool, home: &Home) -> VariantStats {
    VariantStats {
        name: name.to_owned(),
        active,
        walls: home.walls.len(),
        rooms: home.rooms.len(),
        area: home.rooms.iter().map(newera_core::Room::area).sum(),
        furniture: home.furniture.len(),
        issues: newera_core::check_layout(home).len(),
    }
}

pub(crate) fn bar(app: &mut NewEraApp, ui: &mut egui::Ui) {
    let infos = app.document.read().variant_infos();
    let count = infos.len();
    let mut switch_to = None;
    let mut duplicate = false;
    let mut blank = false;
    let mut close = None;

    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 3.0;
        egui::ScrollArea::horizontal()
            .id_salt("variant_tabs")
            .max_width(ui.available_width() - 190.0)
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    for info in &infos {
                        if let Some((index, name)) = &mut app.renaming_variant
                            && *index == info.index
                        {
                            let edit =
                                ui.add(egui::TextEdit::singleline(name).desired_width(120.0));
                            edit.request_focus();
                            if edit.lost_focus() || ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                                let (index, name) = app.renaming_variant.take().expect("renaming");
                                app.run(|doc| doc.rename_variant(index, name.trim()));
                            }
                            continue;
                        }
                        let text = RichText::new(format!("{} {}", icon::FILE_TEXT, info.name));
                        let tab = ui.add(egui::Button::selectable(
                            info.active,
                            if info.active { text.strong() } else { text },
                        ));
                        if tab.clicked() && !info.active {
                            switch_to = Some(info.index);
                        }
                        if tab.double_clicked() {
                            app.renaming_variant = Some((info.index, info.name.clone()));
                        }
                        tab.context_menu(|ui| {
                            if ui
                                .button(format!("{} Renomear", icon::PENCIL_SIMPLE))
                                .clicked()
                            {
                                app.renaming_variant = Some((info.index, info.name.clone()));
                                ui.close();
                            }
                            if ui.button(format!("{} Duplicar", icon::COPY)).clicked() {
                                switch_to = Some(info.index);
                                duplicate = true;
                                ui.close();
                            }
                            if ui
                                .add_enabled(
                                    count > 1,
                                    egui::Button::new(format!("{} Fechar versão", icon::X)),
                                )
                                .clicked()
                            {
                                close = Some(info.index);
                                ui.close();
                            }
                        });
                    }
                });
            });
        ui.menu_button(RichText::new(icon::PLUS).size(16.0), |ui| {
            if ui
                .add(
                    egui::Button::new(format!("{} Duplicar versão atual", icon::COPY))
                        .shortcut_text("Ctrl+T"),
                )
                .clicked()
            {
                duplicate = true;
                ui.close();
            }
            if ui
                .button(format!("{} Nova versão em branco", icon::FILE_PLUS))
                .clicked()
            {
                blank = true;
                ui.close();
            }
        })
        .response
        .on_hover_text("Nova versão da planta");
        let compare = count > 1
            && ui
                .button(format!("{} Comparar", icon::CHART_BAR))
                .on_hover_text("Comparar as versões")
                .clicked();
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            levels(app, ui);
        });
        if compare {
            app.open_compare();
        }
    });

    if let Some(index) = switch_to {
        app.run(|doc| doc.switch_variant(index));
        app.after_variant_change();
    }
    if duplicate || blank {
        app.run(|doc| {
            doc.add_variant(None, duplicate);
            Ok(())
        });
        app.after_variant_change();
    }
    if let Some(index) = close {
        let name = infos[index].name.clone();
        app.set_dialog(Dialog::ConfirmCloseVariant { index, name });
    }
}

/// Storey selector: pick, add, edit and delete levels.
fn levels(app: &mut NewEraApp, ui: &mut egui::Ui) {
    let (levels, current) = {
        let doc = app.document.read();
        let home = doc.home();
        let levels: Vec<newera_core::Level> = home.sorted_levels().into_iter().cloned().collect();
        (levels, home.current_level())
    };
    if ui
        .button(format!("{} Andar", icon::PLUS))
        .on_hover_text("Adicionar um andar acima do mais alto")
        .clicked()
    {
        app.run(|doc| newera_core::ops::add_level(doc, None, None).map(|_| ()));
        app.after_variant_change();
    }
    if levels.is_empty() {
        ui.weak(format!("{} Térreo", icon::STACK));
        return;
    }
    let current_name = levels
        .iter()
        .find(|l| Some(l.id) == current)
        .map_or_else(String::new, |l| l.name.clone());
    let mut select = None;
    egui::ComboBox::from_id_salt("level_select")
        .selected_text(format!("{} {current_name}", icon::STACK))
        .show_ui(ui, |ui| {
            for level in levels.iter().rev() {
                let label = format!(
                    "{} · {}",
                    level.name,
                    app.unit().format_length(level.elevation)
                );
                if ui
                    .selectable_label(Some(level.id) == current, label)
                    .clicked()
                {
                    select = Some(level.id);
                }
            }
            ui.separator();
            if ui
                .button(format!("{} Editar andar…", icon::PENCIL_SIMPLE))
                .clicked()
                && let Some(id) = current
            {
                app.open_modify(&[id.into()]);
            }
            if ui
                .add_enabled(
                    levels.len() > 1,
                    egui::Button::new(format!("{} Excluir andar", icon::TRASH)),
                )
                .clicked()
                && let Some(level) = levels.iter().find(|l| Some(l.id) == current)
            {
                app.set_dialog(Dialog::ConfirmDeleteLevel {
                    id: level.id,
                    name: level.name.clone(),
                });
            }
        });
    if let Some(id) = select {
        app.document.write().select_level(Some(id));
        app.after_variant_change();
    }
}

#[cfg(test)]
mod tests {
    use newera_core::{Command, Document, Point2, Wall};

    use super::*;

    #[test]
    fn stats_summarize_a_home() {
        let mut doc = Document::default();
        let wall = Wall::new(
            doc.new_wall_id(),
            Point2::new(0.0, 0.0),
            Point2::new(300.0, 0.0),
        );
        doc.execute(Command::insert(wall)).unwrap();
        let s = stats("A", true, doc.home());
        assert_eq!((s.walls, s.rooms, s.furniture, s.issues), (1, 0, 0, 0));
    }
}
