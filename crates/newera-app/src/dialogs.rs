//! Modal dialogs: element properties, background calibration, home settings.

use eframe::egui::{self, DragValue, RichText};
use egui_phosphor::regular as icon;
use newera_core::{
    BackgroundImage, Command, Compass, Dimension, Element, Furniture, Label, Point2, Room, Wall,
    WallId,
};

use crate::app::{NewEraApp, Pending};

#[derive(Debug, Clone)]
pub(crate) enum Dialog {
    ModifyWalls {
        ids: Vec<WallId>,
        /// Only for a single wall: editable endpoints.
        points: Option<(Point2, Point2)>,
        thickness: f64,
        height: f64,
        arc: f64,
    },
    ModifyRoom(Room),
    ModifyDimension(Dimension),
    ModifyLabel(Label),
    ModifyFurniture {
        piece: Furniture,
        keep_ratio: bool,
    },
    NewLabel {
        at: Point2,
        text: String,
    },
    Calibrate {
        a: Point2,
        b: Point2,
        distance: f64,
    },
    Background(BackgroundImage),
    HomeSettings {
        name: String,
        compass: Compass,
    },
    ConfirmDiscard(Pending),
    ConfirmCloseVariant {
        index: usize,
        name: String,
    },
    Compare(Vec<crate::tabs::VariantStats>),
    ModifyLevel(newera_core::Level),
    ConfirmDeleteLevel {
        id: newera_core::LevelId,
        name: String,
    },
    Help,
}

#[derive(Debug)]
pub(crate) enum DialogOutcome {
    Keep(Dialog),
    Close,
}

impl Dialog {
    /// The properties dialog for a selection, if its elements share one.
    pub(crate) fn modify(elements: &[Element]) -> Option<Self> {
        let walls: Vec<&Wall> = elements
            .iter()
            .filter_map(|e| match e {
                Element::Wall(w) => Some(w),
                _ => None,
            })
            .collect();
        if !walls.is_empty() && walls.len() == elements.len() {
            let first = walls[0];
            return Some(Self::ModifyWalls {
                ids: walls.iter().map(|w| w.id).collect(),
                points: (walls.len() == 1).then_some((first.start, first.end)),
                thickness: first.thickness,
                height: first.height,
                arc: first.arc_extent.unwrap_or(0.0),
            });
        }
        match elements {
            [Element::Room(r)] => Some(Self::ModifyRoom(r.clone())),
            [Element::Dimension(d)] => Some(Self::ModifyDimension(d.clone())),
            [Element::Label(l)] => Some(Self::ModifyLabel(l.clone())),
            [Element::Level(l)] => Some(Self::ModifyLevel(l.clone())),
            [Element::Furniture(f)] => Some(Self::ModifyFurniture {
                piece: f.clone(),
                keep_ratio: false,
            }),
            _ => None,
        }
    }
}

fn cm(value: &mut f64, range: std::ops::RangeInclusive<f64>) -> DragValue<'_> {
    DragValue::new(value)
        .range(range)
        .suffix(" cm")
        .speed(1.0)
        .max_decimals(1)
}

/// Shows a dialog with OK/Cancel. Returns `Some(true)` on OK, `Some(false)` on cancel.
fn modal(ctx: &egui::Context, title: &str, body: impl FnOnce(&mut egui::Ui)) -> Option<bool> {
    let mut result = None;
    let response = egui::Modal::new(egui::Id::new("newera-dialog")).show(ctx, |ui| {
        ui.set_min_width(320.0);
        ui.heading(title);
        ui.add_space(6.0);
        body(ui);
        ui.add_space(10.0);
        ui.separator();
        ui.horizontal(|ui| {
            // Shift+Enter stays available for new lines in multi-line fields.
            let enter = ui.input(|i| i.key_pressed(egui::Key::Enter) && !i.modifiers.shift);
            if ui.button(format!("{} OK", icon::CHECK)).clicked() || enter {
                result = Some(true);
            }
            if ui.button("Cancelar").clicked() {
                result = Some(false);
            }
        });
    });
    if response.should_close() && result.is_none() {
        result = Some(false);
    }
    result
}

fn grid(ui: &mut egui::Ui, id: &str, rows: impl FnOnce(&mut egui::Ui)) {
    egui::Grid::new(id)
        .num_columns(2)
        .spacing([12.0, 6.0])
        .show(ui, rows);
}

