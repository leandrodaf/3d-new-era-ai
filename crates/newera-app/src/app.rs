use eframe::egui;
use newera_core::{Command, CoreResult, Home, SharedDocument, Wall, WallId};

use crate::view::plan::{PlanView, Tool};
use crate::view::scene::SceneView;

/// How often to look for changes made by other clients (MCP, HTTP) while the
/// window is idle.
const EXTERNAL_CHANGES_POLL: std::time::Duration = std::time::Duration::from_millis(200);

/// Furniture categories shown in the catalog. Items arrive with the
/// furniture milestone; the tree is here so the layout matches the target UX.
const CATALOG: &[(&str, &[&str])] = &[
    (
        "Sala de estar",
        &["Sofá", "Poltrona", "Mesa de centro", "Estante"],
    ),
    ("Cozinha", &["Geladeira", "Fogão", "Pia", "Armário"]),
    (
        "Quarto",
        &["Cama de casal", "Cama de solteiro", "Guarda-roupa"],
    ),
    ("Banheiro", &["Vaso sanitário", "Chuveiro", "Lavatório"]),
    ("Portas e janelas", &["Porta", "Janela", "Porta de correr"]),
];

#[derive(Debug)]
pub(crate) struct NewEraApp {
    document: SharedDocument,
    mcp_url: Option<String>,
    tool: Tool,
    selected: Option<WallId>,
    plan: PlanView,
    scene: SceneView,
    status: Option<String>,
    last_revision: u64,
}

impl NewEraApp {
    pub(crate) fn new(document: SharedDocument, mcp_url: Option<String>) -> Self {
        Self {
            document,
            mcp_url,
            tool: Tool::default(),
            selected: None,
            plan: PlanView::default(),
            scene: SceneView::new(),
            status: None,
            last_revision: 0,
        }
    }

    fn run(&mut self, action: impl FnOnce(&mut newera_core::Document) -> CoreResult<()>) {
        let result = action(&mut self.document.write());
        self.status = result.err().map(|err| format!("⚠ {err}"));
    }

    fn execute(&mut self, command: Command) {
        self.run(|doc| doc.execute(command));
    }

    fn delete_selected(&mut self) {
        if let Some(id) = self.selected.take() {
            self.execute(Command::RemoveWall { id });
        }
    }

