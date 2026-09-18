//! Connecting an AI to this window — and showing, live, that it worked.
//!
//! The MCP server runs inside the editor, but the connection is made in
//! another program's settings file. Someone who has just installed this has no
//! way of telling a working setup from a typo: nothing on screen changes. So
//! this module is two things — the address and the exact snippet each client
//! wants, and the proof: who said hello, when, and the tools they are calling
//! as they call them.

use eframe::egui::{self, RichText};
use egui_phosphor::regular as icon;

use crate::app::NewEraApp;

/// One AI client and what it needs pasted where.
pub(crate) struct Client {
    pub(crate) label: &'static str,
    /// Where the snippet goes, in the person's language.
    pub(crate) place: &'static str,
    /// What to paste, with this window's address already in it.
    pub(crate) code: String,
    pub(crate) note: &'static str,
    /// The same thing as a command this window can run itself, when the
    /// client has a command line: `(program, arguments)`.
    #[cfg_attr(
        target_arch = "wasm32",
        allow(dead_code, reason = "a browser tab runs no command lines")
    )]
    pub(crate) run: Option<(&'static str, Vec<String>)>,
}

/// The address to hand out. A window without a server still shows what the
/// snippet will look like, with the address it would have.
pub(crate) fn address(app: &NewEraApp) -> String {
    app.mcp_url
        .clone()
        .unwrap_or_else(|| "http://127.0.0.1:7878/mcp".to_owned())
}

/// The clients people actually arrive with, each with the one line it wants.
pub(crate) fn clients(url: &str) -> Vec<Client> {
    let terminal = crate::i18n::tr("No terminal:");
    vec![
        Client {
            label: "Claude Code",
            place: terminal,
            code: format!("claude mcp add --transport http newera {url}"),
            note: crate::i18n::tr(
                "Os instaladores já registram sozinhos se o Claude Code estiver instalado.",
            ),
            run: Some((
                "claude",
                vec![
                    "mcp".to_owned(),
                    "add".to_owned(),
                    "--transport".to_owned(),
                    "http".to_owned(),
                    "newera".to_owned(),
                    url.to_owned(),
                ],
            )),
        },
        Client {
            label: "Claude Desktop",
            place: crate::i18n::tr("Configurações → Desenvolvedor → Editar configuração"),
            code: format!(
                "{{\n  \"mcpServers\": {{\n    \"newera\": {{\n      \"command\": \"npx\",\n      \"args\": [\"-y\", \"mcp-remote\", \"{url}\"]\n    }}\n  }}\n}}"
            ),
            note: crate::i18n::tr(
                "Precisa do Node.js instalado para a ponte mcp-remote. Reinicie o Claude Desktop.",
            ),
            run: None,
        },
        Client {
            label: "Codex",
            place: terminal,
            code: format!("codex mcp add newera --url {url}"),
            note: crate::i18n::tr(
                "Os instaladores já registram sozinhos se o Codex estiver instalado.",
            ),
            run: Some((
                "codex",
                vec![
                    "mcp".to_owned(),
                    "add".to_owned(),
                    "newera".to_owned(),
                    "--url".to_owned(),
                    url.to_owned(),
                ],
            )),
        },
        Client {
            label: "Gemini CLI",
            place: terminal,
            code: format!("gemini mcp add --transport http newera {url}"),
            note: crate::i18n::tr("Depois abra o Gemini CLI e peça o projeto."),
            run: Some((
                "gemini",
                vec![
                    "mcp".to_owned(),
                    "add".to_owned(),
                    "--transport".to_owned(),
                    "http".to_owned(),
                    "newera".to_owned(),
                    url.to_owned(),
                ],
            )),
        },
        Client {
            label: "VS Code",
            place: terminal,
            code: format!(
                "code --add-mcp '{{\"name\":\"newera\",\"type\":\"http\",\"url\":\"{url}\"}}'"
            ),
            note: crate::i18n::tr("Use no modo agente do Copilot."),
            run: Some((
                "code",
                vec![
                    "--add-mcp".to_owned(),
                    format!("{{\"name\":\"newera\",\"type\":\"http\",\"url\":\"{url}\"}}"),
                ],
            )),
        },
        Client {
            label: "Cursor",
            place: crate::i18n::tr("Arquivo ~/.cursor/mcp.json"),
            code: format!(
                "{{\n  \"mcpServers\": {{\n    \"newera\": {{ \"url\": \"{url}\" }}\n  }}\n}}"
            ),
            note: crate::i18n::tr("Reinicie o Cursor depois de salvar."),
            run: None,
        },
        Client {
            label: crate::i18n::tr("Outro app"),
            place: crate::i18n::tr(
                "Nas configurações de MCP do app (Cline, Roo Code, Cherry Studio, LM Studio…)",
            ),
            code: format!(
                "{{\n  \"mcp\": {{\n    \"newera\": {{ \"type\": \"remote\", \"url\": \"{url}\" }}\n  }}\n}}"
            ),
            note: crate::i18n::tr(
                "O que importa é o app falar MCP: o modelo pode ser qualquer um.",
            ),
            run: None,
        },
    ]
}

