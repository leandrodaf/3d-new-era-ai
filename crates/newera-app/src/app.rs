use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use eframe::egui::{self, Key, KeyboardShortcut, Modifiers, RichText};
use egui_phosphor::regular as icon;
use newera_core::{
    Command, CoreResult, Document, Element, ElementId, Home, LengthUnit, Point2, SharedDocument,
    ops,
};
use newera_draw::{
    Palette, RenderOptions, SceneOptions, SvgOptions, plan_scene, render_png, to_svg,
};

use crate::dialogs::{Dialog, DialogOutcome};
use crate::panels;
use crate::view::plan::{PlanEvent, PlanInput, PlanView, Selection, Tool};
use crate::view::scene::SceneView;

/// How often to look for changes made by other clients (MCP, HTTP) while idle.
const EXTERNAL_CHANGES_POLL: Duration = Duration::from_millis(200);
const PASTE_OFFSET: f64 = 30.0;

/// What to do once unsaved changes have been dealt with.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Pending {
    New,
    Open(Option<PathBuf>),
    Quit,
}

/// Preferences kept between runs.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Default)]
struct Settings {
    unit: LengthUnit,
    recent: Vec<PathBuf>,
}

const SETTINGS_KEY: &str = "newera-settings";

pub(crate) struct NewEraApp {
    pub(crate) document: SharedDocument,
    mcp_url: Option<String>,
    pub(crate) tool: Tool,
    pub(crate) selection: Selection,
    plan: PlanView,
    pub(crate) scene: SceneView,
    palette: Palette,
    settings: Settings,
    dialog: Option<Dialog>,
    status: Option<(String, Instant)>,
    clipboard: Vec<Element>,
    pending: Option<Pending>,
    allow_close: bool,
    title: String,
    plan_rect: egui::Rect,
    pub(crate) catalog_query: String,
    /// Tab being renamed in place: `(variant index, draft name)`.
    pub(crate) renaming_variant: Option<(usize, String)>,
}

impl std::fmt::Debug for NewEraApp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NewEraApp")
            .field("tool", &self.tool)
            .finish_non_exhaustive()
    }
}

impl NewEraApp {
    pub(crate) fn new(
        cc: &eframe::CreationContext<'_>,
        document: SharedDocument,
        mcp_url: Option<String>,
    ) -> Self {
        let mut fonts = egui::FontDefinitions::default();
        egui_phosphor::add_to_fonts(&mut fonts, egui_phosphor::Variant::Regular);
        cc.egui_ctx.set_fonts(fonts);
        crate::theme::apply(&cc.egui_ctx);

        let settings = cc
            .storage
            .and_then(|s| eframe::get_value::<Settings>(s, SETTINGS_KEY))
            .unwrap_or_default();
        Self {
            document,
            mcp_url,
            tool: Tool::default(),
            selection: Selection::new(),
            plan: PlanView::new(),
            scene: SceneView::new(),
            palette: Palette::default(),
            settings,
            dialog: None,
            status: None,
            clipboard: Vec::new(),
            pending: None,
            allow_close: false,
            title: String::new(),
            plan_rect: egui::Rect::NOTHING,
            catalog_query: String::new(),
            renaming_variant: None,
        }
    }

    pub(crate) fn unit(&self) -> LengthUnit {
        self.settings.unit
    }

    pub(crate) fn set_status(&mut self, text: impl Into<String>) {
        self.status = Some((text.into(), Instant::now()));
    }

    /// Runs an edit on the document, reporting errors in the status bar.
    pub(crate) fn run(&mut self, action: impl FnOnce(&mut Document) -> CoreResult<()>) {
        let result = action(&mut self.document.write());
        if let Err(err) = result {
            self.set_status(format!("⚠ {err}"));
        }
    }

    pub(crate) fn set_tool(&mut self, tool: Tool) {
        if self.tool != tool {
            self.plan.cancel();
            self.tool = tool;
        }
    }

    // --- Files -------------------------------------------------------------

    fn is_modified(&self) -> bool {
        self.document.read().is_modified()
    }

    fn request(&mut self, action: Pending) {
        if self.is_modified() {
            self.dialog = Some(Dialog::ConfirmDiscard(action));
        } else {
            self.perform(action);
        }
    }

    pub(crate) fn perform(&mut self, action: Pending) {
        match action {
            Pending::New => {
                let mut doc = self.document.write();
                doc.load(Home::default());
                doc.set_path(None);
                doc.set_asset_dir(None);
                drop(doc);
                self.after_load();
            }
            Pending::Open(path) => {
                let path = path.or_else(|| {
                    rfd::FileDialog::new()
                        .add_filter(
                            "Projetos (3D New Era AI, Sweet Home 3D)",
                            &[newera_core::PROJECT_EXTENSION, "sh3d"],
                        )
                        .add_filter("3D New Era AI", &[newera_core::PROJECT_EXTENSION])
                        .add_filter("Sweet Home 3D", &["sh3d"])
                        .pick_file()
                });
                if let Some(path) = path {
                    self.open_path(&path);
                }
            }
            Pending::Quit => {
                self.allow_close = true;
                self.pending = Some(Pending::Quit);
            }
        }
    }

    pub(crate) fn open_path(&mut self, path: &Path) {
        let loaded = newera_sh3d::open_file(&mut self.document.write(), path);
        match loaded {
            Ok(opened) => {
                self.remember(path);
                self.after_load();
                let mut status = if opened.imported {
                    format!(
                        "Importado de {} — salve como projeto para manter tudo num arquivo",
                        path.display()
                    )
                } else {
                    format!("Aberto: {}", path.display())
                };
                if !opened.warnings.is_empty() {
                    let _ = write!(status, " · {} aviso(s)", opened.warnings.len());
                    for warning in &opened.warnings {
                        tracing::warn!("import: {warning}");
                    }
                }
                self.set_status(status);
            }
            Err(err) => self.set_status(format!(
                "⚠ Não foi possível abrir {}: {err}",
                path.display()
            )),
        }
    }

    fn after_load(&mut self) {
        self.selection.clear();
        self.plan.cancel();
        self.plan.request_fit();
        self.scene.request_frame();
    }

