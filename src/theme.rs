use std::{fs, path::Path, str::FromStr};

use color_eyre::eyre::{Result, eyre};
use ratatui::style::{Color, Modifier, Style};
use serde::Deserialize;

/// Semantic colors shared by the Markdown renderer and terminal chrome.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Theme {
    pub background: Color,
    pub foreground: Color,
    pub muted: Color,
    pub heading: Color,
    pub heading_alt: Color,
    pub link: Color,
    pub code_foreground: Color,
    pub code_background: Color,
    pub blockquote: Color,
    pub table_border: Color,
    pub image: Color,
    pub accent: Color,
    pub border: Color,
    pub status: Color,
    pub search: Color,
}

impl Default for Theme {
    fn default() -> Self {
        Self {
            background: Color::Rgb(20, 22, 26),
            foreground: Color::Rgb(230, 232, 235),
            muted: Color::Rgb(145, 150, 160),
            heading: Color::Rgb(255, 196, 86),
            heading_alt: Color::Rgb(120, 200, 255),
            link: Color::Rgb(110, 190, 255),
            code_foreground: Color::Rgb(235, 240, 245),
            code_background: Color::Rgb(42, 46, 54),
            blockquote: Color::Rgb(130, 210, 185),
            table_border: Color::Rgb(170, 180, 195),
            image: Color::Rgb(230, 150, 220),
            accent: Color::Rgb(115, 220, 220),
            border: Color::Rgb(90, 100, 115),
            status: Color::Rgb(170, 220, 170),
            search: Color::Rgb(255, 190, 90),
        }
    }
}

impl Theme {
    pub fn from_path(path: &Path) -> Result<Self> {
        let input = fs::read_to_string(path)
            .map_err(|error| eyre!("failed to read theme '{}': {error}", path.display()))?;
        Self::from_toml_str_at(&input, Some(path))
    }

    pub fn from_toml_str(input: &str) -> Result<Self> {
        Self::from_toml_str_at(input, None)
    }

    fn from_toml_str_at(input: &str, path: Option<&Path>) -> Result<Self> {
        let overrides: ThemeOverrides = toml::from_str(input).map_err(|error| {
            if let Some(path) = path {
                eyre!("failed to parse theme '{}': {error}", path.display())
            } else {
                eyre!("failed to parse theme TOML: {error}")
            }
        })?;

        let mut theme = Self::default();
        apply_override(
            &mut theme.background,
            "background",
            overrides.background.as_deref(),
            path,
        )?;
        apply_override(
            &mut theme.foreground,
            "foreground",
            overrides.foreground.as_deref(),
            path,
        )?;
        apply_override(&mut theme.muted, "muted", overrides.muted.as_deref(), path)?;
        apply_override(
            &mut theme.heading,
            "heading",
            overrides.heading.as_deref(),
            path,
        )?;
        apply_override(
            &mut theme.heading_alt,
            "heading_alt",
            overrides.heading_alt.as_deref(),
            path,
        )?;
        apply_override(&mut theme.link, "link", overrides.link.as_deref(), path)?;
        apply_override(
            &mut theme.code_foreground,
            "code_foreground",
            overrides.code_foreground.as_deref(),
            path,
        )?;
        apply_override(
            &mut theme.code_background,
            "code_background",
            overrides.code_background.as_deref(),
            path,
        )?;
        apply_override(
            &mut theme.blockquote,
            "blockquote",
            overrides.blockquote.as_deref(),
            path,
        )?;
        apply_override(
            &mut theme.table_border,
            "table_border",
            overrides.table_border.as_deref(),
            path,
        )?;
        apply_override(&mut theme.image, "image", overrides.image.as_deref(), path)?;
        apply_override(
            &mut theme.accent,
            "accent",
            overrides.accent.as_deref(),
            path,
        )?;
        apply_override(
            &mut theme.border,
            "border",
            overrides.border.as_deref(),
            path,
        )?;
        apply_override(
            &mut theme.status,
            "status",
            overrides.status.as_deref(),
            path,
        )?;
        apply_override(
            &mut theme.search,
            "search",
            overrides.search.as_deref(),
            path,
        )?;
        Ok(theme)
    }

    pub(crate) fn body_style(&self) -> Style {
        style(self.foreground, self.background)
    }

    pub(crate) fn heading(&self, level: u8) -> Style {
        let (color, modifiers) = match level {
            1 => (self.heading, Modifier::BOLD),
            2 => (self.heading_alt, Modifier::BOLD | Modifier::UNDERLINED),
            3 => (self.heading, Modifier::ITALIC),
            _ => (self.heading_alt, Modifier::ITALIC | Modifier::UNDERLINED),
        };
        style(color, self.background).add_modifier(modifiers)
    }

    pub(crate) fn inline_code(&self) -> Style {
        style(self.code_foreground, self.code_background)
    }

    pub(crate) fn code_block(&self) -> Style {
        style(self.code_foreground, self.code_background)
    }

