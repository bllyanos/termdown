use std::cmp::min;

use color_eyre::eyre::WrapErr;
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use ratatui::{
    DefaultTerminal,
    text::Text,
    widgets::{Paragraph, Wrap},
};

use crate::{markdown::RenderedDocument, theme::Theme, ui};

/// The state owned by the running reader application.
pub struct App {
    pub(crate) document: RenderedDocument,
    pub(crate) source_label: String,
    pub(crate) theme: Theme,
    pub(crate) state: ViewState,
}

/// Mutable viewport and search state for the current document.
#[derive(Debug, Default)]
pub struct ViewState {
    pub(crate) scroll: usize,
    pub(crate) content_rows: usize,
    pub(crate) viewport_rows: usize,
    pub(crate) pending_jump: Option<usize>,
    pub(crate) search: SearchState,
    pub(crate) should_quit: bool,
}

/// Search input and committed result state.
#[derive(Debug, Default)]
pub struct SearchState {
    pub(crate) active: bool,
    pub(crate) draft: String,
    pub(crate) committed: String,
    pub(crate) matches: Vec<usize>,
    pub(crate) current: Option<usize>,
}

impl App {
    pub fn new(document: RenderedDocument, source_label: String, theme: Theme) -> Self {
        Self {
            document,
            source_label,
            theme,
            state: ViewState::default(),
        }
    }

    /// Run the static reader event loop until a quit binding is received.
    pub fn run(&mut self, terminal: &mut DefaultTerminal) -> color_eyre::Result<()> {
        terminal.draw(|frame| ui::render(frame, self))?;

        loop {
            let event = event::read().wrap_err("failed to read terminal event")?;
            if self.handle_event(event) {
                terminal.draw(|frame| ui::render(frame, self))?;
            }
            if self.state.should_quit {
                break;
            }
        }

        Ok(())
    }

    /// Handle one terminal event. A return value of `true` means the event
    /// changed reader state (or the terminal was resized) and should be drawn.
    pub(crate) fn handle_event(&mut self, event: Event) -> bool {
        match event {
            Event::Key(key) => self.handle_key(key),
            Event::Resize(_, _) => true,
            _ => false,
        }
    }

    /// Handle one key event, ignoring key releases and repeats.
    pub(crate) fn handle_key(&mut self, key: KeyEvent) -> bool {
        if key.kind != KeyEventKind::Press {
            return false;
        }

        if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
            self.state.should_quit = true;
            return true;
        }