    /// Saves to the current path, or asks for one. Returns true when saved.
    pub(crate) fn save(&mut self, save_as: bool) -> bool {
        let current = self.document.read().path().map(Path::to_path_buf);
        let path = match current {
            Some(path) if !save_as => Some(path),
            _ => {
                let name = self.document.read().home().name.clone();
                rfd::FileDialog::new()
                    .add_filter("3D New Era AI", &[newera_core::PROJECT_EXTENSION])
                    .set_file_name(format!("{name}.{}", newera_core::PROJECT_EXTENSION))
                    .save_file()
            }
        };
        let Some(mut path) = path else { return false };
        if path.extension().is_none() {
            path.set_extension(newera_core::PROJECT_EXTENSION);
        }
        let saved = newera_core::save_project(&self.document.read(), &path);
        match saved {
            Ok(()) => {
                self.document.write().mark_saved(&path);
                self.remember(&path);
                self.set_status(format!("Salvo em {}", path.display()));
                true
            }
            Err(err) => {
                self.set_status(format!("⚠ Não foi possível salvar: {err}"));
                false
            }
        }
    }

    fn remember(&mut self, path: &Path) {
        self.settings.recent.retain(|p| p != path);
        self.settings.recent.insert(0, path.to_path_buf());
        self.settings.recent.truncate(8);
    }

    fn export(&mut self, svg: bool) {
        let name = self.document.read().home().name.clone();
        let ext = if svg { "svg" } else { "png" };
        let Some(path) = rfd::FileDialog::new()
            .add_filter(ext.to_uppercase(), &[ext])
            .set_file_name(format!("{name}.{ext}"))
            .save_file()
        else {
            return;
        };
        let doc = self.document.read();
        let scene = plan_scene(
            &doc.home().level_view(doc.home().current_level()),
            &SceneOptions {
                unit: self.settings.unit,
                show_background: true,
                ..SceneOptions::default()
            },
        );
        let assets = doc.asset_dir();
        drop(doc);
        let bytes = if svg {
            Ok(to_svg(&scene, &SvgOptions::default()).into_bytes())
        } else {
            let load = |p: &str| {
                image::open(newera_core::resolve_asset(assets.as_deref(), p))
                    .ok()
                    .map(|i| i.to_rgba8())
            };
            let options = RenderOptions {
                width: 2400,
                height: 1800,
                grid: false,
                ..RenderOptions::default()
            };
            render_png(&scene, &options, &load).map_err(|e| e.to_string())
        };
        match bytes.and_then(|b| std::fs::write(&path, b).map_err(|e| e.to_string())) {
            Ok(()) => self.set_status(format!("Planta exportada para {}", path.display())),
            Err(err) => self.set_status(format!("⚠ Falha ao exportar: {err}")),
        }
    }

    /// Imports an OBJ/glTF file as a piece at its natural size, placed at
    /// the middle of the plan view.
    pub(crate) fn import_model(&mut self) {
        let Some(path) = rfd::FileDialog::new()
            .add_filter("Modelos 3D", &["obj", "gltf", "glb"])
            .pick_file()
        else {
            return;
        };
        match newera_catalog::load_model(&path) {
            Ok(model) => {
                let at = self.plan.view_center();
                let name = path
                    .file_stem()
                    .map_or_else(|| "Modelo".to_owned(), |s| s.to_string_lossy().into_owned());
                let mut placed = None;
                self.run(|doc| {
                    let piece = newera_core::Furniture {
                        id: doc.new_furniture_id(),
                        catalog: "imported".to_owned(),
                        name,
                        position: at,
                        elevation: 0.0,
                        angle: 0.0,
                        width: model.size[0],
                        depth: model.size[1],
                        height: model.size[2],
                        mirrored: false,
                        color: None,
                        opening: None,
                        model: Some(path.display().to_string()),
                        visible: true,
                        level: None,
                        ..Default::default()
                    };
                    placed = Some(piece.id);
                    doc.execute(Command::insert(piece))
                });
                if let Some(id) = placed {
                    self.selection = std::iter::once(ElementId::from(id)).collect();
                    self.set_tool(Tool::Select);
                    self.set_status("Modelo importado. Ajuste medidas com Enter ou pelas alças.");
                }
            }
            Err(err) => self.set_status(format!("⚠ {err}")),
        }
    }

    pub(crate) fn import_background(&mut self) {
        let Some(path) = rfd::FileDialog::new()
            .add_filter("Imagens", &["png", "jpg", "jpeg", "webp", "bmp"])
            .pick_file()
        else {
            return;
        };
        let size = match image::image_dimensions(&path) {
            Ok((w, h)) => [w, h],
            Err(err) => return self.set_status(format!("⚠ Imagem inválida: {err}")),
        };
        // Start at a plausible scale: fit the image width to ~15 m.
        let cm_per_px = 1500.0 / f64::from(size[0]);
        self.run(|doc| {
            doc.execute(Command::SetBackground {
                background: Some(newera_core::BackgroundImage {
                    path: path.display().to_string(),
                    size_px: size,
                    cm_per_px,
                    offset: Point2::new(0.0, 0.0),
                    opacity: 0.5,
                    visible: true,
                }),
            })
        });
        self.plan.request_fit();
        self.set_tool(Tool::Calibrate);
        self.set_status(
            "Imagem importada. Clique em dois pontos de medida conhecida para calibrar; arraste para posicionar.",
        );
    }

    // --- Selection & clipboard -------------------------------------------------

    fn selected_elements(&self) -> Vec<Element> {
        let doc = self.document.read();
        self.selection
            .iter()
            .filter_map(|id| doc.home().element(*id))
            .collect()
    }

    fn delete_selection(&mut self) {
        if self.selection.is_empty() {
            return;
        }
        let ids: Vec<ElementId> = std::mem::take(&mut self.selection).into_iter().collect();
        self.run(|doc| {
            doc.execute(Command::Batch {
                commands: ids.into_iter().map(Command::remove).collect(),
            })
        });
    }

    fn copy(&mut self) {
        self.clipboard = self.selected_elements();
    }

    fn paste(&mut self, elements: Vec<Element>) {
        if elements.is_empty() {
            return;
        }
        let mut new_ids = Vec::new();
        self.run(|doc| {
            let commands = elements
                .into_iter()
                .map(|element| {
                    let moved = shift_with_new_id(doc, element, PASTE_OFFSET);
                    new_ids.push(moved.id());
                    Command::insert(moved)
                })
                .collect();
            doc.execute(Command::Batch { commands })
        });
        self.selection = new_ids.into_iter().collect();
    }

    fn select_all(&mut self) {
        let doc = self.document.read();
        let view = doc.home().level_view(doc.home().current_level());
        self.selection = view
            .elements()
            .filter(|e| !matches!(e, Element::Level(_)))
            .map(|e| e.id())
            .collect();
    }