pub(crate) fn show(app: &mut NewEraApp, ctx: &egui::Context, dialog: Dialog) -> DialogOutcome {
    let unit = app.unit();
    match dialog {
        Dialog::ModifyWalls {
            ids,
            mut points,
            mut thickness,
            mut height,
            mut arc,
        } => {
            let title = if ids.len() == 1 {
                "Modificar parede".to_owned()
            } else {
                format!("Modificar {} paredes", ids.len())
            };
            let answer = modal(ctx, &title, |ui| {
                grid(ui, "walls", |ui| {
                    if let Some((start, end)) = &mut points {
                        ui.label("Início (x, y)");
                        ui.horizontal(|ui| {
                            ui.add(cm(&mut start.x, -1e6..=1e6));
                            ui.add(cm(&mut start.y, -1e6..=1e6));
                        });
                        ui.end_row();
                        ui.label("Fim (x, y)");
                        ui.horizontal(|ui| {
                            ui.add(cm(&mut end.x, -1e6..=1e6));
                            ui.add(cm(&mut end.y, -1e6..=1e6));
                        });
                        ui.end_row();
                        ui.label("Comprimento");
                        ui.label(RichText::new(unit.format_length(start.distance(*end))).strong());
                        ui.end_row();
                    }
                    ui.label("Espessura");
                    ui.add(cm(&mut thickness, 0.5..=500.0));
                    ui.end_row();
                    ui.label("Altura");
                    ui.add(cm(&mut height, 1.0..=5000.0));
                    ui.end_row();
                    ui.label("Arco");
                    ui.add(
                        DragValue::new(&mut arc)
                            .range(-270.0..=270.0)
                            .suffix("°")
                            .speed(1.0),
                    );
                    ui.end_row();
                });
            });
            match answer {
                Some(true) => {
                    app.run(|doc| {
                        let commands = ids
                            .iter()
                            .filter_map(|id| doc.home().wall(*id).cloned())
                            .map(|mut w| {
                                if let Some((start, end)) = points {
                                    w.start = start;
                                    w.end = end;
                                }
                                w.thickness = thickness;
                                w.height = height;
                                w.arc_extent = (arc.abs() >= 1.0).then_some(arc);
                                Command::update(w)
                            })
                            .collect();
                        doc.execute(Command::Batch { commands })
                    });
                    DialogOutcome::Close
                }
                Some(false) => DialogOutcome::Close,
                None => DialogOutcome::Keep(Dialog::ModifyWalls {
                    ids,
                    points,
                    thickness,
                    height,
                    arc,
                }),
            }
        }
        Dialog::ModifyRoom(mut room) => {
            let answer = modal(ctx, "Modificar cômodo", |ui| {
                grid(ui, "room", |ui| {
                    ui.label("Nome");
                    ui.text_edit_singleline(&mut room.name).request_focus();
                    ui.end_row();
                    ui.label("Área");
                    ui.label(RichText::new(unit.format_area(room.area())).strong());
                    ui.end_row();
                    ui.label("Exibir");
                    ui.vertical(|ui| {
                        ui.checkbox(&mut room.area_visible, "Área na planta");
                        ui.checkbox(&mut room.floor_visible, "Piso");
                        ui.checkbox(&mut room.ceiling_visible, "Teto");
                    });
                    ui.end_row();
                });
            });
            finish(app, answer, Dialog::ModifyRoom(room.clone()), || {
                Command::update(room)
            })
        }
        Dialog::ModifyDimension(mut dim) => {
            let answer = modal(ctx, "Modificar cota", |ui| {
                grid(ui, "dim", |ui| {
                    ui.label("Medida");
                    ui.label(RichText::new(unit.format_length(dim.length())).strong());
                    ui.end_row();
                    ui.label("Afastamento");
                    ui.add(cm(&mut dim.offset, -10_000.0..=10_000.0));
                    ui.end_row();
                });
            });
            finish(app, answer, Dialog::ModifyDimension(dim.clone()), || {
                Command::update(dim)
            })
        }
        Dialog::ModifyLabel(mut label) => {
            let answer = modal(ctx, "Modificar texto", |ui| {
                label_fields(ui, &mut label.text, &mut label.size, &mut label.angle);
            });
            finish(app, answer, Dialog::ModifyLabel(label.clone()), || {
                Command::update(label)
            })
        }
        Dialog::ModifyFurniture {
            mut piece,
            mut keep_ratio,
        } => {
            let before = (piece.width, piece.depth, piece.height);
            let answer =
                modal(
                    ctx,
                    &format!("Modificar {}", piece.name.to_lowercase()),
                    |ui| {
                        grid(ui, "furniture", |ui| {
                            ui.label("Nome");
                            ui.text_edit_singleline(&mut piece.name);
                            ui.end_row();
                            ui.label("Posição (x, y)");
                            ui.horizontal(|ui| {
                                ui.add(cm(&mut piece.position.x, -1e6..=1e6));
                                ui.add(cm(&mut piece.position.y, -1e6..=1e6));
                            });
                            ui.end_row();
                            ui.label("Elevação");
                            ui.add(cm(&mut piece.elevation, -1000.0..=10_000.0));
                            ui.end_row();
                            ui.label("Ângulo");
                            ui.add(
                                DragValue::new(&mut piece.angle)
                                    .range(-360.0..=360.0)
                                    .suffix("°")
                                    .speed(1.0),
                            );
                            ui.end_row();
                            ui.label("Largura");
                            ui.add(cm(&mut piece.width, 1.0..=10_000.0));
                            ui.end_row();
                            ui.label("Profundidade");
                            ui.add(cm(&mut piece.depth, 1.0..=10_000.0));
                            ui.end_row();
                            ui.label("Altura");
                            ui.add(cm(&mut piece.height, 1.0..=10_000.0));
                            ui.end_row();
                            ui.label("");
                            ui.checkbox(&mut keep_ratio, "Manter proporções");
                            ui.end_row();
                            ui.label("Cor");
                            ui.horizontal(|ui| {
                                let mut custom = piece.color.is_some();
                                if ui.checkbox(&mut custom, "Personalizada").changed() {
                                    piece.color = custom.then_some([180, 180, 180]);
                                }
                                if let Some(color) = &mut piece.color {
                                    ui.color_edit_button_srgb(color);
                                }
                            });
                            ui.end_row();
                            ui.label("");
                            ui.vertical(|ui| {
                                ui.checkbox(&mut piece.mirrored, "Espelhado");
                                ui.checkbox(&mut piece.visible, "Visível");
                                if let Some(opening) = piece.opening.as_mut().filter(|o| {
                                    o.kind == newera_core::OpeningKind::Door && !o.sliding
                                }) {
                                    ui.checkbox(&mut opening.hinge_right, "Dobradiça à direita");
                                }
                            });
                            ui.end_row();
                        });
                    },
                );
            if keep_ratio {
                // Scale the other sizes by whichever one changed.
                let (w0, d0, h0) = before;
                let factor = if (piece.width - w0).abs() > 1e-9 {
                    piece.width / w0
                } else if (piece.depth - d0).abs() > 1e-9 {
                    piece.depth / d0
                } else if (piece.height - h0).abs() > 1e-9 {
                    piece.height / h0
                } else {
                    1.0
                };
                (piece.width, piece.depth, piece.height) = (w0 * factor, d0 * factor, h0 * factor);
            }
            finish(
                app,
                answer,
                Dialog::ModifyFurniture {
                    piece: piece.clone(),
                    keep_ratio,
                },
                || Command::update(piece),
            )
        }
        Dialog::NewLabel { at, mut text } => {
            let mut size = Label::DEFAULT_SIZE;
            let mut angle = 0.0;
            let answer = modal(ctx, "Adicionar texto", |ui| {
                label_fields(ui, &mut text, &mut size, &mut angle);
            });
            match answer {
                Some(true) if !text.trim().is_empty() => {
                    app.run(|doc| {
                        let label = Label {
                            id: doc.new_label_id(),
                            text: text.trim_end().to_owned(),
                            position: at,
                            size,
                            angle,
                            level: None,
                        };
                        doc.execute(Command::insert(label))
                    });
                    DialogOutcome::Close
                }
                Some(_) => DialogOutcome::Close,
                None => DialogOutcome::Keep(Dialog::NewLabel { at, text }),
            }
        }
        Dialog::Calibrate { a, b, mut distance } => {
            let answer = modal(ctx, "Calibrar escala da imagem", |ui| {
                ui.label(format!(
                    "Os pontos marcados estão a {} na escala atual.",
                    unit.format_length(a.distance(b))
                ));
                ui.add_space(4.0);
                grid(ui, "calib", |ui| {
                    ui.label("Distância real");
                    ui.add(cm(&mut distance, 1.0..=1e6)).request_focus();
                    ui.end_row();
                });
            });
            match answer {
                Some(true) => {
                    app.run(|doc| {
                        let Some(mut bg) = doc.home().background.clone() else {
                            return Ok(());
                        };
                        let measured = a.distance(b);
                        if measured > 1e-6 {
                            // Scale around the first point so it stays where it was marked.
                            let factor = distance / measured;
                            bg.cm_per_px *= factor;
                            bg.offset = Point2::new(
                                a.x - (a.x - bg.offset.x) * factor,
                                a.y - (a.y - bg.offset.y) * factor,
                            );
                        }
                        doc.execute(Command::SetBackground {
                            background: Some(bg),
                        })
                    });
                    app.set_status("Escala calibrada. Arraste para posicionar a imagem ou troque de ferramenta.");
                    DialogOutcome::Close
                }
                Some(false) => DialogOutcome::Close,
                None => DialogOutcome::Keep(Dialog::Calibrate { a, b, distance }),
            }
        }
        Dialog::Background(mut bg) => {
            let mut remove = false;
            let answer = modal(ctx, "Imagem de fundo", |ui| {
                grid(ui, "bg", |ui| {
                    ui.label("Arquivo");
                    ui.label(RichText::new(&bg.path).small());
                    ui.end_row();
                    ui.label("Escala");
                    ui.add(
                        DragValue::new(&mut bg.cm_per_px)
                            .range(0.001..=1000.0)
                            .speed(0.01)
                            .suffix(" cm/px"),
                    );
                    ui.end_row();
                    ui.label("Posição (x, y)");
                    ui.horizontal(|ui| {
                        ui.add(cm(&mut bg.offset.x, -1e6..=1e6));
                        ui.add(cm(&mut bg.offset.y, -1e6..=1e6));
                    });
                    ui.end_row();
                    ui.label("Opacidade");
                    ui.add(egui::Slider::new(&mut bg.opacity, 0.0..=1.0));
                    ui.end_row();
                    ui.label("");
                    ui.checkbox(&mut bg.visible, "Visível");
                    ui.end_row();
                });
                ui.add_space(6.0);
                if ui
                    .button(format!("{} Remover imagem", icon::TRASH))
                    .clicked()
                {
                    remove = true;
                }
            });
            if remove {
                app.run(|doc| doc.execute(Command::SetBackground { background: None }));
                return DialogOutcome::Close;
            }
            finish(app, answer, Dialog::Background(bg.clone()), || {
                Command::SetBackground {
                    background: Some(bg),
                }
            })
        }
        Dialog::HomeSettings {
            mut name,
            mut compass,
        } => {
            let answer = modal(ctx, "Casa e bússola", |ui| {
                grid(ui, "home", |ui| {
                    ui.label("Nome do projeto");
                    ui.text_edit_singleline(&mut name);
                    ui.end_row();
                    ui.label("Norte");
                    ui.add(
                        DragValue::new(&mut compass.north_degrees)
                            .range(0.0..=360.0)
                            .suffix("°")
                            .speed(1.0),
                    );
                    ui.end_row();
                    ui.label("Centro (x, y)");
                    ui.horizontal(|ui| {
                        ui.add(cm(&mut compass.center.x, -1e6..=1e6));
                        ui.add(cm(&mut compass.center.y, -1e6..=1e6));
                    });
                    ui.end_row();
                    ui.label("Diâmetro");
                    ui.add(cm(&mut compass.diameter, 10.0..=10_000.0));
                    ui.end_row();
                    ui.label("");
                    ui.checkbox(&mut compass.visible, "Mostrar bússola");
                    ui.end_row();
                });
            });
            let (n, c) = (name.clone(), compass);
            finish(app, answer, Dialog::HomeSettings { name, compass }, || {
                Command::Batch {
                    commands: vec![
                        Command::RenameHome { name: n },
                        Command::SetCompass { compass: c },
                    ],
                }
            })
        }
        Dialog::ConfirmDiscard(action) => {
            let mut choice = None;
            egui::Modal::new(egui::Id::new("confirm-discard")).show(ctx, |ui| {
                ui.set_min_width(340.0);
                ui.heading("Salvar alterações?");
                ui.label("O projeto tem alterações que ainda não foram salvas.");
                ui.add_space(10.0);
                ui.horizontal(|ui| {
                    if ui.button(format!("{} Salvar", icon::FLOPPY_DISK)).clicked() {
                        choice = Some(0);
                    }
                    if ui.button("Descartar").clicked() {
                        choice = Some(1);
                    }
                    if ui.button("Cancelar").clicked() {
                        choice = Some(2);
                    }
                });
            });
            match choice {
                Some(0) => {
                    if app.save(false) {
                        app.perform(action);
                    }
                    DialogOutcome::Close
                }
                Some(1) => {
                    app.perform(action);
                    DialogOutcome::Close
                }
                Some(_) => DialogOutcome::Close,
                None => DialogOutcome::Keep(Dialog::ConfirmDiscard(action)),
            }
        }
        Dialog::ConfirmCloseVariant { index, name } => {
            let mut choice = None;
            egui::Modal::new(egui::Id::new("close-variant")).show(ctx, |ui| {
                ui.set_min_width(340.0);
                ui.heading(format!("Fechar \"{name}\"?"));
                ui.label("Esta versão e o histórico dela serão removidos do projeto.");
                ui.add_space(10.0);
                ui.horizontal(|ui| {
                    if ui
                        .button(format!("{} Fechar versão", icon::TRASH))
                        .clicked()
                    {
                        choice = Some(true);
                    }
                    if ui.button("Cancelar").clicked() {
                        choice = Some(false);
                    }
                });
            });
            match choice {
                Some(true) => {
                    app.run(|doc| doc.remove_variant(index));
                    app.after_variant_change();
                    DialogOutcome::Close
                }
                Some(false) => DialogOutcome::Close,
                None => DialogOutcome::Keep(Dialog::ConfirmCloseVariant { index, name }),
            }
        }
        Dialog::ModifyLevel(mut level) => {
            let answer = modal(ctx, "Modificar andar", |ui| {
                grid(ui, "level", |ui| {
                    ui.label("Nome");
                    ui.text_edit_singleline(&mut level.name);
                    ui.end_row();
                    ui.label("Elevação do piso");
                    ui.add(cm(&mut level.elevation, -10_000.0..=100_000.0));
                    ui.end_row();
                    ui.label("Pé-direito");
                    ui.add(cm(&mut level.height, 50.0..=2_000.0));
                    ui.end_row();
                    ui.label("Espessura da laje");
                    ui.add(cm(&mut level.floor_thickness, 0.0..=200.0));
                    ui.end_row();
                });
            });
            finish(app, answer, Dialog::ModifyLevel(level.clone()), || {
                Command::update(level)
            })
        }
        Dialog::ConfirmDeleteLevel { id, name } => {
            let mut choice = None;
            egui::Modal::new(egui::Id::new("delete-level")).show(ctx, |ui| {
                ui.set_min_width(340.0);
                ui.heading(format!("Excluir \"{name}\"?"));
                ui.label("O andar e tudo o que está nele serão removidos (dá para desfazer).");
                ui.add_space(10.0);
                ui.horizontal(|ui| {
                    if ui
                        .button(format!("{} Excluir andar", icon::TRASH))
                        .clicked()
                    {
                        choice = Some(true);
                    }
                    if ui.button("Cancelar").clicked() {
                        choice = Some(false);
                    }
                });
            });
            match choice {
                Some(true) => {
                    app.run(|doc| newera_core::ops::delete_level(doc, id));
                    app.after_variant_change();
                    DialogOutcome::Close
                }
                Some(false) => DialogOutcome::Close,
                None => DialogOutcome::Keep(Dialog::ConfirmDeleteLevel { id, name }),
            }
        }
        Dialog::Compare(rows) => {
            let mut close = false;
            let mut switch = None;
            egui::Modal::new(egui::Id::new("compare-variants")).show(ctx, |ui| {
                ui.heading(format!("{} Comparar versões", icon::CHART_BAR));
                ui.add_space(6.0);
                egui::Grid::new("compare")
                    .striped(true)
                    .spacing([18.0, 6.0])
                    .show(ui, |ui| {
                        for title in [
                            "Versão",
                            "Paredes",
                            "Cômodos",
                            "Área",
                            "Móveis",
                            "Problemas",
                            "",
                        ] {
                            ui.label(RichText::new(title).strong());
                        }
                        ui.end_row();
                        let best_area = rows.iter().map(|r| r.area).fold(0.0, f64::max);
                        for (i, row) in rows.iter().enumerate() {
                            let name = RichText::new(&row.name);
                            ui.label(if row.active { name.strong() } else { name });
                            ui.label(row.walls.to_string());
                            ui.label(row.rooms.to_string());
                            let area = RichText::new(unit.format_area(row.area));
                            ui.label(
                                if rows.len() > 1
                                    && row.area > 0.0
                                    && (row.area - best_area).abs() < 1e-6
                                {
                                    area.strong()
                                } else {
                                    area
                                },
                            );
                            ui.label(row.furniture.to_string());
                            let issues = RichText::new(row.issues.to_string());
                            ui.label(if row.issues > 0 {
                                issues.color(ui.visuals().warn_fg_color)
                            } else {
                                issues
                            });
                            if row.active {
                                ui.weak("atual");
                            } else if ui.small_button("Abrir").clicked() {
                                switch = Some(i);
                            }
                            ui.end_row();
                        }
                    });
                ui.add_space(8.0);
                if ui.button("Fechar").clicked() {
                    close = true;
                }
            });
            if let Some(index) = switch {
                app.run(|doc| doc.switch_variant(index));
                app.after_variant_change();
                return DialogOutcome::Close;
            }
            if close || ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
                DialogOutcome::Close
            } else {
                DialogOutcome::Keep(Dialog::Compare(rows))
            }
        }
        Dialog::Help => {
            let mut close = false;
            egui::Modal::new(egui::Id::new("help")).show(ctx, |ui| {
                ui.set_min_width(460.0);
                ui.heading(format!("{} Atalhos e ferramentas", icon::KEYBOARD));
                ui.add_space(6.0);
                grid(ui, "help", |ui| {
                    for (keys, what) in [
                        (
                            "V · H · W · R · D · T",
                            "Selecionar · Mover vista · Paredes · Cômodos · Cotas · Texto",
                        ),
                        (
                            "Digitar número + Enter",
                            "Comprimento exato da parede sendo desenhada",
                        ),
                        (
                            "Shift (segurado)",
                            "Desliga o ímã (ângulos de 15°, pontos e grade)",
                        ),
                        (
                            "Duplo clique",
                            "Encerra paredes · fecha/detecta cômodo · cota parede · modifica",
                        ),
                        (
                            "Scroll · botão do meio · F",
                            "Zoom · mover vista · enquadrar",
                        ),
                        ("Setas (+Shift)", "Move a seleção 1 cm (10 cm)"),
                        (
                            "Ctrl+Z · Ctrl+Shift+Z",
                            "Desfazer · Refazer (inclusive o que a IA fez)",
                        ),
                        ("Ctrl+C · X · V · D", "Copiar · Recortar · Colar · Duplicar"),
                        ("Enter · Del · Esc", "Modificar · Excluir · Cancelar"),
                        (
                            "Ctrl+T · Ctrl+Tab",
                            "Duplicar versão · Próxima versão (guias)",
                        ),
                    ] {
                        ui.label(RichText::new(keys).monospace().strong());
                        ui.label(what);
                        ui.end_row();
                    }
                });
                ui.add_space(8.0);
                if ui.button("Fechar").clicked() {
                    close = true;
                }
            });
            if close || ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
                DialogOutcome::Close
            } else {
                DialogOutcome::Keep(Dialog::Help)
            }
        }
    }
}

fn label_fields(ui: &mut egui::Ui, text: &mut String, size: &mut f64, angle: &mut f64) {
    grid(ui, "label", |ui| {
        ui.label("Texto");
        ui.text_edit_multiline(text).request_focus();
        ui.end_row();
        ui.label("Tamanho");
        ui.add(cm(size, 1.0..=1000.0));
        ui.end_row();
        ui.label("Rotação");
        ui.add(
            DragValue::new(angle)
                .range(-360.0..=360.0)
                .suffix("°")
                .speed(1.0),
        );
        ui.end_row();
    });
}

fn finish(
    app: &mut NewEraApp,
    answer: Option<bool>,
    keep: Dialog,
    command: impl FnOnce() -> Command,
) -> DialogOutcome {
    match answer {
        Some(true) => {
            app.run(|doc| doc.execute(command()));
            DialogOutcome::Close
        }
        Some(false) => DialogOutcome::Close,
        None => DialogOutcome::Keep(keep),
    }
}
