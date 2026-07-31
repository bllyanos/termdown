# termdown

> A fast, keyboard-first GFM Markdown reader for the terminal.

`termdown` turns Markdown into a focused terminal reading experience: clean rendering, predictable navigation, incremental search, and a theme system that belongs in your dotfiles. It is designed for engineers who keep documentation, release notes, design specs, and runbooks close to the command line.

## Why termdown?

Most Markdown workflows force a choice between a full browser and a raw source file. `termdown` occupies the useful middle ground:

- **Read where you work.** Open local documentation without leaving the terminal.
- **Preserve document structure.** Headings, lists, tables, code, links, quotes, and task items remain visually distinct.
- **Navigate at reading speed.** Move by line, page, document boundary, or search result.
- **Keep output deterministic.** The same source and theme produce the same rendered document.
- **Stay local by default.** Files are read directly; no network service or runtime account is required.

## Features

### Markdown rendering

`termdown` uses Comrak with GitHub-Flavored Markdown extensions enabled for:

- Headings with level-aware styling
- Paragraphs and soft or hard line breaks
- Ordered and unordered lists, including nested lists
- Task list items
- Block quotes
- Fenced code blocks and inline code
- Tables with aligned cells and a styled header row
- Links and images
- Emphasis and strong emphasis
- Strikethrough
- Autolinks
- Thematic breaks

- Fenced code blocks use a borderless `code_background` area with one-cell padding on all sides
- Long fenced code rows do not wrap; `←` / `→` or `h` / `l` scroll them horizontally while normal text remains wrapped

Raw HTML, front matter, math, and other parser constructs are rendered as readable literal text where applicable. Raw HTML is not executed or interpreted as terminal control content.

### Terminal-native reading

- Visual-line-aware wrapping and scrolling for normal document text
- Dedicated scrollbar
- Search that follows rendered content, including wrapped lines
- Case-insensitive search with match counts
- Search-result cycling in both directions
- Resize-safe viewport and jump behavior
- Sensible colors and emphasis out of the box

### Custom themes

Supply a TOML file to override semantic colors without recompiling. Colors accept ANSI color names or `#RRGGBB` values. Unknown fields and invalid colors fail with an actionable error.

## Installation

### Quick install

The installer downloads the latest published GitHub release and installs the binary into `~/.local/bin` without requiring `sudo` or Rust:

```bash
curl --proto '=https' --tlsv1.2 -fsSL https://raw.githubusercontent.com/bllyanos/termdown/main/install.sh | sh
```

The prebuilt installer currently supports Linux x86_64. It requires `curl`, `tar`, `install`, `mktemp`, and `sha256sum`.

For a reviewable install, download the script first:

```bash
curl --proto '=https' --tlsv1.2 -fsSL \
  https://raw.githubusercontent.com/bllyanos/termdown/main/install.sh \
  -o install.sh
less install.sh
sh install.sh
```

Set `TERMDOWN_INSTALL_ROOT` to change the installation prefix:

```bash
TERMDOWN_INSTALL_ROOT="$HOME/.local" sh install.sh
```

Add `~/.local/bin` to `PATH` if it is not already present.

### From source

Requirements:

- Rust and Cargo with Edition 2024 support
- A terminal with Unicode and color support recommended

```bash
git clone https://github.com/bllyanos/termdown.git
cd termdown
cargo install --path .
```

The binary is installed as `termdown` in Cargo's binary directory. If you prefer a local build:

```bash
cargo build --release
./target/release/termdown README.md
```

Releases are automated from Conventional Commits pushed to `main`. Release Please opens a release PR; merging it bumps Cargo metadata, creates a `vX.Y.Z` tag and GitHub release, and publishes Linux x86_64 assets.

## Usage

Open a Markdown file:

```bash
termdown README.md
```

Read from standard input:

```bash
curl -fsSL https://example.com/guide.md | termdown -
```

Pipe generated documentation into the reader:

```bash
pandoc design-notes.md -t gfm | termdown -
```

Load a custom theme:

```bash
termdown --theme ~/.config/termdown/theme.toml docs/runbook.md
```

Show the installed version:

```bash
termdown --version
termdown -v
```