    fn nudge(&mut self, dx: f64, dy: f64) {
        let ids: Vec<ElementId> = self.selection.iter().copied().collect();
        if !ids.is_empty() {
            self.run(|doc| ops::translate(doc, &ids, dx, dy, true));
        }
    }

    pub(crate) fn open_modify(&mut self, ids: &[ElementId]) {
        let elements: Vec<Element> = {
            let doc = self.document.read();
            ids.iter()
                .filter_map(|id| doc.home().element(*id))
                .collect()
        };
        self.dialog = Dialog::modify(&elements);
    }

    pub(crate) fn set_dialog(&mut self, dialog: Dialog) {
        self.dialog = Some(dialog);
    }

    /// Selection and in-progress drawing belong to the variant that was active.
    pub(crate) fn after_variant_change(&mut self) {
        self.selection.clear();
        self.plan.cancel();
    }

    pub(crate) fn open_compare(&mut self) {
        let doc = self.document.read();
        let rows = doc
            .variants()
            .enumerate()
            .map(|(i, v)| crate::tabs::stats(&v.name, i == doc.active_variant(), v.home()))
            .collect();
        drop(doc);
        self.dialog = Some(Dialog::Compare(rows));
    }

    fn cycle_variant(&mut self) {
        self.run(|doc| {
            let next = (doc.active_variant() + 1) % doc.variant_count();
            doc.switch_variant(next)
        });
        self.after_variant_change();
    }

    fn open_home_settings(&mut self) {
        let home = self.document.read().home().clone();
        self.dialog = Some(Dialog::HomeSettings {
            name: home.name,
            compass: home.compass,
        });
    }

    // --- Input -----------------------------------------------------------------

    fn shortcuts(&mut self, ctx: &egui::Context) {
        let cmd = Modifiers::COMMAND;
        let cmd_shift = Modifiers::COMMAND.plus(Modifiers::SHIFT);
        let pressed = |m: Modifiers, k: Key| {
            ctx.input_mut(|i| i.consume_shortcut(&KeyboardShortcut::new(m, k)))
        };

        if pressed(cmd, Key::T) {
            self.run(|doc| {
                doc.add_variant(None, true);
                Ok(())
            });
            self.after_variant_change();
        }
        if pressed(cmd, Key::Tab) {
            self.cycle_variant();
        }
        if pressed(cmd, Key::N) {
            self.request(Pending::New);
        }
        if pressed(cmd, Key::O) {
            self.request(Pending::Open(None));
        }
        if pressed(cmd_shift, Key::S) {
            self.save(true);
        } else if pressed(cmd, Key::S) {
            self.save(false);
        }
        if ctx.egui_wants_keyboard_input() || self.dialog.is_some() {
            return;
        }
        if pressed(cmd_shift, Key::Z) || pressed(cmd, Key::Y) {
            self.run(Document::redo);
        } else if pressed(cmd, Key::Z) {
            self.run(Document::undo);
        }
        if pressed(cmd, Key::A) {
            self.select_all();
        }
        if pressed(cmd, Key::C) {
            self.copy();
        }
        if pressed(cmd, Key::X) {
            self.copy();
            self.delete_selection();
        }
        if pressed(cmd, Key::V) {
            self.paste(self.clipboard.clone());
        }
        if pressed(cmd, Key::D) {
            let selected = self.selected_elements();
            self.paste(selected);
        }
        if pressed(cmd, Key::Equals) || pressed(cmd, Key::Plus) {
            self.plan.zoom_by(self.plan_rect, 1.25);
        }
        if pressed(cmd, Key::Minus) {
            self.plan.zoom_by(self.plan_rect, 0.8);
        }
        if pressed(cmd, Key::Num0) {
            self.plan.request_fit();
        }

        let drawing_walls = self.plan.accepts_length_input();
        if ctx.input(|i| i.key_pressed(Key::Escape)) {
            if self.plan.is_drawing() {
                self.plan.cancel();
            } else if self.tool != Tool::Select {
                self.set_tool(Tool::Select);
            } else {
                self.selection.clear();
            }
        }
        if !drawing_walls {
            for (key, tool) in [
                (Key::V, Tool::Select),
                (Key::H, Tool::Pan),
                (Key::W, Tool::Walls),
                (Key::R, Tool::Rooms),
                (Key::D, Tool::Dimensions),
                (Key::T, Tool::Labels),
            ] {
                if ctx.input(|i| i.key_pressed(key) && i.modifiers.is_none()) {
                    self.set_tool(tool);
                }
            }
            if ctx.input(|i| i.key_pressed(Key::Delete) || i.key_pressed(Key::Backspace)) {
                self.delete_selection();
            }
        }
        // Consume the Enter so the dialog it opens doesn't read it as "OK".
        if self.tool == Tool::Select
            && !self.selection.is_empty()
            && ctx.input_mut(|i| i.consume_key(Modifiers::NONE, Key::Enter))
        {
            self.open_modify(&self.selection.iter().copied().collect::<Vec<_>>());
        }
        let step = if ctx.input(|i| i.modifiers.shift) {
            10.0
        } else {
            1.0
        };
        for (key, dx, dy) in [
            (Key::ArrowLeft, -step, 0.0),
            (Key::ArrowRight, step, 0.0),
            (Key::ArrowUp, 0.0, -step),
            (Key::ArrowDown, 0.0, step),
        ] {
            if ctx.input(|i| i.key_pressed(key)) && !self.selection.is_empty() {
                self.nudge(dx, dy);
            }
        }
    }

    // --- Layout ----------------------------------------------------------------

