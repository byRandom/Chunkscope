//! `chunkscope-cli`: Chunkscope's world generation from the command line.
//! See `docs/CLI.md` for every command, option and the JSON output.

mod args;
mod commands;
mod json;

use std::process::ExitCode;

const HELP: &str = "\
chunkscope-cli: Minecraft: Java Edition seed maps from the command line.

usage: chunkscope-cli <command> [options]

commands:
  biome <seed> <x> <z> [y]            the biome at a block (default y 64)
  height <seed> <x> <z>               the terrain height of a column
  spawn <seed>                        the world spawn
  structures <seed> <x> <z> [radius]  the structures near a block (default 1000)
  find <query>...                     search seeds (see `find --help`)
  targets                             the structure targets a query can name
  help, version

options (every command except `find`):
  --dimension <overworld|nether|end>  the dimension (default overworld)
  --json                              print JSON instead of text

A seed is a number or any text, as in the game's world creation screen.
Run `chunkscope-cli <command> --help` for a command's options.
";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let Some((command, rest)) = args.split_first() else {
        eprint!("{HELP}");
        return ExitCode::from(2);
    };
    let result = match command.as_str() {
        "help" | "--help" | "-h" => {
            print!("{HELP}");
            Ok(())
        }
        "version" | "--version" | "-V" => {
            println!("chunkscope-cli {}", env!("CARGO_PKG_VERSION"));
            Ok(())
        }
        other => commands::run(other, rest),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("chunkscope-cli: {message}");
            eprintln!("(run `chunkscope-cli help` for usage)");
            ExitCode::from(2)
        }
    }
}
