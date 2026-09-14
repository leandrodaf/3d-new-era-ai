//! `newera-gui`: the editor for a double click or a shortcut. On Windows it is
//! a windowed app, so no console opens behind the editor. It takes the same
//! arguments as `newera`.

#![cfg_attr(windows, windows_subsystem = "windows")]

fn main() -> anyhow::Result<()> {
    newera_cli::run()
}