    fn menu_bar(&mut self, ui: &mut egui::Ui) {
        egui::MenuBar::new().ui(ui, |ui| {
            ui.menu_button("Arquivo", |ui| {
                if menu_item(ui, icon::FILE_PLUS, "Novo", "Ctrl+N", true) {
                    self.request(Pending::New);
                }
                if menu_item(ui, icon::FOLDER_OPEN, "Abrir…", "Ctrl+O", true) {
                    self.request(Pending::Open(None));
                }
                let recent = self.settings.recent.clone();
                ui.add_enabled_ui(!recent.is_empty(), |ui| {
                    ui.menu_button(
                        format!("{} Recentes", icon::CLOCK_COUNTER_CLOCKWISE),
                        |ui| {
                            for path in recent {
                                let label = path.file_name().map_or_else(
                                    || path.display().to_string(),
                                    |n| n.to_string_lossy().into_owned(),
                                );
                                if ui
                                    .button(label)
                                    .on_hover_text(path.display().to_string())
                                    .clicked()
                                {
                                    self.request(Pending::Open(Some(path)));
                                }
                            }
                        },
                    );
                });
                ui.separator();
                if menu_item(ui, icon::FLOPPY_DISK, "Salvar", "Ctrl+S", true) {
                    self.save(false);
                }
                if menu_item(
                    ui,
                    icon::FLOPPY_DISK_BACK,
                    "Salvar como…",
                    "Ctrl+Shift+S",
                    true,
                ) {
                    self.save(true);
                }
                ui.separator();
                ui.menu_button(format!("{} Exportar planta", icon::EXPORT), |ui| {
                    if ui.button("SVG em escala real…").clicked() {
                        self.export(true);
                    }
                    if ui.button("PNG…").clicked() {
                        self.export(false);
                    }
                });
                ui.separator();
                if menu_item(ui, icon::SIGN_OUT, "Sair", "", true) {
                    ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
                }
            });
            ui.menu_button("Editar", |ui| {
                let (can_undo, can_redo) = {
                    let doc = self.document.read();
                    (doc.can_undo(), doc.can_redo())
                };
                let has_selection = !self.selection.is_empty();
                if menu_item(
                    ui,
                    icon::ARROW_COUNTER_CLOCKWISE,
                    "Desfazer",
                    "Ctrl+Z",
                    can_undo,
                ) {
                    self.run(Document::undo);
                }
                if menu_item(
                    ui,
                    icon::ARROW_CLOCKWISE,
                    "Refazer",
                    "Ctrl+Shift+Z",
                    can_redo,
                ) {
                    self.run(Document::redo);
                }
                ui.separator();
                if menu_item(ui, icon::SCISSORS, "Recortar", "Ctrl+X", has_selection) {
                    self.copy();
                    self.delete_selection();
                }
                if menu_item(ui, icon::COPY, "Copiar", "Ctrl+C", has_selection) {
                    self.copy();
                }
                if menu_item(
                    ui,
                    icon::CLIPBOARD,
                    "Colar",
                    "Ctrl+V",
                    !self.clipboard.is_empty(),
                ) {
                    self.paste(self.clipboard.clone());
                }
                if menu_item(ui, icon::COPY_SIMPLE, "Duplicar", "Ctrl+D", has_selection) {
                    let selected = self.selected_elements();
                    self.paste(selected);
                }
                if menu_item(ui, icon::TRASH, "Excluir", "Del", has_selection) {
                    self.delete_selection();
                }
                ui.separator();
                if menu_item(ui, icon::SELECTION_ALL, "Selecionar tudo", "Ctrl+A", true) {
                    self.select_all();
                }
                if menu_item(
                    ui,
                    icon::PENCIL_SIMPLE,
                    "Modificar…",
                    "Enter",
                    has_selection,
                ) {
                    self.open_modify(&self.selection.iter().copied().collect::<Vec<_>>());
                }
            });
            ui.menu_button("Planta", |ui| {
                for (tool, glyph, label, key) in TOOLS {
                    if ui
                        .add(
                            egui::Button::selectable(self.tool == tool, format!("{glyph} {label}"))
                                .shortcut_text(key),
                        )
                        .clicked()
                    {
                        self.set_tool(tool);
                    }
                }
                ui.separator();
                let single_wall = match self.selection.iter().collect::<Vec<_>>().as_slice() {
                    [ElementId::Wall(id)] => Some(*id),
                    _ => None,
                };
                if menu_item(
                    ui,
                    icon::SCISSORS,
                    "Dividir parede ao meio",
                    "",
                    single_wall.is_some(),
                ) && let Some(id) = single_wall
                {
                    self.run(|doc| ops::split_wall(doc, id, 0.5).map(|_| ()));
                }
                let walls: Vec<_> = self
                    .selection
                    .iter()
                    .filter_map(|id| match id {
                        ElementId::Wall(w) => Some(*w),
                        _ => None,
                    })
                    .collect();
                if menu_item(
                    ui,
                    icon::RULER,
                    "Cotar paredes selecionadas",
                    "",
                    !walls.is_empty(),
                ) {
                    self.run(|doc| {
                        let mut commands = Vec::new();
                        for id in &walls {
                            commands.push(Command::insert(ops::wall_dimension(doc, *id, 40.0)?));
                        }
                        doc.execute(Command::Batch { commands })
                    });
                }
                ui.separator();
                ui.menu_button(format!("{} Imagem de fundo", icon::IMAGE), |ui| {
                    if ui.button("Importar…").clicked() {
                        self.import_background();
                    }
                    let background = self.document.read().home().background.clone();
                    if ui
                        .add_enabled(
                            background.is_some(),
                            egui::Button::new("Calibrar e posicionar"),
                        )
                        .clicked()
                    {
                        self.set_tool(Tool::Calibrate);
                    }
                    if ui
                        .add_enabled(background.is_some(), egui::Button::new("Ajustes…"))
                        .clicked()
                    {
                        self.dialog = background.map(Dialog::Background);
                    }
                });
                if menu_item(ui, icon::CUBE, "Importar modelo 3D…", "", true) {
                    self.import_model();
                }
                if menu_item(ui, icon::COMPASS, "Casa e bússola…", "", true) {
                    self.open_home_settings();
                }
            });
            ui.menu_button("Ver", |ui| {
                if menu_item(ui, icon::CORNERS_OUT, "Enquadrar planta", "Ctrl+0", true) {
                    self.plan.request_fit();
                }
                if menu_item(ui, icon::CUBE, "Enquadrar 3D", "", true) {
                    self.scene.request_frame();
                }
                ui.separator();
                ui.label("Unidade");
                for unit in LengthUnit::ALL {
                    ui.radio_value(&mut self.settings.unit, unit, unit.label());
                }
            });
            ui.menu_button("Ajuda", |ui| {
                if menu_item(ui, icon::KEYBOARD, "Atalhos e ferramentas", "", true) {
                    self.dialog = Some(Dialog::Help);
                }
            });
        });
    }