        if self.state.search.active {
            self.handle_search_key(key)
        } else {
            self.handle_normal_key(key)
        }
    }

    fn handle_normal_key(&mut self, key: KeyEvent) -> bool {
        if key
            .modifiers
            .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT | KeyModifiers::SUPER)
        {
            return false;
        }

        match key.code {
            KeyCode::Char('j') | KeyCode::Down => {
                self.scroll_down(1);
                true
            }
            KeyCode::Char('k') | KeyCode::Up => {
                self.scroll_up(1);
                true
            }
            KeyCode::Char('f') | KeyCode::PageDown | KeyCode::Char(' ') => {
                self.scroll_down(self.state.viewport_rows.max(1));
                true
            }
            KeyCode::Char('b') | KeyCode::PageUp => {
                self.scroll_up(self.state.viewport_rows.max(1));
                true
            }
            KeyCode::Char('g') | KeyCode::Home => {
                self.state.scroll = 0;
                self.clamp_scroll();
                true
            }
            KeyCode::Char('G') | KeyCode::End => {
                self.scroll_to_bottom();
                true
            }
            KeyCode::Char('/') => {
                self.begin_search();
                true
            }
            KeyCode::Char('n') => {
                self.cycle_match(true);
                true
            }
            KeyCode::Char('N') => {
                self.cycle_match(false);
                true
            }
            KeyCode::Char('q') | KeyCode::Esc => {
                self.state.should_quit = true;
                true
            }
            _ => false,
        }
    }

    fn handle_search_key(&mut self, key: KeyEvent) -> bool {
        match key.code {
            KeyCode::Esc => {
                self.cancel_search();
                true
            }
            KeyCode::Enter => {
                self.commit_search();
                true
            }
            KeyCode::Backspace => {
                if self.state.search.draft.pop().is_some() {
                    self.recompute_draft_matches();
                }
                true
            }
            KeyCode::Char(character)
                if !key.modifiers.intersects(
                    KeyModifiers::CONTROL | KeyModifiers::ALT | KeyModifiers::SUPER,
                ) =>
            {
                self.state.search.draft.push(character);
                self.recompute_draft_matches();
                true
            }
            _ => false,
        }
    }

    fn begin_search(&mut self) {
        self.state.search.active = true;
        self.state.search.draft = self.state.search.committed.clone();
        self.recompute_draft_matches();
    }

    fn cancel_search(&mut self) {
        self.state.search.active = false;
        self.state.search.draft = self.state.search.committed.clone();
        let query = self.state.search.committed.clone();
        self.state.search.matches = self.find_matches(&query);
        self.state.search.current = self
            .state
            .search
            .current
            .filter(|&index| index < self.state.search.matches.len());
        self.state.pending_jump = None;
    }

    fn commit_search(&mut self) {
        self.state.search.active = false;
        self.state.search.committed = self.state.search.draft.clone();
        let query = self.state.search.committed.clone();
        self.state.search.matches = self.find_matches(&query);
        self.state.search.current = self.state.search.matches.first().map(|_| 0);
        self.state.pending_jump = self.state.search.matches.first().copied();
    }

    fn recompute_draft_matches(&mut self) {
        let query = self.state.search.draft.clone();
        self.state.search.matches = self.find_matches(&query);
        self.state.search.current = self
            .state
            .search
            .current
            .filter(|&index| index < self.state.search.matches.len());
    }

    fn find_matches(&self, query: &str) -> Vec<usize> {
        if query.is_empty() {
            return Vec::new();
        }

        let query = query.to_lowercase();
        self.document
            .text
            .lines
            .iter()
            .enumerate()
            .filter_map(|(line_index, line)| {
                line.to_string()
                    .to_lowercase()
                    .contains(&query)
                    .then_some(line_index)
            })
            .collect()
    }

    fn cycle_match(&mut self, forward: bool) {
        let matches_len = self.state.search.matches.len();
        if matches_len == 0 {
            return;
        }

        let next = match (self.state.search.current, forward) {
            (Some(current), true) => (current + 1) % matches_len,
            (Some(current), false) => current.checked_sub(1).unwrap_or(matches_len - 1),
            (None, true) => 0,
            (None, false) => matches_len - 1,
        };
        self.state.search.current = Some(next);
        self.state.pending_jump = Some(self.state.search.matches[next]);
    }

    fn scroll_down(&mut self, amount: usize) {
        self.state.scroll = self.state.scroll.saturating_add(amount);
        self.clamp_scroll();
    }

    fn scroll_up(&mut self, amount: usize) {
        self.state.scroll = self.state.scroll.saturating_sub(amount);
        self.clamp_scroll();
    }

    fn scroll_to_bottom(&mut self) {
        self.state.scroll = self.max_scroll();
    }

    fn max_scroll(&self) -> usize {
        self.state
            .content_rows
            .saturating_sub(self.state.viewport_rows)
    }

    /// Update visual row counts after a layout/wrap calculation.
    pub(crate) fn update_layout(&mut self, content_rows: usize, viewport_rows: usize) {
        self.state.content_rows = content_rows;
        self.state.viewport_rows = viewport_rows;
        self.clamp_scroll();
    }

    pub(crate) fn clamp_scroll(&mut self) {
        self.state.scroll = min(self.state.scroll, self.max_scroll());
    }

    /// Apply a visual offset calculated from a pending logical search line.
    /// Returns whether a pending jump was consumed.
    pub(crate) fn apply_pending_jump(&mut self, visual_offset: usize) -> bool {
        if self.state.pending_jump.take().is_some() {
            self.state.scroll = visual_offset.min(self.max_scroll());
            true
        } else {
            false
        }
    }

    pub(crate) fn take_pending_jump(&mut self) -> Option<usize> {
        self.state.pending_jump.take()
    }

    pub(crate) fn set_scroll(&mut self, scroll: usize) {
        self.state.scroll = scroll;
        self.clamp_scroll();
    }

    /// Return the visual offset of a logical rendered-text line at `width`.
    /// This mirrors the wrapping calculation used by the body Paragraph.
    pub(crate) fn visual_offset_for_line(&self, logical_line: usize, width: u16) -> usize {
        if width == 0 || logical_line == 0 {
            return 0;
        }

        let prefix = Text::from(
            self.document
                .text
                .lines
                .iter()
                .take(logical_line)
                .cloned()
                .collect::<Vec<_>>(),
        );
        Paragraph::new(prefix)
            .wrap(Wrap { trim: false })
            .line_count(width)
    }

    pub(crate) fn document_text(&self) -> &Text<'static> {
        &self.document.text
    }

    pub(crate) fn source_label(&self) -> &str {
        &self.source_label
    }

    pub(crate) fn theme(&self) -> &Theme {
        &self.theme
    }

    pub(crate) fn state(&self) -> &ViewState {
        &self.state
    }

    pub(crate) fn state_mut(&mut self) -> &mut ViewState {
        &mut self.state
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::markdown::RenderedDocument;

    fn app_with_text(source: &str) -> App {
        App::new(
            RenderedDocument {
                text: Text::from(source.to_owned()),
            },
            "test.md".to_owned(),
            Theme::default(),
        )
    }

    fn press(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    #[test]
    fn movement_uses_visual_rows_and_page_size() {
        let mut app = app_with_text("one\ntwo\nthree\nfour\nfive\nsix\nseven\neight");
        app.update_layout(8, 3);

        assert!(app.handle_key(press(KeyCode::Char('j'))));
        assert_eq!(app.state.scroll, 1);
        assert!(app.handle_key(press(KeyCode::Char('f'))));
        assert_eq!(app.state.scroll, 4);
        assert!(app.handle_key(press(KeyCode::Char('b'))));
        assert_eq!(app.state.scroll, 1);
        assert!(app.handle_key(press(KeyCode::Char('G'))));
        assert_eq!(app.state.scroll, 5);
        assert!(app.handle_key(press(KeyCode::Char('g'))));
        assert_eq!(app.state.scroll, 0);
    }

    #[test]
    fn movement_clamps_after_resize_and_empty_content() {
        let mut app = app_with_text("one\ntwo\nthree\nfour\nfive");
        app.update_layout(5, 2);
        app.handle_key(press(KeyCode::End));
        assert_eq!(app.state.scroll, 3);

        app.update_layout(5, 8);
        assert_eq!(app.state.scroll, 0);

        let mut empty = app_with_text("");
        empty.update_layout(0, 4);
        empty.handle_key(press(KeyCode::End));
        assert_eq!(empty.state.scroll, 0);
    }

    #[test]
    fn search_is_case_insensitive_and_cycles_matches() {
        let mut app = app_with_text("Alpha\nbeta\nALPHABET");
        app.update_layout(3, 1);
        assert!(app.handle_key(press(KeyCode::Char('/'))));
        assert!(app.state.search.active);
        for character in "alpha".chars() {
            assert!(app.handle_key(press(KeyCode::Char(character))));
        }
        assert_eq!(app.state.search.matches, vec![0, 2]);
        assert!(app.handle_key(press(KeyCode::Enter)));
        assert!(!app.state.search.active);
        assert_eq!(app.state.search.committed, "alpha");
        assert_eq!(app.state.search.current, Some(0));
        assert_eq!(app.state.pending_jump, Some(0));

        assert!(app.handle_key(press(KeyCode::Char('n'))));
        assert_eq!(app.state.search.current, Some(1));
        assert_eq!(app.state.pending_jump, Some(2));
        assert!(app.handle_key(press(KeyCode::Char('N'))));
        assert_eq!(app.state.search.current, Some(0));
        assert_eq!(app.state.pending_jump, Some(0));
    }

    #[test]
    fn escape_cancels_draft_and_restores_committed_query() {
        let mut app = app_with_text("alpha\nbeta");
        app.handle_key(press(KeyCode::Char('/')));
        app.handle_key(press(KeyCode::Char('a')));
        app.handle_key(press(KeyCode::Char('l')));
        app.handle_key(press(KeyCode::Char('p')));
        app.handle_key(press(KeyCode::Char('h')));
        app.handle_key(press(KeyCode::Char('a')));
        app.handle_key(press(KeyCode::Enter));
        assert_eq!(app.state.search.committed, "alpha");

        app.handle_key(press(KeyCode::Char('/')));
        app.handle_key(press(KeyCode::Char('x')));
        app.handle_key(press(KeyCode::Esc));
        assert!(!app.state.search.active);
        assert_eq!(app.state.search.draft, "alpha");
        assert_eq!(app.state.search.committed, "alpha");
        assert_eq!(app.state.search.matches, vec![0]);
    }

    #[test]
    fn wrapped_line_visual_offset_matches_paragraph_layout() {
        let app = app_with_text("12345\nx\nlast");
        assert_eq!(app.visual_offset_for_line(0, 3), 0);
        assert_eq!(app.visual_offset_for_line(1, 3), 2);
        assert_eq!(app.visual_offset_for_line(2, 3), 3);
        assert_eq!(app.visual_offset_for_line(2, 0), 0);
    }
}