/// What a registration attempt left behind: the program's name, or why not.
pub(crate) type RegisterSlot = std::sync::Arc<std::sync::Mutex<Option<Result<String, String>>>>;

/// Whether a command line is installed on this machine, by looking where the
/// shell would look. Windows keeps its command shims under other extensions.
#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn installed(program: &str) -> bool {
    std::env::var_os("PATH").is_some_and(|path| {
        std::env::split_paths(&path).any(|dir| {
            let exe = dir.join(program);
            exe.is_file()
                || exe.with_extension("exe").is_file()
                || exe.with_extension("cmd").is_file()
                || exe.with_extension("bat").is_file()
        })
    })
}

/// Registers this window with a client, for the person, in the background —
/// the same command the panel shows, run instead of copied.
#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn register(app: &mut NewEraApp, program: &'static str, args: Vec<String>) {
    let slot: RegisterSlot = std::sync::Arc::default();
    app.ai_job = Some(slot.clone());
    std::thread::spawn(move || {
        let result = match std::process::Command::new(program).args(&args).output() {
            Ok(done) if done.status.success() => Ok(program.to_owned()),
            Ok(done) => {
                let said = String::from_utf8_lossy(&done.stderr);
                let said = said.trim();
                let said = if said.is_empty() {
                    String::from_utf8_lossy(&done.stdout).trim().to_owned()
                } else {
                    said.to_owned()
                };
                Err(said.lines().last().unwrap_or("").to_owned())
            }
            Err(err) => Err(err.to_string()),
        };
        if let Ok(mut guard) = slot.lock() {
            *guard = Some(result);
        }
    });
}

/// Now, in Unix milliseconds. A browser has no `SystemTime`: asking for it
/// there aborts the whole editor, so the clock comes from the page.
fn now_ms() -> u64 {
    #[cfg(target_arch = "wasm32")]
    {
        // `Date::now` is milliseconds since the epoch, as a float: whole
        // numbers, and exact well past this century. The clamp is for a
        // machine whose clock is set before 1970 or absurdly far ahead.
        #[allow(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "a date in milliseconds fits u64 exactly once clamped"
        )]
        {
            js_sys::Date::now().clamp(0.0, 9e15) as u64
        }
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        newera_core::collab::now_ms()
    }
}

/// How long ago, in words, for a moment in Unix milliseconds.
pub(crate) fn ago(now_ms: u64, then_ms: u64) -> String {
    let seconds = now_ms.saturating_sub(then_ms) / 1000;
    match seconds {
        0..=4 => crate::i18n::tr("agora").to_owned(),
        5..=59 => crate::i18n::fill("há {} s", &[&seconds]),
        60..=3599 => crate::i18n::fill("há {} min", &[&(seconds / 60)]),
        _ => crate::i18n::fill("há {} h", &[&(seconds / 3600)]),
    }
}