    fn toolbar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 2.0;
            let big = |text: &str| RichText::new(text).size(18.0);
            if ui
                .button(big(icon::FILE_PLUS))
                .on_hover_text("Novo (Ctrl+N)")
                .clicked()
            {
                self.request(Pending::New);
            }
            if ui
                .button(big(icon::FOLDER_OPEN))
                .on_hover_text("Abrir (Ctrl+O)")
                .clicked()
            {
                self.request(Pending::Open(None));
            }
            if ui
                .button(big(icon::FLOPPY_DISK))
                .on_hover_text("Salvar (Ctrl+S)")
                .clicked()
            {
                self.save(false);
            }
            ui.separator();
            let (can_undo, can_redo) = {
                let doc = self.document.read();
                (doc.can_undo(), doc.can_redo())
            };
            if ui
                .add_enabled(
                    can_undo,
                    egui::Button::new(big(icon::ARROW_COUNTER_CLOCKWISE)),
                )
                .on_hover_text("Desfazer (Ctrl+Z)")
                .clicked()
            {
                self.run(Document::undo);
            }
            if ui
                .add_enabled(can_redo, egui::Button::new(big(icon::ARROW_CLOCKWISE)))
                .on_hover_text("Refazer (Ctrl+Shift+Z)")
                .clicked()
            {
                self.run(Document::redo);
            }
            ui.separator();
            for (tool, glyph, label, key) in TOOLS {
                if ui
                    .add(egui::Button::selectable(self.tool == tool, big(glyph)))
                    .on_hover_text(format!("{label} ({key})"))
                    .clicked()
                {
                    self.set_tool(tool);
                }
            }
            ui.separator();
            if ui
                .button(big(icon::IMAGE))
                .on_hover_text("Importar imagem de fundo")
                .clicked()
            {
                self.import_background();
            }
            if ui
                .button(big(icon::COMPASS))
                .on_hover_text("Casa e bússola")
                .clicked()
            {
                self.open_home_settings();
            }
            ui.separator();
            if ui
                .button(big(icon::MAGNIFYING_GLASS_PLUS))
                .on_hover_text("Aproximar (Ctrl +)")
                .clicked()
            {
                self.plan.zoom_by(self.plan_rect, 1.25);
            }
            if ui
                .button(big(icon::MAGNIFYING_GLASS_MINUS))
                .on_hover_text("Afastar (Ctrl -)")
                .clicked()
            {
                self.plan.zoom_by(self.plan_rect, 0.8);
            }
            if ui
                .button(big(icon::CORNERS_OUT))
                .on_hover_text("Enquadrar (Ctrl+0)")
                .clicked()
            {
                self.plan.request_fit();
            }
        });
    }

    fn status_bar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            match &self.mcp_url {
                Some(url) => ui.label(
                    RichText::new(format!("{} MCP {url}", icon::ROBOT))
                        .color(ui.visuals().hyperlink_color),
                ),
                None => ui.weak(format!("{} MCP desligado", icon::ROBOT)),
            };
            ui.separator();
            if let Some(p) = self.plan.cursor() {
                let unit = self.settings.unit;
                ui.monospace(format!(
                    "x {}  y {}",
                    unit.format_length(p.x),
                    unit.format_length(p.y)
                ));
                ui.separator();
            }
            ui.weak(format!("{:.0}%", self.plan.zoom_percent()));
            ui.separator();
            ui.weak(tool_hint(self.tool));
            if let Some((text, at)) = &self.status {
                if at.elapsed() < Duration::from_secs(8) {
                    ui.separator();
                    let color = if text.starts_with('⚠') {
                        ui.visuals().warn_fg_color
                    } else {
                        ui.visuals().text_color()
                    };
                    ui.colored_label(color, text);
                } else {
                    self.status = None;
                }
            }
        });
    }

    fn update_title(&mut self, ctx: &egui::Context) {
        let doc = self.document.read();
        let file = doc.path().and_then(Path::file_name).map_or_else(
            || doc.home().name.clone(),
            |n| n.to_string_lossy().into_owned(),
        );
        let title = format!(
            "{}{} — 3D New Era AI",
            if doc.is_modified() { "● " } else { "" },
            file
        );
        drop(doc);
        if title != self.title {
            ctx.send_viewport_cmd(egui::ViewportCommand::Title(title.clone()));
            self.title = title;
        }
    }
}

pub(crate) const TOOLS: [(Tool, &str, &str, &str); 6] = [
    (Tool::Select, icon::CURSOR, "Selecionar", "V"),
    (Tool::Pan, icon::HAND, "Mover vista", "H"),
    (Tool::Walls, icon::WALL, "Criar paredes", "W"),
    (Tool::Rooms, icon::POLYGON, "Criar cômodos", "R"),
    (Tool::Dimensions, icon::RULER, "Criar cotas", "D"),
    (Tool::Labels, icon::TEXT_T, "Adicionar texto", "T"),
];

fn tool_hint(tool: Tool) -> &'static str {
    match tool {
        Tool::Select => {
            "Clique seleciona · Ctrl+clique soma · arraste move · alças editam · duplo clique modifica"
        }
        Tool::Pan => "Arraste para mover a vista",
        Tool::Walls => {
            "Clique encadeia paredes · digite o comprimento + Enter · Shift desliga o ímã · duplo clique encerra"
        }
        Tool::Rooms => {
            "Clique os cantos e duplo clique fecha · duplo clique dentro de paredes detecta o cômodo"
        }
        Tool::Dimensions => {
            "Clique início e fim, mova para afastar e clique · duplo clique numa parede cota a parede"
        }
        Tool::Labels => "Clique onde o texto deve ficar",
        Tool::Calibrate => {
            "Clique dois pontos de medida conhecida · arraste para posicionar a imagem"
        }
        Tool::Place(_) => {
            "Clique para posicionar · portas e janelas encaixam na parede mais próxima · Esc cancela"
        }
    }
}

fn menu_item(ui: &mut egui::Ui, glyph: &str, label: &str, shortcut: &str, enabled: bool) -> bool {
    ui.add_enabled(
        enabled,
        egui::Button::new(format!("{glyph}  {label}")).shortcut_text(shortcut),
    )
    .clicked()
}

/// A copy of `element` with a fresh id, moved by `offset` cm on both axes.
/// The copy lands on the level being edited, like any new element.
fn shift_with_new_id(doc: &mut Document, mut element: Element, offset: f64) -> Element {
    let shift = |p: Point2| Point2::new(p.x + offset, p.y + offset);
    element.set_level(None);
    match element {
        Element::Level(mut level) => {
            level.id = doc.new_level_id();
            level.name = format!("{} (cópia)", level.name);
            Element::Level(level)
        }
        Element::Wall(mut w) => {
            w.id = doc.new_wall_id();
            w.start = shift(w.start);
            w.end = shift(w.end);
            Element::Wall(w)
        }
        Element::Room(mut r) => {
            r.id = doc.new_room_id();
            r.points = r.points.into_iter().map(shift).collect();
            Element::Room(r)
        }
        Element::Dimension(mut d) => {
            d.id = doc.new_dimension_id();
            d.start = shift(d.start);
            d.end = shift(d.end);
            Element::Dimension(d)
        }
        Element::Label(mut l) => {
            l.id = doc.new_label_id();
            l.position = shift(l.position);
            Element::Label(l)
        }
        Element::Furniture(mut f) => {
            f.id = doc.new_furniture_id();
            f.translate(offset, offset);
            Element::Furniture(f)
        }
        Element::Polyline(mut p) => {
            p.id = doc.new_polyline_id();
            p.points = p.points.into_iter().map(shift).collect();
            Element::Polyline(p)
        }
    }
}

