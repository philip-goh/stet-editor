//! Entry point for the `stet` editor.
//!
//! Usage: `stet [FILE]`

use std::path::PathBuf;

fn main() {
    let initial_file = std::env::args_os().nth(1).map(PathBuf::from);
    stet_ui::launch(initial_file);
}
