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
        if ui
            .add_enabled(
                count > 1,
                egui::Button::new(format!("{} Comparar", icon::CHART_BAR)),
            )
            .on_hover_text("Comparar as versões")
            .clicked()
        {
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