/// What the window knows right now about the AI side, read once per frame.
pub(crate) struct Pulse {
    pub(crate) agents: Vec<newera_core::collab::Agent>,
    pub(crate) recent: Vec<newera_core::collab::Call>,
    pub(crate) calls: u64,
    pub(crate) now_ms: u64,
}

impl Pulse {
    pub(crate) fn read(app: &NewEraApp) -> Self {
        let doc = app.document.read();
        let agents = doc.agents();
        Self {
            agents: agents.list().to_vec(),
            recent: agents.recent().to_vec(),
            calls: agents.calls(),
            now_ms: now_ms(),
        }
    }

    /// The client heard from last, which on one desktop is the one.
    pub(crate) fn newest(&self) -> Option<&newera_core::collab::Agent> {
        self.agents.iter().max_by_key(|a| a.seen_ms)
    }

    /// Something happened in the last couple of seconds: worth a flash.
    pub(crate) fn busy(&self) -> bool {
        self.newest()
            .is_some_and(|a| self.now_ms.saturating_sub(a.seen_ms) < 2_000)
    }
}

/// Says it out loud, once, when an AI connects for the first time: the whole
/// point of this is that nobody has to go looking to find out whether it
/// worked.
pub(crate) fn announce(app: &mut NewEraApp) {
    // A registration started from the panel finishes in its own thread.
    let finished = app
        .ai_job
        .as_ref()
        .and_then(|slot| slot.lock().ok().and_then(|mut done| done.take()));
    if let Some(result) = finished {
        app.ai_job = None;
        match result {
            Ok(program) => app.set_status(crate::i18n::fill(
                "Registrado no {} — agora é só pedir",
                &[&program],
            )),
            Err(why) => app.set_status(format!(
                "⚠ {}",
                crate::i18n::fill("Não deu para registrar: {}", &[&why])
            )),
        }
    }
    let (agents, calls, last): (Vec<String>, u64, Option<(String, String)>) = {
        let doc = app.document.read();
        let agents = doc.agents();
        (
            agents.list().iter().map(|a| a.name.clone()).collect(),
            agents.calls(),
            agents
                .recent()
                .last()
                .map(|c| (c.tool.clone(), c.agent.clone())),
        )
    };
    // Every tool the agent uses says its name where messages go: from across
    // the room that is the difference between "it is working" and silence.
    if calls > app.announced_calls {
        app.announced_calls = calls;
        if let Some((tool, agent)) = last {
            // Two proper names and a dot: nothing here to translate.
            app.set_status(format!("{tool} · {agent}"));
        }
    }
    if agents.len() <= app.announced_agents {
        return;
    }
    let newcomer = agents[app.announced_agents..].join(", ");
    app.announced_agents = agents.len();
    app.set_status(crate::i18n::fill(
        "{} conectou — sua IA já pode desenhar aqui",
        &[&newcomer],
    ));
}

/// The MCP chip in the status bar: state, who is connected, and a light that
/// blinks when a tool lands. Clicking it opens the panel.
pub(crate) fn chip(app: &mut NewEraApp, ui: &mut egui::Ui) {
    let t = crate::theme::of(ui.visuals());
    let pulse = Pulse::read(app);
    let serving = app.mcp_url.is_some();
    let (color, text) = match (serving, pulse.newest()) {
        (true, Some(agent)) => (
            if pulse.busy() { t.accent } else { t.ok },
            crate::i18n::fill("{} · {} chamadas", &[&agent.name, &pulse.calls]),
        ),
        (true, None) => (t.ok, crate::i18n::tr("MCP · esperando sua IA").to_owned()),
        (false, _) if cfg!(target_arch = "wasm32") => (
            t.ink_faint,
            crate::i18n::tr("IA: só no aplicativo").to_owned(),
        ),
        (false, _) => (t.ink_faint, crate::i18n::tr("MCP desligado").to_owned()),
    };
    crate::app::lamp(ui, color);
    let chip = ui
        .add(
            egui::Button::new(crate::theme::fig(ui.visuals(), &text).color(color))
                .frame(false)
                .sense(egui::Sense::click()),
        )
        .on_hover_text(crate::i18n::tr("Conectar sua IA — clique para ver como"));
    if chip.clicked() {
        app.dialog = Some(crate::dialogs::Dialog::ConnectAi { client: 0 });
    }
    // While an agent is at work the window has something new to show every
    // moment: keep the clock and the light moving without spinning the CPU.
    if serving {
        ui.ctx()
            .request_repaint_after(std::time::Duration::from_millis(if pulse.busy() {
                150
            } else {
                800
            }));
    }
}

