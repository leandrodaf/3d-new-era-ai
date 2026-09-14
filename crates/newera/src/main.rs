//! `newera`: the editor, the server or MCP over stdio, from the terminal.

fn main() -> anyhow::Result<()> {
    newera_cli::run()
}
