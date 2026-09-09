mod analysis;
mod cancellation;
mod completion;
mod diagnostics;
mod document;
mod files;
mod heading_references;
mod link_diagnostics;
mod links;
mod protocol;
mod query;
mod refactor;
mod rename;
mod resource_completion;
mod resource_edit;
mod server;
mod transport;
mod worker;
mod workspace_index;

use std::io;
use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<_> = std::env::args().skip(1).collect();
    match args.as_slice() {
        [arg] if arg == "--help" || arg == "-h" => {
            println!(
                "Usage: yozora-lsp [--stdio]\n\nYozora Markdown language server over stdin/stdout."
            );
            return ExitCode::SUCCESS;
        }
        [arg] if arg == "--version" || arg == "-V" => {
            println!("yozora-lsp {}", env!("CARGO_PKG_VERSION"));
            return ExitCode::SUCCESS;
        }
        [] => {}
        [arg] if arg == "--stdio" => {}
        _ => {
            eprintln!("Usage: yozora-lsp [--stdio]");
            return ExitCode::FAILURE;
        }
    }

    match server::run(io::BufReader::new(io::stdin()), io::stdout().lock()) {
        Ok(status) => status,
        Err(error) => {
            eprintln!("yozora-lsp: {error}");
            ExitCode::FAILURE
        }
    }
}
