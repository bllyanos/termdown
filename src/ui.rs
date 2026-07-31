use crate::{
    app::{App, SearchState},
    markdown::RenderedDocument,
};
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    text::{Line, Span},
    widgets::{Block, Padding, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState, Wrap},
};

#[derive(Debug)]
struct DocumentLayout {
    visual_offsets: Vec<usize>,
    content_rows: usize,
    max_code_width: usize,
}

/// Draw the complete reader frame.
///
/// The body is measured before it is rendered. Prose uses the same wrapped
/// line count as its paragraph, while fenced code rows remain one visual row
/// and are horizontally clipped inside their tinted block.
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

    // Keep the scrollbar in a dedicated one-cell strip so the document never
    // wraps beneath it. At a one-cell-wide terminal the content area is zero
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
        let layout = measure_document(&app.document, content_area.width);
        let viewport_rows = usize::from(content_area.height);
        let inner_width = usize::from(content_area.width.saturating_sub(2));
        let horizontal_max = layout.max_code_width.saturating_sub(inner_width);

        app.update_layout(layout.content_rows, viewport_rows, horizontal_max);

        if let Some(logical_line) = app.state.pending_jump.take() {
            app.state.scroll = logical_line_to_visual_offset(&app.document, logical_line, &layout);
            app.clamp_scroll();
        }

        render_document(frame, content_area, app, &layout);

        if scrollbar_area.width > 0 && scrollbar_area.height > 0 && layout.content_rows > 0 {
            let mut scrollbar_state = ScrollbarState::new(layout.content_rows)
                .position(app.state.scroll)
                .viewport_content_length(viewport_rows);
            let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight)
                .thumb_style(app.theme.scrollbar_style())
                .track_style(app.theme.scrollbar_style());
            frame.render_stateful_widget(scrollbar, scrollbar_area, &mut scrollbar_state);
        }
    } else {
        // Preserve a pending search target across a temporarily unusable
        // terminal. It will be converted to a visual offset on the first
        // usable resize.
        app.update_layout(0, 0, 0);
    }

    render_header(frame, header_area, app);
    render_footer(frame, footer_area, app);
}

fn measure_document(document: &RenderedDocument, width: u16) -> DocumentLayout {
    let mut visual_offsets = Vec::with_capacity(document.text.lines.len());
    let mut content_rows: usize = 0;
    let mut max_code_width = 0;

    for (logical_line, line) in document.text.lines.iter().enumerate() {
        visual_offsets.push(content_rows);
        if is_code_line(document, logical_line) {
            content_rows = content_rows.saturating_add(1);
            max_code_width = max_code_width.max(line.width());
        } else {
            content_rows = content_rows.saturating_add(
                Paragraph::new(line.clone())
                    .wrap(Wrap { trim: false })
                    .line_count(width),
            );
        }
    }

    DocumentLayout {
        visual_offsets,
        content_rows,
        max_code_width,
    }
}

fn is_code_line(document: &RenderedDocument, logical_line: usize) -> bool {
    document
        .code_blocks
        .iter()
        .any(|range| range.contains(&logical_line))
}

