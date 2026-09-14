//! `newera` — one binary, every mode.
//!
//! - `newera`          opens the editor with the HTTP + MCP server built in
//! - `newera serve`    runs only the HTTP + MCP server (headless)
//! - `newera mcp`      speaks MCP over stdio, for clients that spawn processes

mod demo;

use std::net::SocketAddr;

use anyhow::Context;
use clap::{Parser, Subcommand};
use newera_core::{Document, Home, SharedDocument};
use tokio_util::sync::CancellationToken;

#[derive(Debug, Parser)]
#[command(
    name = "newera",
    version,
    about = "3D New Era AI — home design with a built-in MCP server"
)]
struct Cli {
    #[command(subcommand)]
    command: Option<Mode>,

    /// Address for the HTTP + MCP server.
    #[arg(long, global = true, env = "NEWERA_ADDR", default_value_t = newera_server::DEFAULT_ADDR)]
    addr: SocketAddr,
    /// Token required by the HTTP + MCP server; needed to listen beyond loopback.
    #[arg(long, global = true, env = "NEWERA_TOKEN")]
    token: Option<String>,

    /// Start with a sample house instead of an empty project.
    #[arg(long, global = true)]
    demo: bool,

    /// Project file (.newera) to open.
    #[arg(global = true)]
    file: Option<std::path::PathBuf>,
}

#[derive(Debug, Subcommand)]
enum Mode {
    /// Open the desktop editor (default).
    Gui {
        /// Do not start the embedded HTTP + MCP server.
        #[arg(long)]
        no_server: bool,
    },
    /// Run only the HTTP + MCP server, without a window.
    Serve,
    /// Serve MCP over stdin/stdout.
    Mcp,
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    let mode = cli.command.unwrap_or(Mode::Gui { no_server: false });

    // stdout belongs to the protocol in stdio mode, so logs always go to stderr.
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_env("NEWERA_LOG").unwrap_or_else(|_| {
                "info,wgpu_core=warn,wgpu_hal=warn,naga=warn,rmcp=warn,egui_wgpu=warn".into()
            }),
        )
        .init();

    let mut document = Document::new(if cli.demo {
        demo::sample_home()
    } else {
        Home::default()
    });
    if let Some(path) = &cli.file {
        let opened = newera_sh3d::open_file(&mut document, path)
            .map_err(anyhow::Error::msg)
            .with_context(|| format!("cannot open {}", path.display()))?;
        for warning in opened.warnings {
            tracing::warn!("import: {warning}");
        }
    }
    let document = SharedDocument::new(document);

    match mode {
        Mode::Gui { no_server } => run_gui(document, cli.addr, cli.token.as_deref(), no_server),
        Mode::Serve => runtime()?.block_on(serve_until_ctrl_c(document, cli.addr, cli.token)),
        Mode::Mcp => runtime()?
            .block_on(newera_mcp::serve_stdio(document))
            .context("MCP stdio server failed"),
    }
}

fn runtime() -> anyhow::Result<tokio::runtime::Runtime> {
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .context("failed to start async runtime")
}

async fn serve_until_ctrl_c(
    document: SharedDocument,
    addr: SocketAddr,
    token: Option<String>,
) -> anyhow::Result<()> {
    let shutdown = CancellationToken::new();
    let trigger = shutdown.clone();
    tokio::spawn(async move {
        let _ = tokio::signal::ctrl_c().await;
        trigger.cancel();
    });
    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .with_context(|| format!("cannot bind {addr}"))?;
    newera_server::serve_listener_with(
        document,
        listener,
        shutdown,
        newera_server::ServerOptions {
            token,
            ..Default::default()
        },
    )
    .await
    .with_context(|| format!("HTTP server failed on {addr}"))
}

/// The window owns the main thread (required on macOS); the server runs on a
/// background runtime sharing the same document.
fn run_gui(
    document: SharedDocument,
    addr: SocketAddr,
    token: Option<&str>,
    no_server: bool,
) -> anyhow::Result<()> {
    let shutdown = CancellationToken::new();
    let mut server_thread = None;
    let mut mcp_url = None;

    if !no_server {
        let runtime = runtime()?;
        // Bind before opening the window so a busy port fails loudly.
        let listener = runtime
            .block_on(tokio::net::TcpListener::bind(addr))
            .with_context(|| format!("cannot bind {addr}; use --addr or --no-server"))?;
        let local = listener.local_addr()?;
        if !local.ip().is_loopback() && token.is_none() {
            anyhow::bail!("{local} is reachable from the network: set --token or NEWERA_TOKEN");
        }
        mcp_url = Some(format!("http://{local}/mcp"));

        let (doc, token, access) = (document.clone(), shutdown.clone(), token.map(str::to_owned));
        server_thread = Some(std::thread::spawn(move || {
            let options = newera_server::ServerOptions {
                token: access,
                ..Default::default()
            };
            if let Err(err) = runtime.block_on(newera_server::serve_listener_with(
                doc, listener, token, options,
            )) {
                tracing::error!("HTTP server stopped: {err}");
            }
        }));
    }

    let result = newera_app::run(
        document,
        newera_app::AppOptions {
            mcp_url,
            open: None,
        },
    );

    shutdown.cancel();
    if let Some(thread) = server_thread {
        let _ = thread.join();
    }
    result.map_err(|err| anyhow::anyhow!("editor failed: {err}"))
}
