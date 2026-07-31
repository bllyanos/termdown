use crate::theme::Theme;
use color_eyre::Result;
use comrak::nodes::{
    AstNode, ListDelimType, ListType, NodeCodeBlock, NodeList, NodeTable, NodeValue, TableAlignment,
};
use comrak::{Arena, Options, parse_document};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span, Text};
use unicode_width::UnicodeWidthStr;

pub struct RenderedDocument {
    pub text: Text<'static>,
}

pub fn render(source: &str, theme: &Theme) -> Result<RenderedDocument> {
    let options = gfm_options();
    let arena = Arena::new();
    let root = parse_document(&arena, source, &options);
    let renderer = Renderer { theme };
    let mut lines = renderer.render_block_children(root, 0);
    if lines.is_empty() {
        lines.push(Line::default());
    }
    Ok(RenderedDocument {
        text: Text::from(lines),
    })
}

fn gfm_options() -> Options<'static> {
    let mut options = Options::default();
    options.extension.strikethrough = true;
    options.extension.tagfilter = true;
    options.extension.table = true;
    options.extension.autolink = true;
    options.extension.tasklist = true;
    options.render.r#unsafe = false;
    options
}

struct Renderer<'a> {
    theme: &'a Theme,
}

impl Renderer<'_> {
    fn render_block_children<'n>(
        &self,
        node: &'n AstNode<'n>,
        indent: usize,
    ) -> Vec<Line<'static>> {
        let mut out = Vec::new();
        let mut blocks = 0;
        for child in node.children() {
            let lines = self.render_block(child, indent);
            if lines.is_empty() {
                continue;
            }
            if blocks > 0 {
                push_blank_line(&mut out);
            }
            out.extend(lines);
            blocks += 1;
        }
        out
    }

    fn render_block<'n>(&self, node: &'n AstNode<'n>, indent: usize) -> Vec<Line<'static>> {
        let value = node.data.borrow().value.clone();
        match value {
            NodeValue::Document => self.render_block_children(node, indent),
            NodeValue::Paragraph => {
                let mut out = Vec::new();
                self.render_inline_children(node, &mut out, self.theme.body_style());
                prefix_lines(&mut out, indent, "", self.theme.body_style());
                out
            }
            NodeValue::Heading(heading) => {
                let style = self.theme.heading(heading.level);
                let mut out = Vec::new();
                self.render_inline_children(node, &mut out, style);
                if out.is_empty() {
                    out.push(Line::default());
                }
                prepend(
                    &mut out[0],
                    format!("{} ", "#".repeat(heading.level as usize)),
                    style,
                );
                prefix_lines(&mut out, indent, "", style);
                out
            }
            NodeValue::BlockQuote | NodeValue::MultilineBlockQuote(_) => {
                let mut out = self.render_block_children(node, 0);
                if out.is_empty() {
                    out.push(Line::default());
                }
                prefix_lines(&mut out, indent, "│ ", self.theme.blockquote());
                out
            }
            NodeValue::List(list) => self.render_list(node, &list, indent),
            NodeValue::Item(_) | NodeValue::TaskItem(_) => self.render_block_children(node, indent),
            NodeValue::ThematicBreak => {
                let mut out = vec![Line::from(Span::styled(
                    "────".to_owned(),
                    self.theme.table_border(),
                ))];
                prefix_lines(&mut out, indent, "", self.theme.table_border());
                out
            }
            NodeValue::CodeBlock(code) => self.render_code_block(&code, indent),
            NodeValue::Table(table) => self.render_table(node, &table, indent),
            NodeValue::HtmlBlock(html) => {
                self.render_literal_block(&html.literal, indent, self.theme.muted())
            }
            NodeValue::TableRow(_) | NodeValue::TableCell => {
                self.render_fallback_block(node, indent)
            }
            _ => self.render_fallback_block(node, indent),
        }
    }

    fn render_list<'n>(
        &self,
        node: &'n AstNode<'n>,
        list: &NodeList,
        indent: usize,
    ) -> Vec<Line<'static>> {
        let mut out = Vec::new();
        for (index, item) in node.children().enumerate() {
            let task = match item.data.borrow().value.clone() {
                NodeValue::TaskItem(t) => t.symbol,
                _ => None,
            };
            let marker = match list.list_type {
                ListType::Bullet => "- ".to_owned(),
                ListType::Ordered => {
                    let delimiter = match list.delimiter {
                        ListDelimType::Period => '.',
                        ListDelimType::Paren => ')',
                    };
                    format!("{}{delimiter} ", list.start.saturating_add(index))
                }
            };
            let task_marker = if list.is_task_list {
                if matches!(task, Some('x' | 'X')) {
                    "[x] "
                } else {
                    "[ ] "
                }
            } else {
                ""
            };
            let marker = format!("{marker}{task_marker}");
            let width = display_width(&marker);
            let mut lines = self.render_item_content(item, indent, width);
            if lines.is_empty() {
                lines.push(Line::default());
            }
            if !out.is_empty() {
                push_blank_line(&mut out);
            }
            prepend(&mut lines[0], marker, self.theme.body_style());
            prefix_after_first(&mut lines, width, self.theme.body_style());
            prefix_lines(&mut lines, indent, "", self.theme.body_style());
            out.extend(lines);
        }
        out
    }

    fn render_item_content<'n>(
        &self,
        item: &'n AstNode<'n>,
        indent: usize,
        marker_width: usize,
    ) -> Vec<Line<'static>> {
        let mut out = Vec::new();
        let mut first = true;
        for child in item.children() {
            let value = child.data.borrow().value.clone();
            let is_list = matches!(&value, NodeValue::List(_));
            let mut lines = match value {
                NodeValue::Paragraph => {
                    let mut v = Vec::new();
                    self.render_inline_children(child, &mut v, self.theme.body_style());
                    v
                }
                NodeValue::List(ref list) => self.render_list(child, list, 4),
                _ => self.render_block(child, 0),
            };
            if lines.is_empty() {
                continue;
            }
            if !out.is_empty() {
                push_blank_line(&mut out);
            }
            if is_list {
                out.extend(lines);
                first = false;
                continue;
            }
            for line in &mut lines {
                if first {
                    first = false;
                } else {
                    prepend(line, " ".repeat(marker_width), self.theme.body_style());
                }
            }
            out.extend(lines);
        }
        out
    }

    fn render_code_block(&self, code: &NodeCodeBlock, indent: usize) -> Vec<Line<'static>> {
        let style = self.theme.code_block();
        let info = sanitize_literal(&code.info);
        let mut out = vec![Line::from(Span::styled(
            if info.is_empty() {
                "╭─".to_owned()
            } else {
                format!("╭─ {info}")
            },
            style,
        ))];
        let raw: Vec<&str> = code.literal.split('\n').collect();
        let last = raw.len().saturating_sub(1);
        for (i, line) in raw.into_iter().enumerate() {
            if i == last && line.is_empty() && code.literal.ends_with('\n') {
                continue;
            }
            out.push(Line::from(Span::styled(
                format!("│ {}", sanitize_literal(line)),
                style,
            )));
        }
        out.push(Line::from(Span::styled("╰─".to_owned(), style)));
        prefix_lines(&mut out, indent, "", style);
        out
    }

    fn render_literal_block(
        &self,
        literal: &str,
        indent: usize,
        style: Style,
    ) -> Vec<Line<'static>> {
        let mut out = literal
            .split('\n')
            .map(|part| Line::from(Span::styled(sanitize_literal(part), style)))
            .collect::<Vec<_>>();
        prefix_lines(&mut out, indent, "", style);
        out
    }

    fn render_table<'n>(
        &self,
        node: &'n AstNode<'n>,
        table: &NodeTable,
        indent: usize,
    ) -> Vec<Line<'static>> {
        let mut rows = Vec::new();
        for row in node.children() {
            let header = matches!(row.data.borrow().value.clone(), NodeValue::TableRow(true));
            let mut cells = Vec::new();
            for cell in row.children() {
                let mut cell_lines = Vec::new();
                self.render_inline_children(cell, &mut cell_lines, self.theme.table_cell());
                if cell_lines.is_empty() {
                    cell_lines.push(Line::default());
                }
                let mut spans = Vec::new();
                for (i, line) in cell_lines.into_iter().enumerate() {
                    if i > 0 {
                        spans.push(Span::styled(" ".to_owned(), self.theme.table_cell()));
                    }
                    spans.extend(line.spans);
                }
                if spans.is_empty() {
                    spans.push(Span::styled("".to_owned(), self.theme.table_cell()));
                }
                cells.push(spans);
            }
            rows.push((header, cells));
        }
        let row_columns = rows.iter().map(|(_, cells)| cells.len()).max().unwrap_or(0);
        let cols = table
            .num_columns
            .max(table.alignments.len())
            .max(row_columns);
        if cols == 0 {
            return Vec::new();
        }
        for (_, cells) in &mut rows {
            while cells.len() < cols {
                cells.push(vec![Span::styled("".to_owned(), self.theme.table_cell())]);
            }
        }
        let mut widths = vec![0; cols];
        for (_, cells) in &rows {
            for (i, spans) in cells.iter().enumerate() {
                widths[i] = widths[i].max(
                    spans
                        .iter()
                        .map(|s| display_width(s.content.as_ref()))
                        .sum::<usize>(),
                );
            }
        }
        let mut out = vec![self.border_line(&widths, '┌', '┬', '┐')];
        for (ri, (header, cells)) in rows.iter().enumerate() {
            let cell_style = if *header {
                self.theme.table_header()
            } else {
                self.theme.table_cell()
            };
            let mut line = Line::default();
            line.spans
                .push(Span::styled("│".to_owned(), self.theme.table_border()));
            for (i, spans) in cells.iter().enumerate() {
                let used: usize = spans
                    .iter()
                    .map(|s| display_width(s.content.as_ref()))
                    .sum();
                let extra = widths[i].saturating_sub(used);
                let (left, right) = match table
                    .alignments
                    .get(i)
                    .copied()
                    .unwrap_or(TableAlignment::None)
                {
                    TableAlignment::Right => (extra, 0),
                    TableAlignment::Center => (extra / 2, extra - extra / 2),
                    _ => (0, extra),
                };
                line.spans.push(Span::styled(" ".to_owned(), cell_style));
                if left > 0 {
                    line.spans.push(Span::styled(" ".repeat(left), cell_style));
                }
                for span in spans {
                    line.spans.push(Span {
                        style: span.style.patch(cell_style),
                        content: span.content.clone(),
                    });
                }
                if right > 0 {
                    line.spans.push(Span::styled(" ".repeat(right), cell_style));
                }
                line.spans.push(Span::styled(" ".to_owned(), cell_style));
                line.spans
                    .push(Span::styled("│".to_owned(), self.theme.table_border()));
            }
            out.push(line);
            if ri + 1 < rows.len() {
                out.push(self.border_line(&widths, '├', '┼', '┤'));
            }
        }
        out.push(self.border_line(&widths, '└', '┴', '┘'));
        prefix_lines(&mut out, indent, "", self.theme.table_cell());
        out
    }

    fn border_line(
        &self,
        widths: &[usize],
        left: char,
        junction: char,
        right: char,
    ) -> Line<'static> {
        let mut s = left.to_string();
        for (i, width) in widths.iter().enumerate() {
            s.push_str(&"─".repeat(width + 2));
            s.push(if i + 1 == widths.len() {
                right
            } else {
                junction
            });
        }
        Line::from(Span::styled(s, self.theme.table_border()))
    }

    fn render_fallback_block<'n>(
        &self,
        node: &'n AstNode<'n>,
        indent: usize,
    ) -> Vec<Line<'static>> {
        if node.children().next().is_some() {
            let mut lines = self.render_block_children(node, indent);
            mute_lines(&mut lines, self.theme.muted());
            return lines;
        }
        let value = node.data.borrow().value.clone();
        let literal = match value {
            NodeValue::Text(v) => Some(v.into_owned()),
            NodeValue::FrontMatter(v) | NodeValue::Raw(v) => Some(v),
            NodeValue::Math(v) => Some(v.literal),
            NodeValue::HtmlInline(v) => Some(v),
            NodeValue::FootnoteDefinition(v) => Some(v.name),
            NodeValue::FootnoteReference(v) => Some(v.name),
            NodeValue::WikiLink(v) => Some(v.url),
            NodeValue::Code(v) => Some(v.literal),
            _ => None,
        };
        literal
            .map(|v| self.render_literal_block(&v, indent, self.theme.muted()))
            .unwrap_or_default()
    }

    fn render_inline_children<'n>(
        &self,
        node: &'n AstNode<'n>,
        lines: &mut Vec<Line<'static>>,
        inherited: Style,
    ) {
        for child in node.children() {
            self.render_inline_node(child, lines, inherited);
        }
    }

    fn render_inline_node<'n>(
        &self,
        node: &'n AstNode<'n>,
        lines: &mut Vec<Line<'static>>,
        inherited: Style,
    ) {
        let value = node.data.borrow().value.clone();
        match value {
            NodeValue::Text(v) => append_sanitized(lines, v.as_ref(), inherited),
            NodeValue::SoftBreak => append_span(lines, Span::styled(" ".to_owned(), inherited)),
            NodeValue::LineBreak => {
                ensure_line(lines);
                lines.push(Line::default());
            }
            NodeValue::Emph => self.render_inline_children(
                node,
                lines,
                inherited.patch(Style::default().add_modifier(Modifier::ITALIC)),
            ),
            NodeValue::Strong => self.render_inline_children(
                node,
                lines,
                inherited.patch(Style::default().add_modifier(Modifier::BOLD)),
            ),
            NodeValue::WikiLink(v) => {
                append_sanitized(lines, &v.url, inherited.patch(self.theme.muted()))
            }
            NodeValue::FootnoteReference(v) => {
                append_sanitized(lines, &v.name, inherited.patch(self.theme.muted()))
            }
            NodeValue::FootnoteDefinition(v) => {
                append_sanitized(lines, &v.name, inherited.patch(self.theme.muted()))
            }
            NodeValue::EscapedTag(v) => {
                append_sanitized(lines, v, inherited.patch(self.theme.muted()))
            }
            NodeValue::Code(v) => {
                append_sanitized(lines, &v.literal, inherited.patch(self.theme.inline_code()))
            }
            NodeValue::Link(v) => {
                let label = self.plain_inline_children(node);
                self.render_inline_children(node, lines, inherited.patch(self.theme.link()));
                let dest = v.url;
                if label != dest {
                    append_sanitized(
                        lines,
                        &format!(" ({dest})"),
                        inherited.patch(self.theme.muted()),
                    );
                }
            }
            NodeValue::Image(v) => {
                let alt = self.plain_inline_children(node);
                let text = if alt.is_empty() { v.url } else { alt };
                append_sanitized(
                    lines,
                    &format!("[img] {text}"),
                    inherited.patch(self.theme.image()),
                );
            }
            NodeValue::TaskItem(v) => {
                let marker = if matches!(v.symbol, Some('x' | 'X')) {
                    "[x] "
                } else {
                    "[ ] "
                };
                append_span(lines, Span::styled(marker.to_owned(), inherited));
                self.render_inline_children(node, lines, inherited);
            }
            NodeValue::HtmlInline(v) => {
                append_sanitized(lines, &v, inherited.patch(self.theme.muted()))
            }
            NodeValue::HtmlBlock(v) => {
                append_sanitized(lines, &v.literal, inherited.patch(self.theme.muted()))
            }
            NodeValue::FrontMatter(v) | NodeValue::Raw(v) => {
                append_sanitized(lines, &v, inherited.patch(self.theme.muted()))
            }
            NodeValue::Math(v) => {
                append_sanitized(lines, &v.literal, inherited.patch(self.theme.muted()))
            }
            NodeValue::Document
            | NodeValue::Paragraph
            | NodeValue::Heading(_)
            | NodeValue::BlockQuote
            | NodeValue::List(_)
            | NodeValue::Item(_)
            | NodeValue::ThematicBreak
            | NodeValue::CodeBlock(_)
            | NodeValue::Table(_)
            | NodeValue::TableRow(_)
            | NodeValue::TableCell => self.render_inline_children(node, lines, inherited),
            _ => self.render_inline_children(node, lines, inherited.patch(self.theme.muted())),
        }
    }

    fn plain_inline_children<'n>(&self, node: &'n AstNode<'n>) -> String {
        let mut out = String::new();
        for child in node.children() {
            self.plain_inline_node(child, &mut out);
        }
        out
    }
    fn plain_inline_node<'n>(&self, node: &'n AstNode<'n>, out: &mut String) {
        match node.data.borrow().value.clone() {
            NodeValue::Text(v) => out.push_str(v.as_ref()),
            NodeValue::Code(v) => out.push_str(&v.literal),
            NodeValue::SoftBreak | NodeValue::LineBreak => out.push(' '),
            NodeValue::Image(v) | NodeValue::Link(v) => {
                let start = out.len();
                for child in node.children() {
                    self.plain_inline_node(child, out);
                }
                if out.len() == start {
                    out.push_str(&v.url);
                }
            }
            NodeValue::HtmlInline(v) => out.push_str(&v),
            NodeValue::Math(v) => out.push_str(&v.literal),
            NodeValue::WikiLink(v) => out.push_str(&v.url),
            _ => {
                for child in node.children() {
                    self.plain_inline_node(child, out);
                }
            }
        }
    }
}