/// The menu people find when they go looking for the AI.
pub(crate) fn menu(app: &mut NewEraApp, ui: &mut egui::Ui) {
    ui.menu_button(crate::i18n::tr("IA"), |ui| {
        let pulse = Pulse::read(app);
        match pulse.newest() {
            Some(agent) => {
                ui.label(
                    RichText::new(crate::i18n::fill(
                        "{} conectado · {}",
                        &[&agent.name, &ago(pulse.now_ms, agent.seen_ms)],
                    ))
                    .strong(),
                );
            }
            None if app.mcp_url.is_some() => {
                ui.weak(crate::i18n::tr("Nenhuma IA conectada ainda"));
            }
            None => {
                ui.weak(crate::i18n::tr("MCP desligado"));
            }
        }
        ui.separator();
        if crate::app::menu_item(
            ui,
            icon::ROBOT,
            crate::i18n::tr("Conectar sua IA…"),
            "",
            true,
        ) {
            app.dialog = Some(crate::dialogs::Dialog::ConnectAi { client: 0 });
        }
        let url = address(app);
        if crate::app::menu_item(
            ui,
            icon::COPY,
            crate::i18n::tr("Copiar o endereço do MCP"),
            "",
            app.mcp_url.is_some(),
        ) {
            ui.ctx().copy_text(url);
            app.set_status(crate::i18n::tr("Endereço copiado"));
            ui.close();
        }
    });
}