impl eframe::App for NewEraApp {
    fn ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();

        // Closing the window with unsaved changes asks first.
        if ctx.input(|i| i.viewport().close_requested()) && !self.allow_close && self.is_modified()
        {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.dialog = Some(Dialog::ConfirmDiscard(Pending::Quit));
        }
        if self.pending == Some(Pending::Quit) {
            self.pending = None;
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }

        self.shortcuts(&ctx);
        self.update_title(&ctx);

        egui::Panel::top("menu").show(ui, |ui| {
            self.menu_bar(ui);
            self.toolbar(ui);
        });
        egui::Panel::bottom("status").show(ui, |ui| self.status_bar(ui));
        egui::Panel::left("left")
            .resizable(true)
            .default_size(270.0)
            .show(ui, |ui| panels::left(self, ui));

        egui::CentralPanel::default()
            .frame(egui::Frame::NONE)
            .show(ui, |ui| {
                egui::Panel::top("variant_tabs")
                    .frame(egui::Frame::NONE.inner_margin(egui::Margin::symmetric(6, 3)))
                    .show(ui, |ui| crate::tabs::bar(self, ui));
                egui::Panel::top("plan")
                    .resizable(true)
                    .default_size(ui.available_height() / 2.0)
                    .frame(egui::Frame::NONE)
                    .show(ui, |ui| {
                        self.plan_rect = ui.available_rect_before_wrap();
                        let events = self.plan.ui(
                            ui,
                            PlanInput {
                                document: &self.document,
                                selection: &mut self.selection,
                                tool: self.tool,
                                unit: self.settings.unit,
                                palette: &self.palette,
                            },
                        );
                        for event in events {
                            match event {
                                PlanEvent::Modify(ids) => self.open_modify(&ids),
                                PlanEvent::NewLabel(at) => {
                                    self.dialog = Some(Dialog::NewLabel {
                                        at,
                                        text: String::new(),
                                    });
                                }
                                PlanEvent::Calibrate { a, b } => {
                                    self.dialog = Some(Dialog::Calibrate {
                                        a,
                                        b,
                                        distance: 100.0,
                                    });
                                }
                                PlanEvent::Placed(_) => self.tool = Tool::Select,
                                PlanEvent::Status(text) => self.set_status(text),
                            }
                        }
                    });
                egui::CentralPanel::default()
                    .frame(egui::Frame::NONE)
                    .show(ui, |ui| {
                        let (home, revision, project) = {
                            let doc = self.document.read();
                            (doc.home().clone(), doc.revision(), doc.asset_dir())
                        };
                        self.scene.ui(
                            ui,
                            frame.wgpu_render_state(),
                            &home,
                            revision,
                            &self.selection,
                            project.as_deref(),
                        );
                    });
            });

        // Selection may point to elements removed by undo or by an agent.
        {
            let doc = self.document.read();
            self.selection.retain(|id| doc.home().contains(*id));
        }

        if let Some(dialog) = self.dialog.take() {
            match crate::dialogs::show(self, &ctx, dialog) {
                DialogOutcome::Keep(dialog) => {
                    if self.dialog.is_none() {
                        self.dialog = Some(dialog);
                    }
                }
                DialogOutcome::Close => {}
            }
        }

        ctx.request_repaint_after(EXTERNAL_CHANGES_POLL);
    }

    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, SETTINGS_KEY, &self.settings);
    }
}

#[cfg(test)]
mod tests {
    //! Whole-app tests through `egui_kittest`: keyboard shortcuts, dialogs and
    //! their effect on the document.

    use eframe::egui::{Key, Modifiers};
    use egui_kittest::Harness;
    use egui_kittest::kittest::Queryable;
    use newera_core::{Point2, Wall, WallId};

    use super::*;