fn ensure_line(lines: &mut Vec<Line<'static>>) {
    if lines.is_empty() {
        lines.push(Line::default());
    }
}
fn is_blank_line(line: &Line<'static>) -> bool {
    line.spans.iter().all(|span| span.content.trim().is_empty())
}

fn append_span(lines: &mut Vec<Line<'static>>, span: Span<'static>) {
    ensure_line(lines);
    lines.last_mut().unwrap().spans.push(span);
}
fn append_sanitized(lines: &mut Vec<Line<'static>>, value: &str, style: Style) {
    let parts: Vec<&str> = value.split('\n').collect();
    for (i, part) in parts.iter().enumerate() {
        append_span(lines, Span::styled(sanitize_literal(part), style));
        if i + 1 < parts.len() {
            lines.push(Line::default());
        }
    }
}
fn sanitize_literal(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for c in value.chars() {
        match c {
            '\t' => out.push_str("    "),
            c if c.is_control() => out.push('\u{fffd}'),
            c => out.push(c),
        }
    }
    out
}
fn display_width(value: &str) -> usize {
    UnicodeWidthStr::width(value)
}
fn push_blank_line(lines: &mut Vec<Line<'static>>) {
    if !lines.last().is_some_and(is_blank_line) {
        lines.push(Line::default());
    }
}
fn prepend(line: &mut Line<'static>, prefix: String, style: Style) {
    if !prefix.is_empty() {
        line.spans.insert(0, Span::styled(prefix, style));
    }
}
fn prefix_lines(lines: &mut [Line<'static>], indent: usize, marker: &str, style: Style) {
    let prefix = format!("{}{}", " ".repeat(indent), marker);
    for line in lines {
        prepend(line, prefix.clone(), style);
    }
}
fn prefix_after_first(lines: &mut [Line<'static>], width: usize, style: Style) {
    for line in lines.iter_mut().skip(1) {
        let nested = line
            .spans
            .first()
            .is_some_and(|span| span.content.starts_with("    "));
        if !nested {
            prepend(line, " ".repeat(width), style);
        }
    }
}

fn mute_lines(lines: &mut [Line<'static>], style: Style) {
    for line in lines {
        line.style = line.style.patch(style);
        for span in &mut line.spans {
            span.style = span.style.patch(style);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn gfm_options_enable_only_selected_extensions() {
        let o = gfm_options();
        assert!(
            o.extension.strikethrough
                && o.extension.tagfilter
                && o.extension.table
                && o.extension.autolink
                && o.extension.tasklist
        );
        assert!(
            !o.extension.superscript
                && !o.extension.footnotes
                && !o.extension.inline_footnotes
                && !o.extension.description_lists
                && !o.extension.multiline_block_quotes
                && !o.extension.alerts
        );
        assert!(
            !o.extension.math_dollars
                && !o.extension.math_latex
                && !o.extension.math_code
                && !o.extension.underline
                && !o.extension.subscript
                && !o.extension.spoiler
                && !o.extension.greentext
        );
        assert!(
            !o.extension.cjk_friendly_emphasis
                && !o.extension.subtext
                && !o.extension.highlight
                && !o.extension.insert
                && !o.extension.block_directive
                && !o.render.r#unsafe
        );
    }
    #[test]
    fn gfm_constructs_and_safety_are_visible() {
        let source = "# GFM\n\n| left | center | right |\n| :--- | :----: | ---: |\n| a | b | c |\n\n- [x] done\n- [ ] todo\n\n~~gone~~ www.example.com <https://example.com>\n\n[link](https://example.com) ![diagram](diagram.png)\n\n<script>\u{1b}[31malert(1)</script>\n\n```rust\n\tlet x = 1;\u{7f}\n```\n";
        let text = render(source, &Theme::default()).unwrap().text;
        let rendered = text
            .lines
            .iter()
            .map(Line::to_string)
            .collect::<Vec<_>>()
            .join("\n");
        assert!(
            rendered.contains("# GFM")
                && rendered.contains("┌")
                && rendered.contains("[x] done")
                && rendered.contains("[ ] todo"),
            "{rendered}"
        );
        assert!(
            rendered.contains("gone")
                && rendered.contains("www.example.com")
                && rendered.contains("https://example.com")
                && rendered.contains("[img] diagram")
                && rendered.contains("╭─ rust")
                && rendered.contains('�')
        );
        assert!(!rendered.contains('\u{1b}'));
    }
    #[test]
    fn sanitize_literal_expands_tabs_and_replaces_controls() {
        assert_eq!(sanitize_literal("a\tb\r\0\u{1b}c"), "a    b���c");
    }
}