    pub(crate) fn link(&self) -> Style {
        style(self.link, self.background).add_modifier(Modifier::UNDERLINED)
    }

    pub(crate) fn blockquote(&self) -> Style {
        style(self.blockquote, self.background).add_modifier(Modifier::ITALIC)
    }

    pub(crate) fn muted(&self) -> Style {
        style(self.muted, self.background)
    }

    pub(crate) fn image(&self) -> Style {
        style(self.image, self.background).add_modifier(Modifier::ITALIC)
    }

    pub(crate) fn table_header(&self) -> Style {
        style(self.accent, self.background).add_modifier(Modifier::BOLD)
    }

    pub(crate) fn table_cell(&self) -> Style {
        style(self.foreground, self.background)
    }

    pub(crate) fn table_border(&self) -> Style {
        style(self.table_border, self.background).add_modifier(Modifier::BOLD)
    }

    pub(crate) fn header_style(&self) -> Style {
        style(self.accent, self.background).add_modifier(Modifier::BOLD)
    }

    pub(crate) fn footer_style(&self) -> Style {
        style(self.status, self.background)
    }

    pub(crate) fn scrollbar_style(&self) -> Style {
        style(self.border, self.background)
    }

    pub(crate) fn search_style(&self) -> Style {
        style(self.search, self.background).add_modifier(Modifier::BOLD)
    }
}

fn style(foreground: Color, background: Color) -> Style {
    Style::default().fg(foreground).bg(background)
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct ThemeOverrides {
    background: Option<String>,
    foreground: Option<String>,
    muted: Option<String>,
    heading: Option<String>,
    heading_alt: Option<String>,
    link: Option<String>,
    code_foreground: Option<String>,
    code_background: Option<String>,
    blockquote: Option<String>,
    table_border: Option<String>,
    image: Option<String>,
    accent: Option<String>,
    border: Option<String>,
    status: Option<String>,
    search: Option<String>,
}

fn apply_override(
    target: &mut Color,
    field: &'static str,
    value: Option<&str>,
    path: Option<&Path>,
) -> Result<()> {
    if let Some(value) = value {
        *target = parse_color(field, value, path)?;
    }
    Ok(())
}

fn parse_color(field: &'static str, value: &str, path: Option<&Path>) -> Result<Color> {
    let literal = value.trim();
    let parsed = if let Some(hex) = literal.strip_prefix('#') {
        (hex.len() == 6 && hex.bytes().all(|byte| byte.is_ascii_hexdigit())).then(|| {
            let red = u8::from_str_radix(&hex[0..2], 16).expect("validated hexadecimal pair");
            let green = u8::from_str_radix(&hex[2..4], 16).expect("validated hexadecimal pair");
            let blue = u8::from_str_radix(&hex[4..6], 16).expect("validated hexadecimal pair");
            Color::Rgb(red, green, blue)
        })
    } else {
        Color::from_str(literal).ok()
    };

    parsed.ok_or_else(|| {
        let location = path.map(|path| format!(" in theme '{}'", path.display())).unwrap_or_default();
        eyre!("invalid color for field '{field}'{location}: expected an ANSI color name or #RRGGBB, got '{value}'")
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_palette_is_dark_and_high_contrast() {
        let theme = Theme::default();
        assert_eq!(theme.background, Color::Rgb(20, 22, 26));
        assert_eq!(theme.foreground, Color::Rgb(230, 232, 235));
        assert_ne!(theme.background, theme.foreground);
        assert_ne!(theme.heading, theme.heading_alt);
    }

    #[test]
    fn partial_toml_overrides_inherit_defaults() {
        let theme = Theme::from_toml_str(
            r##"
                foreground = "#00ffcc"
                heading = "yellow"
            "##,
        )
        .expect("partial theme should parse");
        let default = Theme::default();

        assert_eq!(theme.foreground, Color::Rgb(0, 255, 204));
        assert_eq!(theme.heading, Color::Yellow);
        assert_eq!(theme.background, default.background);
        assert_eq!(theme.link, default.link);
    }

    #[test]
    fn parses_named_and_hex_colors() {
        let theme = Theme::from_toml_str(
            r##"
                link = "light-blue"
                accent = "#aBcD09"
            "##,
        )
        .expect("named and hex colors should parse");

        assert_eq!(theme.link, Color::LightBlue);
        assert_eq!(theme.accent, Color::Rgb(171, 205, 9));
    }

    #[test]
    fn rejects_unknown_keys() {
        let error = Theme::from_toml_str("unknown = \"red\"").expect_err("unknown key must fail");
        assert!(error.to_string().contains("unknown field"));
    }

    #[test]
    fn invalid_color_identifies_field() {
        let error =
            Theme::from_toml_str("heading = \"not-a-color\"").expect_err("invalid color must fail");
        let message = error.to_string();
        assert!(message.contains("heading"));
        assert!(message.contains("#RRGGBB"));
    }
}
