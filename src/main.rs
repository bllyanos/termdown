use std::io::Read;
use std::path::{Path, PathBuf};

use clap::{ArgAction, Parser};

mod app;
mod markdown;
mod theme;
mod ui;

#[derive(Debug, Parser)]
#[command(
    name = "termdown",
    version,
    about = "A fast GFM Markdown reader for the terminal",
    disable_version_flag = true
)]
struct Cli {
    /// Print version information
    #[arg(short = 'v', long = "version", action = ArgAction::Version)]
    version: Option<bool>,

    /// Markdown file to read, or - for stdin
    #[arg(value_name = "FILE")]
    file: PathBuf,

    /// TOML theme file
    #[arg(long, value_name = "PATH")]
    theme: Option<PathBuf>,
}

fn read_markdown(path: &Path) -> color_eyre::Result<(String, String)> {
    if path.as_os_str() == "-" {
        let mut source = String::new();
        std::io::stdin()
            .read_to_string(&mut source)
            .map_err(|error| color_eyre::eyre::eyre!("failed to read stdin: {error}"))?;
        return Ok((source, "stdin".to_owned()));
    }

    let source = std::fs::read_to_string(path).map_err(|error| {
        color_eyre::eyre::eyre!("failed to read Markdown '{}': {error}", path.display())
    })?;
    Ok((source, path.to_string_lossy().into_owned()))
}

fn main() -> color_eyre::Result<()> {
    color_eyre::install()?;
    let cli = Cli::parse();
    let (source, source_label) = read_markdown(&cli.file)?;
    let theme = cli
        .theme
        .as_deref()
        .map(theme::Theme::from_path)
        .transpose()?;
    let theme = theme.unwrap_or_default();
    let document = markdown::render(&source, &theme)?;
    let mut app = app::App::new(document, source_label, theme);
    ratatui::run(|terminal| app.run(terminal))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn cli_requires_file() {
        assert!(Cli::try_parse_from(["termdown"]).is_err());
        let cli = Cli::try_parse_from(["termdown", "-"]).unwrap();
        assert_eq!(cli.file, PathBuf::from("-"));
        assert!(cli.theme.is_none());
    }

    #[test]
    fn cli_short_version_flag() {
        let error = Cli::try_parse_from(["termdown", "-v"]).unwrap_err();
        assert_eq!(error.kind(), clap::error::ErrorKind::DisplayVersion);
        let output = error.to_string();
        assert!(output.contains("termdown"));
        assert!(output.contains(env!("CARGO_PKG_VERSION")));
    }

    #[test]
    fn cli_long_version_flag() {
        let error = Cli::try_parse_from(["termdown", "--version"]).unwrap_err();
        assert_eq!(error.kind(), clap::error::ErrorKind::DisplayVersion);
        let output = error.to_string();
        assert!(output.contains("termdown"));
        assert!(output.contains(env!("CARGO_PKG_VERSION")));
    }

    #[test]
    fn cli_accepts_theme() {
        let cli = Cli::try_parse_from(["termdown", "--theme", "theme.toml", "doc.md"]).unwrap();
        assert_eq!(cli.file, PathBuf::from("doc.md"));
        assert_eq!(cli.theme, Some(PathBuf::from("theme.toml")));
    }

    #[test]
    fn missing_file_error_contains_path() {
        let path = Path::new("/definitely/missing/termdown.md");
        let error = read_markdown(path).unwrap_err().to_string();
        assert!(error.contains(path.to_string_lossy().as_ref()));
    }

    #[test]
    fn reads_file_input() {
        let path = std::env::temp_dir().join(format!("termdown-test-{}.md", std::process::id()));
        let mut file = std::fs::File::create(&path).unwrap();
        writeln!(file, "# test").unwrap();
        let result = read_markdown(&path).unwrap();
        std::fs::remove_file(&path).unwrap();
        assert_eq!(result.0, "# test\n");
        assert_eq!(result.1, path.to_string_lossy());
    }
}
