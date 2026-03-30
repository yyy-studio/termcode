use std::path::PathBuf;
use std::process;

use clap::Parser;

use termcode_term::app::App;

/// termcode - A terminal-based code viewer and editor
#[derive(Parser)]
#[command(name = "termcode", version, about)]
struct Cli {
    /// File or directory to open
    path: Option<PathBuf>,
}

fn main() {
    let cli = Cli::parse();

    let (root, file) = match &cli.path {
        Some(path) if path.is_dir() => (Some(path.clone()), None),
        Some(path) if path.is_file() => {
            let parent = path.parent().map(|p| p.to_path_buf());
            (parent, Some(path.clone()))
        }
        Some(path) => {
            eprintln!("Error: path does not exist: {}", path.display());
            process::exit(1);
        }
        None => (None, None),
    };

    let mut app = App::new(root);

    if let Some(path) = &file {
        if let Err(e) = app.open_file(path) {
            eprintln!("Error opening file: {e}");
            process::exit(1);
        }
    } else {
        app.focus_sidebar_if_visible();
        app.restore_session();
    }

    if let Err(e) = app.run() {
        eprintln!("Error: {e}");
        process::exit(1);
    }
}