/// The panel itself. Returns whether it should close.
pub(crate) fn panel(app: &mut NewEraApp, ctx: &egui::Context, chosen: &mut usize) -> bool {
    let mut close = false;
    let t = ctx.style_of(ctx.theme());
    let t = crate::theme::of(&t.visuals);
    let url = address(app);
    let serving = app.mcp_url.is_some();
    let pulse = Pulse::read(app);
    let clients = clients(&url);
    *chosen = (*chosen).min(clients.len() - 1);

    crate::theme::modal(ctx, egui::Id::new("connect-ai")).show(ctx, |ui| {
        ui.set_min_width(560.0);
        crate::theme::title(
            ui,
            &format!(
                "{} {}",
                icon::ROBOT,
                crate::i18n::tr("Conectar sua IA")
            ),
        );
        ui.label(crate::i18n::tr(
            "O editor abre uma porta MCP: a sua IA desenha aqui dentro, em centímetros, e você vê acontecer.",
        ));
        ui.add_space(10.0);

        // ---- What is true right now ----
        crate::theme::section(ui, crate::i18n::tr("Agora"));
        egui::Frame::group(ui.style())
            .fill(t.raised)
            .stroke(egui::Stroke::new(1.0, t.rule))
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.horizontal(|ui| {
                    crate::app::lamp(ui, if serving { t.ok } else { t.ink_faint });
                    if serving {
                        ui.label(crate::i18n::tr("Servidor MCP ligado"));
                        // An address is read and typed as it is written: the
                        // small-capitals figures line would change it.
                        ui.label(
                            RichText::new(&url)
                                .monospace()
                                .size(11.0)
                                .color(t.ink_dim),
                        );
                        if ui
                            .small_button(format!("{} {}", icon::COPY, crate::i18n::tr("Copiar")))
                            .clicked()
                        {
                            ui.ctx().copy_text(url.clone());
                            app.set_status(crate::i18n::tr("Endereço copiado"));
                        }
                    } else if cfg!(target_arch = "wasm32") {
                        ui.label(crate::i18n::tr(
                            "No navegador o editor roda sozinho: o MCP vive no aplicativo do computador.",
                        ));
                    } else {
                        ui.label(crate::i18n::tr(
                            "O servidor está desligado (--no-server): reabra o aplicativo sem essa opção.",
                        ));
                    }
                });
                ui.horizontal(|ui| {
                    if let Some(agent) = pulse.newest() {
                        crate::app::lamp(ui, if pulse.busy() { t.accent } else { t.ok });
                        ui.label(
                            RichText::new(crate::i18n::fill(
                                "{} conectado · {} chamadas · {}",
                                &[&agent.name, &pulse.calls, &ago(pulse.now_ms, agent.seen_ms)],
                            ))
                            .color(t.ok)
                            .strong(),
                        );
                    } else {
                        crate::app::lamp(ui, t.ink_faint);
                        ui.label(crate::i18n::tr(
                            "Nenhuma IA conectada ainda — siga os três passos abaixo.",
                        ));
                    }
                });
            });
        ui.add_space(10.0);

        // ---- Three steps ----
        crate::theme::section(
            ui,
            crate::i18n::tr("1. Escolha o aplicativo de IA que você usa"),
        );
        ui.horizontal_wrapped(|ui| {
            for (i, client) in clients.iter().enumerate() {
                if ui.selectable_label(*chosen == i, client.label).clicked() {
                    *chosen = i;
                }
            }
        });
        let client = &clients[*chosen];
        ui.add_space(8.0);
        crate::theme::section(ui, crate::i18n::tr("2. Cole isto onde ele pede"));
        ui.label(RichText::new(client.place).color(t.ink_dim));
        egui::Frame::group(ui.style())
            .fill(t.inset)
            .stroke(egui::Stroke::new(1.0, t.rule))
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                // The snippet is a block of its own: a long address must not
                // be pushed under the buttons beside it.
                ui.add(egui::Label::new(RichText::new(&client.code).monospace()).wrap());
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui
                            .button(format!("{} {}", icon::COPY, crate::i18n::tr("Copiar")))
                            .clicked()
                        {
                            ui.ctx().copy_text(client.code.clone());
                            app.set_status(crate::i18n::tr("Copiado"));
                        }
                        // When the client has a command line and it is on this
                        // machine, there is no reason to make anybody open a
                        // terminal: the window runs it.
                        #[cfg(not(target_arch = "wasm32"))]
                        if let Some((program, args)) = &client.run
                            && installed(program)
                        {
                            let busy = app.ai_job.is_some();
                            let label = format!(
                                "{} {}",
                                icon::LIGHTNING,
                                if busy {
                                    crate::i18n::tr("Registrando…")
                                } else {
                                    crate::i18n::tr("Registrar agora")
                                }
                            );
                            if ui
                                .add_enabled(!busy && serving, egui::Button::new(label))
                                .on_hover_text(crate::i18n::tr(
                                    "Roda esse mesmo comando aqui, sem abrir o terminal.",
                                ))
                                .clicked()
                            {
                                register(app, program, args.clone());
                            }
                        }
                    });
                });
            });
        ui.label(RichText::new(client.note).color(t.ink_dim).small());
        ui.add_space(8.0);
        crate::theme::section(ui, crate::i18n::tr("3. Peça alguma coisa"));
        let ask = crate::i18n::tr(
            "Quantas paredes tem este projeto? Depois coloque uma janela de 120 cm na sala.",
        );
        ui.horizontal(|ui| {
            ui.label(RichText::new(ask).italics());
            if ui
                .small_button(format!("{} {}", icon::COPY, crate::i18n::tr("Copiar")))
                .clicked()
            {
                ui.ctx().copy_text(ask.to_owned());
                app.set_status(crate::i18n::tr("Copiado"));
            }
        });
        ui.add_space(10.0);

        // ---- The proof ----
        crate::theme::section(ui, crate::i18n::tr("Últimas chamadas"));
        if pulse.recent.is_empty() {
            ui.weak(crate::i18n::tr(
                "Nada ainda. Assim que sua IA usar uma ferramenta, ela aparece aqui.",
            ));
        } else {
            egui::ScrollArea::vertical()
                .max_height(120.0)
                .show(ui, |ui| {
                    for call in pulse.recent.iter().rev() {
                        ui.horizontal(|ui| {
                            ui.label(
                                RichText::new(&call.tool)
                                    .monospace()
                                    .color(if pulse.now_ms.saturating_sub(call.at_ms) < 2_000 {
                                        t.accent
                                    } else {
                                        t.ink
                                    }),
                            );
                            ui.label(
                                RichText::new(format!(
                                    "{} · {}",
                                    call.agent,
                                    ago(pulse.now_ms, call.at_ms)
                                ))
                                .color(t.ink_dim)
                                .small(),
                            );
                        });
                    }
                });
        }

        crate::theme::footer(ui, |ui| {
            if crate::theme::primary(ui, crate::i18n::tr("Fechar")).clicked() {
                close = true;
            }
            if !serving
                && cfg!(target_arch = "wasm32")
                && crate::theme::secondary(ui, crate::i18n::tr("Baixar o aplicativo")).clicked()
            {
                ui.ctx().open_url(egui::OpenUrl::new_tab(
                    "https://3dneweraai.com/#baixar",
                ));
            }
        });
    });
    // The panel is a live thing: it has a clock in it and calls landing under
    // the person's eyes.
    ctx.request_repaint_after(std::time::Duration::from_millis(250));
    close || ctx.input(|i| i.key_pressed(egui::Key::Escape))
}