    fn app_with_wall() -> Harness<'static, NewEraApp> {
        let mut doc = Document::default();
        let wall = Wall::new(
            doc.new_wall_id(),
            Point2::new(0.0, 0.0),
            Point2::new(400.0, 0.0),
        );
        doc.execute(Command::insert(wall)).unwrap();
        let document = SharedDocument::new(doc);
        let mut harness = Harness::builder()
            .with_size(egui::vec2(1280.0, 800.0))
            .with_step_dt(1.0 / 60.0)
            .build_eframe(move |cc| NewEraApp::new(cc, document, None));
        harness.run_steps(5);
        harness
    }

    fn walls(h: &Harness<'_, NewEraApp>) -> Vec<Wall> {
        h.state().document.read().home().walls.clone()
    }

    #[test]
    fn delete_undo_and_duplicate_shortcuts() {
        let mut h = app_with_wall();
        h.state_mut().selection.insert(WallId(1).into());
        h.key_press(Key::Delete);
        h.run_steps(3);
        assert!(walls(&h).is_empty(), "Delete removes the selection");

        h.key_press_modifiers(Modifiers::COMMAND, Key::Z);
        h.run_steps(3);
        assert_eq!(walls(&h).len(), 1, "Ctrl+Z restores it");

        h.state_mut().selection.insert(WallId(1).into());
        h.key_press_modifiers(Modifiers::COMMAND, Key::D);
        h.run_steps(3);
        let all = walls(&h);
        assert_eq!(all.len(), 2, "Ctrl+D duplicates");
        assert_eq!(all[1].start, Point2::new(PASTE_OFFSET, PASTE_OFFSET));
        assert_eq!(
            h.state().selection.len(),
            1,
            "the copy becomes the selection"
        );
    }

    #[test]
    fn tool_keys_switch_tools() {
        let mut h = app_with_wall();
        for (key, tool) in [
            (Key::W, Tool::Walls),
            (Key::R, Tool::Rooms),
            (Key::D, Tool::Dimensions),
            (Key::V, Tool::Select),
        ] {
            h.key_press(key);
            h.run_steps(2);
            assert_eq!(h.state().tool, tool);
        }
    }

    #[test]
    fn enter_opens_the_modify_dialog_and_ok_applies_it() {
        let mut h = app_with_wall();
        h.state_mut().selection.insert(WallId(1).into());
        h.key_press(Key::Enter);
        h.run_steps(3);
        let Some(Dialog::ModifyWalls { ids, .. }) = h.state().dialog.clone() else {
            panic!("modify dialog should be open");
        };
        assert_eq!(ids, vec![WallId(1)]);

        // Change the draft as the drag values would, then press OK.
        if let Some(Dialog::ModifyWalls { thickness, arc, .. }) = &mut h.state_mut().dialog {
            *thickness = 30.0;
            *arc = 45.0;
        }
        h.run_steps(2);
        h.get_by_label_contains("OK").click();
        h.run_steps(5);
        let wall = walls(&h).remove(0);
        assert!((wall.thickness - 30.0).abs() < 1e-9);
        assert_eq!(wall.arc_extent, Some(45.0));
        assert!(h.state().dialog.is_none(), "dialog closes");
    }

    #[test]
    fn wall_dialog_picks_a_type_and_a_side_finish_by_clicking() {
        let mut h = app_with_wall();
        h.state_mut().selection.insert(WallId(1).into());
        h.key_press(Key::Enter);
        h.run_steps(3);
        h.get_by_value("Personalizada").click();
        h.run_steps(3);
        h.get_by_label_contains("Drywall 95 mm").click();
        h.run_steps(3);
        // Two finish combos (left, right); the first is the left side.
        h.get_all_by_value("Sem acabamento")
            .next()
            .expect("left finish")
            .click();
        h.run_steps(3);
        h.get_by_label("Tijolo aparente").click();
        h.run_steps(3);
        h.get_by_label_contains("OK").click();
        h.run_steps(5);
        let wall = walls(&h).remove(0);
        assert_eq!(wall.wall_type.as_deref(), Some("drywall-95"));
        assert!((wall.thickness - 9.5).abs() < 1e-9, "{}", wall.thickness);
        assert_eq!(
            wall.left_side,
            Some(newera_core::Material::pattern(newera_core::Pattern::Brick))
        );
        assert!(wall.right_side.is_none());
    }

    #[test]
    fn modify_room_name_by_typing() {
        let mut h = app_with_wall();
        let room_id = {
            let mut doc = h.state().document.write();
            let room = newera_core::Room::new(
                doc.new_room_id(),
                "",
                vec![
                    Point2::new(0.0, 0.0),
                    Point2::new(300.0, 0.0),
                    Point2::new(300.0, 300.0),
                ],
            );
            let id = room.id;
            doc.execute(Command::insert(room)).unwrap();
            id
        };
        h.run_steps(2);
        h.state_mut().open_modify(&[room_id.into()]);
        h.run_steps(3);
        // The catalog search is also a text input; the dialog's comes last.
        h.get_all_by_role(egui::accesskit::Role::TextInput)
            .last()
            .expect("room name field")
            .type_text("Suíte");
        h.run_steps(3);
        h.get_by_label_contains("OK").click();
        h.run_steps(5);
        assert_eq!(
            h.state().document.read().home().room(room_id).unwrap().name,
            "Suíte"
        );
    }
}

#[cfg(test)]
mod screenshots {
    //! Renders the real UI headlessly for visual review:
    //! `cargo test -p newera-app screenshots -- --ignored`, images land in
    //! `target/screenshots/`.

    use egui_kittest::Harness;
    use newera_core::{Point2, Room, Wall};

    use super::*;

    fn render(name: &str, setup: impl FnOnce(&mut NewEraApp)) {
        let mut doc = Document::default();
        let pts = [(0.0, 0.0), (600.0, 0.0), (600.0, 400.0), (0.0, 400.0)];
        for i in 0..4 {
            let (a, b) = (pts[i], pts[(i + 1) % 4]);
            let wall = Wall::new(
                doc.new_wall_id(),
                Point2::new(a.0, a.1),
                Point2::new(b.0, b.1),
            );
            doc.execute(Command::insert(wall)).unwrap();
        }
        let room = Room::new(
            doc.new_room_id(),
            "Sala",
            pts.iter().map(|&(x, y)| Point2::new(x, y)).collect(),
        );
        doc.execute(Command::insert(room)).unwrap();
        let document = SharedDocument::new(doc);
        let mut harness = Harness::builder()
            .with_size(egui::vec2(1280.0, 800.0))
            .with_step_dt(1.0 / 60.0)
            .wgpu()
            .build_eframe(move |cc| {
                NewEraApp::new(cc, document, Some("http://127.0.0.1:7878/mcp".into()))
            });
        harness.run_steps(5);
        setup(harness.state_mut());
        harness.run_steps(10);
        let image = harness.render().expect("render");
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/screenshots");
        std::fs::create_dir_all(&dir).unwrap();
        image.save(dir.join(format!("{name}.png"))).unwrap();
    }

    #[test]
    #[ignore = "visual review; needs a GPU"]
    fn dialogs() {
        render("modify-wall", |app| {
            app.selection.insert(newera_core::WallId(1).into());
            app.open_modify(&[newera_core::WallId(1).into()]);
        });
        render("home-settings", NewEraApp::open_home_settings);
        render("help", |app| app.dialog = Some(Dialog::Help));
        render("confirm-discard", |app| {
            app.dialog = Some(Dialog::ConfirmDiscard(Pending::New));
        });
        render("calibrate", |app| {
            app.dialog = Some(Dialog::Calibrate {
                a: Point2::new(0.0, 0.0),
                b: Point2::new(600.0, 0.0),
                distance: 500.0,
            });
        });
    }
}

#[cfg(test)]
mod furniture_screenshots {
    use egui_kittest::Harness;
    use newera_core::{Point2, Room, Wall, align_to_wall};

    use super::*;

