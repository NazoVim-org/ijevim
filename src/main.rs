use ijevim::config::Config;
use ijevim::editor::Editor;
use ijevim::types::{IjevimError, Keymap};

const USAGE: &str =
    "Usage: ijevim [<file>] | ijevim {vim|emacs} [<file>]\n       ijevim --version | --help";

const HELP: &str = "\
ijevim — a minimal Vim-like TUI editor

USAGE:
    ijevim [<file>]
    ijevim vim [<file>]
    ijevim emacs [<file>]

OPTIONS:
    -v, --version    Print version and exit
    -h, --help       Print this help message

ARGS:
    <file>          File to edit";

/// Parsed CLI input.
struct Cli {
    keymap: Option<Keymap>,
    file: Option<String>,
}

fn print_help() {
    println!("{}", HELP);
}

fn print_version() {
    println!("ijevim {}", env!("CARGO_PKG_VERSION"));
}

/// Result of parsing command-line arguments.
enum CliAction {
    /// Normal edit invocation.
    Edit(Cli),
    /// Print help text and exit.
    Help,
    /// Print version and exit.
    Version,
}

/// Parse command-line arguments without external dependencies.
///
/// `args` should contain the raw program arguments **excluding** argv[0].
///
/// Accepted shapes:
/// - `ijevim`
/// - `ijevim <file>`
/// - `ijevim vim [<file>]`
/// - `ijevim emacs [<file>]`
/// - `ijevim <file> vim`        (file first, mode after; preserved by old behaviour)
/// - `ijevim <file> vim <file2>` (subcommand file wins)
///
/// Any token that is not a recognised flag or mode is treated as a file path,
/// matching the pre-refactor `clap` behaviour.
fn parse_args(args: &[String]) -> CliAction {
    let mut keymap: Option<Keymap> = None;
    let mut file: Option<String> = None;
    let mut mode_taken_file: Option<String> = None;

    for arg in args {
        match arg.as_str() {
            "-h" | "--help" => return CliAction::Help,
            "-v" | "--version" => return CliAction::Version,
            "vim" => {
                keymap = Some(Keymap::Vim);
            }
            "emacs" => {
                keymap = Some(Keymap::Emacs);
            }
            _ => {
                // First non-mode token becomes the parent file, unless a mode
                // subcommand already supplied its own file.
                if file.is_none() {
                    file = Some(arg.clone());
                } else if keymap.is_some() && mode_taken_file.is_none() {
                    mode_taken_file = Some(arg.clone());
                } else {
                    // Extra positional: keep first as the canonical file.
                    eprintln!("{}: ignoring extra argument '{}'", USAGE, arg);
                }
            }
        }
    }

    // If a mode subcommand had its own file, prefer it over the parent file.
    if let Some(f) = mode_taken_file {
        file = Some(f);
    }

    CliAction::Edit(Cli { keymap, file })
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), IjevimError> {
    // Runtime initialised by #[tokio::main]
    // flavor = "current_thread": we don't need a multi-threaded executor
    // for a single-TUI-editor, and dropping rt-multi-thread keeps tokio smaller.

    // Set panic hook to restore terminal state
    std::panic::set_hook(Box::new(|_| {
        let _ = crossterm::terminal::disable_raw_mode();
        let _ = crossterm::execute!(std::io::stdout(), crossterm::terminal::LeaveAlternateScreen);
    }));

    let cfg = Config::load();

    let args: Vec<String> = std::env::args().skip(1).collect();
    let cli = match parse_args(&args) {
        CliAction::Edit(cli) => cli,
        CliAction::Help => {
            print_help();
            return Ok(());
        }
        CliAction::Version => {
            print_version();
            return Ok(());
        }
    };
    // CLI keymap takes priority over config keymap
    let keymap = cli.keymap.unwrap_or(match cfg.keymap.as_str() {
        "emacs" => Keymap::Emacs,
        _ => Keymap::Vim,
    });

    let mut editor = Editor::new(cli.file.as_deref(), keymap, cfg).await?;
    editor.run().await?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_cli(args: &[&str]) -> Cli {
        let argv: Vec<String> = args.iter().map(|s| s.to_string()).collect();
        match parse_args(&argv) {
            CliAction::Edit(cli) => cli,
            _ => panic!("expected Edit, got Help or Version"),
        }
    }

    #[test]
    fn parses_help() {
        assert!(matches!(parse_args(&["--help".into()]), CliAction::Help));
        assert!(matches!(parse_args(&["-h".into()]), CliAction::Help));
    }

    #[test]
    fn parses_version() {
        assert!(matches!(
            parse_args(&["--version".into()]),
            CliAction::Version
        ));
        assert!(matches!(parse_args(&["-v".into()]), CliAction::Version));
    }

    #[test]
    fn parses_vim_subcommand_without_file() {
        let cli = parse_cli(&["vim"]);
        assert_eq!(cli.keymap, Some(Keymap::Vim));
        assert_eq!(cli.file, None);
    }

    #[test]
    fn parses_emacs_subcommand_without_file() {
        let cli = parse_cli(&["emacs"]);
        assert_eq!(cli.keymap, Some(Keymap::Emacs));
        assert_eq!(cli.file, None);
    }

    #[test]
    fn parses_vim_subcommand_with_file() {
        let cli = parse_cli(&["vim", "sample.txt"]);
        assert_eq!(cli.keymap, Some(Keymap::Vim));
        assert_eq!(cli.file.as_deref(), Some("sample.txt"));
    }

    #[test]
    fn parses_emacs_subcommand_with_file() {
        let cli = parse_cli(&["emacs", "sample.txt"]);
        assert_eq!(cli.keymap, Some(Keymap::Emacs));
        assert_eq!(cli.file.as_deref(), Some("sample.txt"));
    }

    #[test]
    fn parses_top_level_file_as_default_vim() {
        let cli = parse_cli(&["sample.txt"]);
        assert_eq!(cli.keymap, None, "no keymap override when no subcommand");
        assert_eq!(cli.file.as_deref(), Some("sample.txt"));
    }

    #[test]
    fn honors_parent_file_when_mode_subcommand_is_present() {
        let cli = parse_cli(&["sample.txt", "emacs"]);
        assert_eq!(cli.keymap, Some(Keymap::Emacs));
        assert_eq!(cli.file.as_deref(), Some("sample.txt"));
    }

    #[test]
    fn prefers_subcommand_file_over_parent_file() {
        let cli = parse_cli(&["parent.txt", "vim", "child.txt"]);
        assert_eq!(cli.keymap, Some(Keymap::Vim));
        assert_eq!(cli.file.as_deref(), Some("child.txt"));
    }

    #[test]
    fn treats_unknown_subcommand_as_file_argument() {
        let cli = parse_cli(&["invalid-mode"]);
        assert_eq!(
            cli.keymap, None,
            "no keymap override for unknown subcommand"
        );
        assert_eq!(cli.file.as_deref(), Some("invalid-mode"));
    }
}