These commands do not require `FILE` and print the version sourced from the package metadata.

For normal reading invocations, the positional `FILE` argument is required. Use `-` to read Markdown from standard input.

## Keyboard controls

| Key | Action |
| --- | --- |
| `h` / `←` | Scroll fenced code left one cell |
| `l` / `→` | Scroll fenced code right one cell |
| `j` / `↓` | Scroll down one visual row |
| `k` / `↑` | Scroll up one visual row |
| `f` / `PageDown` / `Space` | Scroll down one viewport |
| `b` / `PageUp` | Scroll up one viewport |
| `g` / `Home` | Jump to the beginning |
| `G` / `End` | Jump to the end |
| `/` | Start search |
| `Enter` | Commit the current search |
| `Esc` | Cancel search, or quit from the reader |
| `n` | Next search match |
| `N` | Previous search match |
| `q` / `Ctrl-C` | Quit |

While searching, type to update matches and use `Backspace` to edit the query. Search is case-insensitive.

## Theme configuration

Theme files are TOML documents. Any omitted field keeps the default value.

```toml
background = "#14161A"
foreground = "#E6E8EB"
muted = "#9196A0"
heading = "yellow"
heading_alt = "#78C8FF"
link = "#6EBEFF"
code_foreground = "#EBF0F5"
code_background = "#2A2E36"
blockquote = "#82D2B9"
table_border = "#AAB4C3"
image = "magenta"
accent = "cyan"
border = "#5A6473"
status = "green"
search = "#FFBE5A"
```

Supported fields:

`background`, `foreground`, `muted`, `heading`, `heading_alt`, `link`, `code_foreground`, `code_background`, `blockquote`, `table_border`, `image`, `accent`, `border`, `status`, and `search`.

## Architecture

The application is intentionally small and layered:

1. **Input** — the CLI accepts a file path or `-` for standard input and loads an optional TOML theme.
2. **Parsing** — Comrak parses the source with GFM extensions enabled.
3. **Rendering** — the Markdown AST is converted into styled Ratatui text, with Unicode display width accounted for during layout.
4. **Interaction** — a stateful event loop handles vertical reading, fenced-code horizontal scrolling, viewport changes, search, and exit behavior.
5. **Presentation** — Ratatui draws a header, a wrapped document body, borderless tinted code areas with one-cell padding on all sides, a scrollbar, and a context-sensitive footer.

The parser and terminal renderer are separate from the interaction state, which keeps formatting behavior testable and makes viewport correctness explicit.

## Safety and boundaries

`termdown` is a reader, not a browser:

- It reads the requested file or standard input.
- It does not fetch linked resources.
- It does not execute Markdown, HTML, shell commands, or code blocks.
- Comrak's unsafe rendering option is disabled.
- Links and images are displayed as styled text; they are not opened automatically.

As with any terminal program, use a trusted terminal emulator and review untrusted input before piping it into interactive tooling.

## Development

Run the test suite:

```bash
cargo test
```

Run the application against a local fixture:

```bash
cargo run -- README.md
```

Build an optimized binary:

```bash
cargo build --release
```

The project favors small, direct modules over a framework-heavy architecture. Changes should preserve keyboard behavior, visual-line accounting, parser safety, and clear error messages for invalid input or themes.

## Project status

`termdown` is early-stage software with a deliberately narrow product surface: read Markdown well in a terminal. The public interfaces are the CLI, keyboard controls, rendered document behavior, and TOML theme format. Expect the implementation and release process to evolve as those contracts mature.

## Contributing

Contributions are welcome when they improve the reading experience without obscuring the core workflow. Good first contributions include:

- Renderer coverage for a concrete Markdown construct
- Terminal layout and accessibility improvements
- Theme design and validation improvements
- Deterministic tests for scrolling, wrapping, search, or parsing
- Documentation with reproducible examples

Before opening a pull request:

1. Keep the change focused.
2. Add or update behavioral tests for new observable behavior.
3. Run `cargo fmt --check` and `cargo test`.
4. Describe terminal dimensions, input format, and expected behavior for UI changes.
## License

Licensed under the [MIT License](LICENSE).