    fn furnished() -> Document {
        let mut doc = Document::default();
        let pts = [(0.0, 0.0), (700.0, 0.0), (700.0, 500.0), (0.0, 500.0)];
        let mut walls = Vec::new();
        for i in 0..4 {
            let (a, b) = (pts[i], pts[(i + 1) % 4]);
            walls.push(Wall {
                thickness: 20.0,
                ..Wall::new(
                    doc.new_wall_id(),
                    Point2::new(a.0, a.1),
                    Point2::new(b.0, b.1),
                )
            });
        }
        let room = Room::new(
            doc.new_room_id(),
            "Sala",
            pts.iter().map(|&(x, y)| Point2::new(x, y)).collect(),
        );
        let mut commands: Vec<Command> = walls.iter().cloned().map(Command::insert).collect();
        commands.push(Command::insert(room));
        let mut add = |doc: &mut Document,
                       id: &str,
                       at: (f64, f64),
                       angle: f64,
                       wall: Option<(usize, f64)>| {
            let mut piece = newera_catalog::find(id)
                .unwrap()
                .instantiate(doc.new_furniture_id(), Point2::new(at.0, at.1));
            piece.angle = angle;
            if let Some((w, along)) = wall {
                align_to_wall(&mut piece, &walls[w], along);
            }
            commands.push(Command::insert(piece));
        };
        add(&mut doc, "door", (0.0, 0.0), 0.0, Some((2, 150.0)));
        add(&mut doc, "window", (0.0, 0.0), 0.0, Some((0, 350.0)));
        add(&mut doc, "sofa-3", (350.0, 400.0), 180.0, None);
        add(&mut doc, "coffee-table", (350.0, 280.0), 0.0, None);
        add(&mut doc, "armchair", (150.0, 250.0), 90.0, None);
        add(&mut doc, "plant", (620.0, 80.0), 0.0, None);
        add(&mut doc, "bookcase", (60.0, 30.0), 0.0, None);
        doc.execute(Command::Batch { commands }).unwrap();
        doc
    }

    fn render(name: &str, setup: impl FnOnce(&mut NewEraApp)) {
        let document = SharedDocument::new(furnished());
        let mut harness = Harness::builder()
            .with_size(egui::vec2(1400.0, 860.0))
            .with_step_dt(1.0 / 60.0)
            .wgpu()
            .build_eframe(move |cc| NewEraApp::new(cc, document, None));
        harness.run_steps(5);
        setup(harness.state_mut());
        harness.run_steps(12);
        let image = harness.render().expect("render");
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/screenshots");
        std::fs::create_dir_all(&dir).unwrap();
        image.save(dir.join(format!("{name}.png"))).unwrap();
    }

    #[test]
    #[ignore = "visual review; needs a GPU"]
    fn furniture() {
        render("furniture-selected", |app| {
            app.selection.insert(newera_core::FurnitureId(7).into());
        });
        render("furniture-dialog", |app| {
            app.open_modify(&[newera_core::FurnitureId(7).into()]);
        });
        render("catalog-search", |app| app.catalog_query = "cama".into());
        render("catalog-place", |app| {
            app.set_tool(Tool::Place("bed-double"));
        });
    }
}

#[cfg(test)]
mod variant_tests {
    use eframe::egui::{Key, Modifiers};
    use egui_kittest::Harness;
    use egui_kittest::kittest::Queryable;
    use newera_core::{Point2, Wall};

    use super::*;

    fn app() -> Harness<'static, NewEraApp> {
        let mut doc = Document::default();
        let wall = Wall::new(
            doc.new_wall_id(),
            Point2::new(0.0, 0.0),
            Point2::new(400.0, 0.0),
        );
        doc.execute(Command::insert(wall)).unwrap();
        let document = SharedDocument::new(doc);
        let mut h = Harness::builder()
            .with_size(egui::vec2(1280.0, 800.0))
            .with_step_dt(1.0 / 60.0)
            .build_eframe(move |cc| NewEraApp::new(cc, document, None));
        h.run_steps(5);
        h
    }

    #[test]
    fn duplicate_edit_and_switch_tabs() {
        let mut h = app();
        h.key_press_modifiers(Modifiers::COMMAND, Key::T);
        h.run_steps(4);
        assert_eq!(h.state().document.read().variant_count(), 2);
        assert_eq!(h.state().document.read().active_variant(), 1);

        // Edit the copy.
        h.state()
            .document
            .write()
            .execute(Command::remove(newera_core::WallId(1)))
            .unwrap();
        h.run_steps(3);

        // Click the first tab: the original still has its wall.
        // Exact label: "Versão 1 (cópia)" also contains "Versão 1".
        h.get_by_label(&format!("{} Versão 1", icon::FILE_TEXT))
            .click();
        h.run_steps(4);
        let doc = h.state().document.read();
        assert_eq!(doc.active_variant(), 0);
        assert_eq!(doc.home().walls.len(), 1);
        drop(doc);

        h.key_press_modifiers(Modifiers::COMMAND, Key::Tab);
        h.run_steps(3);
        assert_eq!(h.state().document.read().active_variant(), 1);
        assert!(h.state().document.read().home().walls.is_empty());
    }

    #[test]
    fn add_level_button_creates_a_storey_and_scopes_select_all() {
        let mut h = app();
        h.get_by_label(&format!("{} Andar", icon::PLUS)).click();
        h.run_steps(4);
        {
            let doc = h.state().document.read();
            assert_eq!(doc.home().levels.len(), 2, "ground + first floor");
            assert_ne!(
                doc.home().current_level(),
                Some(doc.home().base_level().unwrap())
            );
        }
        // The upper storey is empty: select-all picks nothing from below.
        h.key_press_modifiers(Modifiers::COMMAND, Key::A);
        h.run_steps(2);
        assert!(h.state().selection.is_empty(), "{:?}", h.state().selection);

        let ground = h.state().document.read().home().base_level().unwrap();
        h.state().document.write().select_level(Some(ground));
        h.run_steps(3);
        h.key_press_modifiers(Modifiers::COMMAND, Key::A);
        h.run_steps(2);
        assert_eq!(
            h.state().selection,
            [ElementId::Wall(newera_core::WallId(1))].into()
        );
    }

    #[test]
    fn compare_dialog_lists_every_version() {
        let mut h = app();
        h.key_press_modifiers(Modifiers::COMMAND, Key::T);
        h.run_steps(3);
        h.get_by_label_contains("Comparar").click();
        h.run_steps(4);
        match &h.state().dialog {
            Some(Dialog::Compare(rows)) => {
                assert_eq!(rows.len(), 2);
                assert!(rows[1].active);
                assert_eq!(rows[0].walls, 1);
            }
            other => panic!("compare dialog should be open, got {other:?}"),
        }
    }

    #[test]
    fn closing_a_version_asks_first() {
        let mut h = app();
        h.key_press_modifiers(Modifiers::COMMAND, Key::T);
        h.run_steps(3);
        h.state_mut().set_dialog(Dialog::ConfirmCloseVariant {
            index: 1,
            name: "x".into(),
        });
        h.run_steps(3);
        h.get_by_label_contains("Fechar versão").click();
        h.run_steps(4);
        assert_eq!(h.state().document.read().variant_count(), 1);
    }
}