    fn shortcuts(&mut self, ctx: &egui::Context) {
        use egui::{Key, KeyboardShortcut, Modifiers};
        const UNDO: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::Z);
        const REDO: KeyboardShortcut =
            KeyboardShortcut::new(Modifiers::COMMAND.plus(Modifiers::SHIFT), Key::Z);
        const REDO_ALT: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::Y);

        // Text fields keep their own shortcuts.
        if ctx.egui_wants_keyboard_input() {
            return;
        }
        // Check redo first: Ctrl+Shift+Z also matches the plain Ctrl+Z shortcut.
        if ctx.input_mut(|i| i.consume_shortcut(&REDO) || i.consume_shortcut(&REDO_ALT)) {
            self.run(newera_core::Document::redo);
        } else if ctx.input_mut(|i| i.consume_shortcut(&UNDO)) {
            self.run(newera_core::Document::undo);
        }
        if ctx.input(|i| i.key_pressed(Key::Delete) || i.key_pressed(Key::Backspace)) {
            self.delete_selected();
        }
        if ctx.input(|i| i.key_pressed(Key::Escape)) {
            self.plan.cancel();
            self.tool = Tool::Select;
        }
    }

    fn menu_bar(&mut self, ui: &mut egui::Ui) {
        egui::MenuBar::new().ui(ui, |ui| {
            ui.menu_button("Arquivo", |ui| {
                if ui.button("Nova casa").clicked() {
                    self.document.write().load(Home::default());
                    self.selected = None;
                    self.plan.request_fit();
                }
                ui.separator();
                if ui.button("Sair").clicked() {
                    ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
                }
            });
            ui.menu_button("Editar", |ui| {
                let (can_undo, can_redo) = {
                    let doc = self.document.read();
                    (doc.can_undo(), doc.can_redo())
                };
                if ui
                    .add_enabled(
                        can_undo,
                        egui::Button::new("Desfazer").shortcut_text("Ctrl+Z"),
                    )
                    .clicked()
                {
                    self.run(newera_core::Document::undo);
                }
                if ui
                    .add_enabled(
                        can_redo,
                        egui::Button::new("Refazer").shortcut_text("Ctrl+Shift+Z"),
                    )
                    .clicked()
                {
                    self.run(newera_core::Document::redo);
                }
                ui.separator();
                if ui
                    .add_enabled(
                        self.selected.is_some(),
                        egui::Button::new("Excluir").shortcut_text("Del"),
                    )
                    .clicked()
                {
                    self.delete_selected();
                }
            });
            ui.menu_button("Planta", |ui| {
                ui.radio_value(&mut self.tool, Tool::Select, "Selecionar");
                ui.radio_value(&mut self.tool, Tool::CreateWalls, "Criar paredes");
            });
            ui.menu_button("Ver", |ui| {
                if ui.button("Enquadrar planta").clicked() {
                    self.plan.request_fit();
                }
                if ui.button("Enquadrar 3D").clicked() {
                    self.scene.frame_home(self.document.read().home());
                }
            });
        });
    }

    fn toolbar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.selectable_value(&mut self.tool, Tool::Select, "Selecionar");
            ui.selectable_value(&mut self.tool, Tool::CreateWalls, "Criar paredes");
            ui.separator();
            let (can_undo, can_redo) = {
                let doc = self.document.read();
                (doc.can_undo(), doc.can_redo())
            };
            if ui
                .add_enabled(can_undo, egui::Button::new("Desfazer"))
                .on_hover_text("Ctrl+Z")
                .clicked()
            {
                self.run(newera_core::Document::undo);
            }
            if ui
                .add_enabled(can_redo, egui::Button::new("Refazer"))
                .on_hover_text("Ctrl+Shift+Z")
                .clicked()
            {
                self.run(newera_core::Document::redo);
            }
        });
    }

    fn status_bar(&self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            match &self.mcp_url {
                Some(url) => ui.label(format!("MCP ativo · {url}")),
                None => ui.weak("MCP desligado"),
            };
            ui.separator();
            let hint = match self.tool {
                Tool::Select => "Clique numa parede para selecionar · Del exclui · Scroll zoom · botão do meio move · F enquadra",
                Tool::CreateWalls => "Clique para iniciar e encadear paredes · duplo clique ou botão direito encerra · Esc sai",
            };
            ui.weak(hint);
            if let Some(status) = &self.status {
                ui.separator();
                ui.colored_label(ui.visuals().warn_fg_color, status);
            }
        });
    }

    fn left_panel(&mut self, ui: &mut egui::Ui) {
        egui::Panel::top("catalog")
            .resizable(true)
            .default_size(ui.available_height() * 0.55)
            .show(ui, |ui| {
                ui.heading("Catálogo");
                egui::ScrollArea::vertical()
                    .id_salt("catalog_scroll")
                    .show(ui, |ui| {
                        for (category, items) in CATALOG {
                            egui::CollapsingHeader::new(*category).show(ui, |ui| {
                                for item in *items {
                                    ui.add_enabled(false, egui::Label::new(*item))
                                        .on_disabled_hover_text(
                                            "Móveis chegam no próximo marco do roadmap",
                                        );
                                }
                            });
                        }
                    });
            });

        egui::CentralPanel::default().show(ui, |ui| {
            let doc = self.document.read();
            let home = doc.home();
            ui.heading(&home.name);
            ui.weak(format!(
                "{} paredes · {} cômodos",
                home.walls.len(),
                home.rooms.len()
            ));
            ui.separator();
            let mut clicked = None;
            egui::ScrollArea::vertical()
                .id_salt("home_scroll")
                .show(ui, |ui| {
                    for (i, wall) in home.walls.iter().enumerate() {
                        let label = format!("Parede {} · {:.0} cm", i + 1, wall.length());
                        if ui
                            .selectable_label(self.selected == Some(wall.id), label)
                            .clicked()
                        {
                            clicked = Some(wall.id);
                        }
                    }
                    for room in &home.rooms {
                        ui.label(format!("{} · {:.2} m²", room.name, room.area() / 10_000.0));
                    }
                });
            drop(doc);
            if clicked.is_some() {
                self.selected = clicked;
            }
        });
    }
}

impl eframe::App for NewEraApp {
    fn ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        self.shortcuts(&ctx);

        egui::Panel::top("menu").show(ui, |ui| {
            self.menu_bar(ui);
            self.toolbar(ui);
        });
        egui::Panel::bottom("status").show(ui, |ui| self.status_bar(ui));
        egui::Panel::left("left")
            .resizable(true)
            .default_size(260.0)
            .show(ui, |ui| self.left_panel(ui));

        egui::CentralPanel::default()
            .frame(egui::Frame::NONE)
            .show(ui, |ui| {
                let (home, revision) = {
                    let doc = self.document.read();
                    (doc.home().clone(), doc.revision())
                };
                // A selection may vanish after undo or an agent's edit.
                if self.selected.is_some_and(|id| home.wall(id).is_none()) {
                    self.selected = None;
                }

                egui::Panel::top("plan")
                    .resizable(true)
                    .default_size(ui.available_height() / 2.0)
                    .frame(egui::Frame::NONE)
                    .show(ui, |ui| {
                        let out = self.plan.ui(ui, &home, self.tool, self.selected);
                        if let Some(selection) = out.select {
                            self.selected = selection;
                        }
                        if let Some((start, end)) = out.new_wall {
                            self.run(|doc| {
                                let wall = Wall::new(doc.new_wall_id(), start, end);
                                doc.execute(Command::add_wall(wall))
                            });
                        }
                    });

                egui::CentralPanel::default()
                    .frame(egui::Frame::NONE)
                    .show(ui, |ui| {
                        self.scene.ui(
                            ui,
                            frame.wgpu_render_state(),
                            &home,
                            revision,
                            self.selected,
                        );
                    });

                if revision != self.last_revision {
                    self.last_revision = revision;
                    ctx.request_repaint();
                }
            });

        ctx.request_repaint_after(EXTERNAL_CHANGES_POLL);
    }
}
