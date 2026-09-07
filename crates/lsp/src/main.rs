mod analysis;
mod document;
mod protocol;
mod server;
mod transport;

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

    match server::run(io::stdin().lock(), io::stdout().lock()) {
        Ok(status) => status,
        Err(error) => {
            eprintln!("yozora-lsp: {error}");
            ExitCode::FAILURE
        }
    }
}