fn render_document(frame: &mut Frame, area: Rect, app: &App, layout: &DocumentLayout) {
    frame.render_widget(Block::default().style(app.theme.body_style()), area);

    let visible_start = app.state.scroll;
    let visible_end = visible_start.saturating_add(app.state.viewport_rows);
    let inner_width = usize::from(area.width.saturating_sub(2));

    for (logical_line, line) in app.document.text.lines.iter().enumerate() {
        let visual_start = layout.visual_offsets[logical_line];
        let visual_end = layout
            .visual_offsets
            .get(logical_line + 1)
            .copied()
            .unwrap_or(layout.content_rows);
        if visual_end <= visible_start || visual_start >= visible_end {
            continue;
        }

        let clipped_start = visual_start.max(visible_start);
        let clipped_end = visual_end.min(visible_end);
        let row = usize::from(area.y).saturating_add(clipped_start - visible_start);
        let rect = Rect {
            x: area.x,
            y: row.min(u16::MAX as usize) as u16,
            width: area.width,
            height: (clipped_end - clipped_start).min(u16::MAX as usize) as u16,
        };
        let local_vertical_scroll = clipped_start.saturating_sub(visual_start);

        if is_code_line(&app.document, logical_line) {
            let local_horizontal_max = line.width().saturating_sub(inner_width);
            let horizontal_scroll = app
                .state
                .horizontal_scroll
                .min(local_horizontal_max)
                .min(usize::from(u16::MAX)) as u16;
            let block = Block::default()
                .style(app.theme.code_block())
                .padding(Padding::horizontal(1));
            let paragraph = Paragraph::new(line.clone())
                .style(app.theme.code_block())
                .block(block);
            frame.render_widget(
                paragraph.scroll((
                    local_vertical_scroll.min(usize::from(u16::MAX)) as u16,
                    horizontal_scroll,
                )),
                rect,
            );
        } else {
            let paragraph = Paragraph::new(line.clone())
                .style(app.theme.body_style())
                .wrap(Wrap { trim: false });
            frame.render_widget(
                paragraph.scroll((local_vertical_scroll.min(usize::from(u16::MAX)) as u16, 0)),
                rect,
            );
        }
    }
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
            Line::from("←/→ h/l  ↑/↓ j/k  PgUp/PgDn b/f  g/G  / search  q quit"),
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
fn logical_line_to_visual_offset(
    document: &RenderedDocument,
    logical_line: usize,
    layout: &DocumentLayout,
) -> usize {
    if document.text.lines.is_empty() {
        0
    } else {
        layout
            .visual_offsets
            .get(logical_line)
            .copied()
            .unwrap_or(layout.content_rows)
    }
}

#[cfg(test)]
mod tests {
    use super::{logical_line_to_visual_offset, measure_document, render};
    use crate::{app::App, markdown::RenderedDocument, theme::Theme};
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use ratatui::{
        Terminal,
        backend::TestBackend,
        text::{Line, Text},
    };

    fn document(
        lines: Vec<Line<'static>>,
        code_blocks: Vec<std::ops::Range<usize>>,
    ) -> RenderedDocument {
        RenderedDocument {
            text: Text::from(lines),
            code_blocks,
        }
    }

    #[test]
    fn test_backend_draws_header_body_footer_and_scrollbar() {
        let mut lines = vec![Line::from("# Fixture")];
        lines.extend((0..63).map(|index| Line::from(format!("line {index}: a long enough body"))));
        let document = document(lines, Vec::new());
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
    fn code_rows_are_tinted_padded_and_horizontally_scrollable() {
        let source = "0123456789ABCDEFGHIJabcdefghij";
        let document = document(
            vec![Line::from("prose"), Line::from(source), Line::from("after")],
            vec![1..2],
        );
        let theme = Theme::default();
        let mut app = App::new(document, "fixture.md".to_owned(), theme.clone());
        let mut terminal = Terminal::new(TestBackend::new(24, 8)).expect("test backend");

        terminal
            .draw(|frame| render(frame, &mut app))
            .expect("initial draw succeeds");
        let initial_rows = app.state.content_rows;
        let buffer = terminal.backend().buffer();
        assert_eq!(buffer[(0, 2)].symbol(), " ");
        assert_eq!(buffer[(1, 2)].symbol(), "0");
        assert_eq!(buffer[(0, 2)].bg, theme.code_background);
        let rendered = buffer
            .content
            .iter()
            .map(|cell| cell.symbol().to_owned())
            .collect::<String>();
        assert!(!rendered.contains('╭'));
        assert!(!rendered.contains('╰'));
        assert!(!rendered.contains('│'));

        for _ in 0..5 {
            assert!(app.handle_key(KeyEvent::new(KeyCode::Right, KeyModifiers::NONE,)));
        }
        terminal
            .draw(|frame| render(frame, &mut app))
            .expect("scrolled draw succeeds");
        assert_eq!(app.state.content_rows, initial_rows);
        assert_eq!(terminal.backend().buffer()[(1, 2)].symbol(), "5");

        for _ in 0..5 {
            assert!(app.handle_key(KeyEvent::new(KeyCode::Left, KeyModifiers::NONE,)));
        }
        terminal
            .draw(|frame| render(frame, &mut app))
            .expect("returned draw succeeds");
        assert_eq!(terminal.backend().buffer()[(1, 2)].symbol(), "0");
        assert!((1u16..7u16).any(|row| {
            let cell = &terminal.backend().buffer()[(23, row)];
            !cell.symbol().trim().is_empty()
        }));
    }

    #[test]
    fn wrapped_logical_jump_counts_visual_prefix_rows() {
        let document = document(
            vec![
                Line::from("short"),
                Line::from("this line is deliberately wider than the viewport"),
                Line::from("target"),
            ],
            Vec::new(),
        );
        let layout = measure_document(&document, 10);
        assert_eq!(logical_line_to_visual_offset(&document, 0, &layout), 0);
        assert!(logical_line_to_visual_offset(&document, 2, &layout) > 2);
    }
}
