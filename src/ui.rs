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
    let mut logical_line = 0;

    while logical_line < document.text.lines.len() {
        if let Some(range) = document
            .code_blocks
            .iter()
            .find(|range| range.start == logical_line)
        {
            content_rows = content_rows.saturating_add(1);
            for code_line in range.clone() {
                visual_offsets.push(content_rows);
                content_rows = content_rows.saturating_add(1);
                max_code_width = max_code_width.max(document.text.lines[code_line].width());
            }
            content_rows = content_rows.saturating_add(1);
            logical_line = range.end;
        } else {
            visual_offsets.push(content_rows);
            content_rows = content_rows.saturating_add(
                Paragraph::new(document.text.lines[logical_line].clone())
                    .wrap(Wrap { trim: false })
                    .line_count(width),
            );
            logical_line += 1;
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

    for (logical_line, line) in app.document.text.lines.iter().enumerate() {
        if is_code_line(&app.document, logical_line) {
            continue;
        }

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
        let rect = Rect {
            x: area.x,
            y: (usize::from(area.y) + clipped_start - visible_start).min(u16::MAX as usize) as u16,
            width: area.width,
            height: (clipped_end - clipped_start).min(u16::MAX as usize) as u16,
        };
        let paragraph = Paragraph::new(line.clone())
            .style(app.theme.body_style())
            .wrap(Wrap { trim: false });
        frame.render_widget(
            paragraph.scroll((
                clipped_start
                    .saturating_sub(visual_start)
                    .min(usize::from(u16::MAX)) as u16,
                0,
            )),
            rect,
        );
    }

    for range in &app.document.code_blocks {
        render_code_block(frame, area, app, layout, range, visible_start, visible_end);
    }
}

fn render_code_block(
    frame: &mut Frame,
    area: Rect,
    app: &App,
    layout: &DocumentLayout,
    range: &std::ops::Range<usize>,
    visible_start: usize,
    visible_end: usize,
) {
    if range.is_empty() {
        return;
    }

    let source_start = layout.visual_offsets[range.start];
    let block_start = source_start.saturating_sub(1);
    let block_end = layout.visual_offsets[range.end - 1].saturating_add(2);
    if block_end <= visible_start || block_start >= visible_end {
        return;
    }

    let clipped_start = block_start.max(visible_start);
    let clipped_end = block_end.min(visible_end);
    let rect = Rect {
        x: area.x,
        y: (usize::from(area.y) + clipped_start - visible_start).min(u16::MAX as usize) as u16,
        width: area.width,
        height: (clipped_end - clipped_start).min(u16::MAX as usize) as u16,
    };
    let top_padding = usize::from(block_start >= visible_start);
    let bottom_padding = usize::from(block_end <= visible_end);
    let source_skip = visible_start.saturating_sub(source_start);
    let inner_width = usize::from(area.width.saturating_sub(2));
    let raw_width = range
        .clone()
        .map(|line| app.document.text.lines[line].width())
        .max()
        .unwrap_or_default();
    let horizontal_scroll = app
        .state
        .horizontal_scroll
        .min(raw_width.saturating_sub(inner_width))
        .min(usize::from(u16::MAX)) as u16;
    let lines = range
        .clone()
        .map(|line| app.document.text.lines[line].clone())
        .collect::<Vec<_>>();
    let block = Block::default()
        .style(app.theme.code_block())
        .padding(Padding::new(
            1,
            1,
            top_padding as u16,
            bottom_padding as u16,
        ));
    let paragraph = Paragraph::new(lines)
        .style(app.theme.code_block())
        .block(block);
    frame.render_widget(
        paragraph.scroll((
            source_skip.min(usize::from(u16::MAX)) as u16,
            horizontal_scroll,
        )),
        rect,
    );
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
        assert_eq!(buffer[(1, 3)].symbol(), "0");
        assert_eq!(buffer[(0, 2)].bg, theme.code_background);
        assert_eq!(buffer[(0, 4)].bg, theme.code_background);
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
        assert_eq!(terminal.backend().buffer()[(1, 3)].symbol(), "5");

        for _ in 0..5 {
            assert!(app.handle_key(KeyEvent::new(KeyCode::Left, KeyModifiers::NONE,)));
        }
        terminal
            .draw(|frame| render(frame, &mut app))
            .expect("returned draw succeeds");
        assert_eq!(terminal.backend().buffer()[(1, 3)].symbol(), "0");
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
