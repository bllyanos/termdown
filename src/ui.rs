use crate::app::{App, SearchState};
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    text::{Line, Span, Text},
    widgets::{Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState, Wrap},
};

/// Draw the complete reader frame.
///
/// The body is measured before it is rendered.  That measurement is important:
/// the document's logical lines are not necessarily visual lines once wrapping is
/// enabled, so search jumps and the scrollbar both use the same wrapped count as
/// the paragraph that is eventually drawn.
pub fn render(frame: &mut Frame, app: &mut App) {
    let vertical_areas = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Min(1),
            Constraint::Length(1),
        ])
        .split(frame.area());
    let header_area = vertical_areas[0];
    let body_area = vertical_areas[1];
    let footer_area = vertical_areas[2];

    let theme = &app.theme;

    // Keep the scrollbar in a dedicated one-cell strip so the document never
    // wraps beneath it.  At a one-cell-wide terminal the content area is zero
    // width; that case is handled below instead of passing width zero to the
    // line-counting API.
    let horizontal_areas = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Min(1), Constraint::Length(1)])
        .split(body_area);
    let content_area = horizontal_areas[0];
    let scrollbar_area = horizontal_areas[1];

    let has_content_area = content_area.width > 0 && content_area.height > 0;
    if has_content_area {
        let paragraph = Paragraph::new(app.document.text.clone())
            .style(theme.body_style())
            .wrap(Wrap { trim: false });
        let content_rows = paragraph.line_count(content_area.width);
        let viewport_rows = usize::from(content_area.height);

        app.state.content_rows = content_rows;
        app.state.viewport_rows = viewport_rows;

        if let Some(logical_line) = app.state.pending_jump.take() {
            let visual_offset =
                logical_line_to_visual_offset(&app.document.text, logical_line, content_area.width);
            app.state.scroll = visual_offset;
        }

        let max_scroll = content_rows.saturating_sub(viewport_rows);
        app.state.scroll = app.state.scroll.min(max_scroll);

        let scroll = app.state.scroll.min(usize::from(u16::MAX)) as u16;
        frame.render_widget(paragraph.scroll((scroll, 0)), content_area);

        if scrollbar_area.width > 0 && scrollbar_area.height > 0 && content_rows > 0 {
            let mut scrollbar_state = ScrollbarState::new(content_rows)
                .position(app.state.scroll)
                .viewport_content_length(viewport_rows);
            let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight)
                .thumb_style(theme.scrollbar_style())
                .track_style(theme.scrollbar_style());
            frame.render_stateful_widget(scrollbar, scrollbar_area, &mut scrollbar_state);
        }
    } else {
        // Preserve a pending search target across a temporarily unusable
        // terminal.  It will be converted to a visual offset on the first
        // usable resize.  Existing scroll arithmetic is reset only because no
        // viewport exists in which it could be meaningful.
        app.state.content_rows = 0;
        app.state.viewport_rows = 0;
        app.state.scroll = 0;
    }

    render_header(frame, header_area, app);
    render_footer(frame, footer_area, app);
}

fn render_header(frame: &mut Frame, area: Rect, app: &App) {
    let indicator = format!("{}/{}", app.state.scroll, app.state.content_rows);
    let line = Line::from(vec![
        Span::raw("termdown"),
        Span::raw("  "),
        Span::raw(app.source_label.as_str()),
        Span::raw("  "),
        Span::raw(indicator),
    ]);
    frame.render_widget(Paragraph::new(line).style(app.theme.header_style()), area);
}

fn render_footer(frame: &mut Frame, area: Rect, app: &App) {
    let (line, style) = if app.state.search.active {
        let summary = match_summary(&app.state.search);
        (
            Line::from(vec![
                Span::raw("/"),
                Span::raw(app.state.search.draft.as_str()),
                Span::raw("  "),
                Span::raw(summary),
                Span::raw("  Enter search  Esc cancel"),
            ]),
            app.theme.search_style(),
        )
    } else {
        (
            Line::from("↑/↓ j/k  PgUp/PgDn b/f  g/G  / search  q quit"),
            app.theme.footer_style(),
        )
    };

    frame.render_widget(Paragraph::new(line).style(style), area);
}

fn match_summary(search: &SearchState) -> String {
    if search.matches.is_empty() {
        "0 matches".to_owned()
    } else if let Some(current) = search.current {
        format!(
            "match {}/{}",
            current.saturating_add(1),
            search.matches.len()
        )
    } else {
        format!("{} matches", search.matches.len())
    }
}

/// Convert a logical rendered-text line into the visual row at which it starts.
///
/// Counting a wrapped prefix rather than multiplying a logical index by a
/// constant keeps jumps correct for prose, tables, code, and any other line
/// whose display width exceeds the current content viewport.
fn logical_line_to_visual_offset(text: &Text<'static>, logical_line: usize, width: u16) -> usize {
    if width == 0 || logical_line == 0 {
        return 0;
    }

    let prefix = Text::from(
        text.lines
            .iter()
            .take(logical_line)
            .cloned()
            .collect::<Vec<_>>(),
    );
    Paragraph::new(prefix)
        .wrap(Wrap { trim: false })
        .line_count(width)
}

#[cfg(test)]
mod tests {
    use super::{logical_line_to_visual_offset, render};
    use crate::{app::App, markdown::RenderedDocument, theme::Theme};
    use ratatui::{
        Terminal,
        backend::TestBackend,
        text::{Line, Text},
    };

    #[test]
    fn test_backend_draws_header_body_footer_and_scrollbar() {
        let mut lines = vec![Line::from("# Fixture")];
        lines.extend((0..63).map(|index| Line::from(format!("line {index}: a long enough body"))));
        let text = Text::from(lines);
        let document = RenderedDocument { text };
        let mut app = App::new(document, "fixture.md".to_owned(), Theme::default());
        let backend = TestBackend::new(80, 20);
        let mut terminal = Terminal::new(backend).expect("test backend");

        terminal
            .draw(|frame| render(frame, &mut app))
            .expect("draw succeeds");

        let buffer = terminal.backend().buffer();
        let rendered = buffer
            .content
            .iter()
            .map(|cell| cell.symbol().to_owned())
            .collect::<String>();
        assert!(rendered.contains("# Fixture"));
        assert!(rendered.contains("line 0"));
        assert!(rendered.contains("q quit"));
        assert!(app.state.content_rows > app.state.viewport_rows);
        assert!((1u16..19u16).any(|row| {
            let cell = &buffer[(79, row)];
            !cell.symbol().trim().is_empty()
        }));
    }

    #[test]
    fn wrapped_logical_jump_counts_visual_prefix_rows() {
        let text = Text::from(vec![
            Line::from("short"),
            Line::from("this line is deliberately wider than the viewport"),
            Line::from("target"),
        ]);
        assert_eq!(logical_line_to_visual_offset(&text, 0, 10), 0);
        assert!(logical_line_to_visual_offset(&text, 2, 10) > 2);
    }
}