#[cfg(test)]
mod tests {
    /// The snippet each client gets carries the address of this window, so
    /// pasting it connects to this window and not to a guess.
    #[test]
    fn every_client_gets_this_window_address() {
        let clients = super::clients("http://127.0.0.1:7999/mcp");
        assert!(clients.len() >= 6);
        for client in clients {
            assert!(
                client.code.contains("http://127.0.0.1:7999/mcp"),
                "{} lost the address: {}",
                client.label,
                client.code
            );
            assert!(!client.place.is_empty());
            assert!(!client.note.is_empty());
        }
    }

    /// The clients with a command line carry it ready to run, so the window
    /// can do the registering instead of asking for a terminal.
    #[test]
    fn the_command_line_clients_can_be_registered_from_here() {
        let clients = super::clients("http://127.0.0.1:7999/mcp");
        let runnable: Vec<&str> = clients
            .iter()
            .filter(|c| c.run.is_some())
            .map(|c| c.label)
            .collect();
        assert!(
            runnable.contains(&"Claude Code") && runnable.contains(&"Codex"),
            "expected the CLIs among {runnable:?}"
        );
        for client in clients.iter().filter_map(|c| c.run.as_ref()) {
            assert!(
                client.1.iter().any(|a| a.contains("127.0.0.1:7999")),
                "the command must carry this window's address"
            );
        }
    }

    /// Whatever else it finds, it finds the shell's own tools — and does not
    /// invent a program nobody has.
    #[cfg(unix)]
    #[test]
    fn it_can_tell_what_is_installed() {
        assert!(super::installed("sh"));
        assert!(!super::installed("newera-not-a-real-program"));
    }

    /// The moment reads as a person would say it.
    #[test]
    fn how_long_ago_in_words() {
        let now = 10_000_000;
        assert_eq!(super::ago(now, now), "agora");
        assert_eq!(super::ago(now, now - 30_000), "há 30 s");
        assert_eq!(super::ago(now, now - 300_000), "há 5 min");
        assert_eq!(super::ago(now, now - 7_200_000), "há 2 h");
    }
}
