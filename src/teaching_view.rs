use ratatui::layout::{Position, Rect};

#[derive(Clone, Copy, Default, PartialEq)]
pub enum Pane {
    #[default]
    Conversation,
    Output,
}

#[derive(Default)]
pub struct Viewport {
    pub area: Rect,
    pub top: usize,
    pub lines: usize,
    pub follow: bool,
}

impl Viewport {
    pub fn update(&mut self, lines: usize, area: Rect) {
        self.lines = lines;
        self.area = area;
        self.top = if self.follow {
            self.limit()
        } else {
            self.top.min(self.limit())
        };
    }

    pub fn limit(&self) -> usize {
        self.lines
            .saturating_sub(self.area.height.saturating_sub(2) as usize)
    }

    pub fn scroll(&mut self, rows: isize) {
        self.top = self.top.saturating_add_signed(rows).min(self.limit());
        self.follow = self.top == self.limit();
    }

    pub fn home(&mut self) {
        self.top = 0;
        self.follow = false;
    }

    pub fn end(&mut self) {
        self.top = self.limit();
        self.follow = true;
    }
}

pub struct ReadingView {
    pub conversation: Viewport,
    pub output: Viewport,
    pub focused: Pane,
    pub messages: crate::teaching_messages::MessageCache,
    pub started: std::time::Instant,
}

impl Default for ReadingView {
    fn default() -> Self {
        Self {
            conversation: Viewport {
                follow: true,
                ..Viewport::default()
            },
            output: Viewport::default(),
            focused: Pane::Conversation,
            messages: crate::teaching_messages::MessageCache::default(),
            started: std::time::Instant::now(),
        }
    }
}

impl ReadingView {
    pub fn selected(&mut self) -> &mut Viewport {
        match self.focused {
            Pane::Conversation => &mut self.conversation,
            Pane::Output => &mut self.output,
        }
    }

    pub fn toggle(&mut self) {
        self.focused = match self.focused {
            Pane::Conversation if !self.output.area.is_empty() => Pane::Output,
            _ => Pane::Conversation,
        };
    }

    pub fn focus_at(&mut self, column: u16, row: u16) -> bool {
        let point = Position::new(column, row);
        if self.conversation.area.contains(point) {
            self.focused = Pane::Conversation;
            return true;
        }
        if self.output.area.contains(point) {
            self.focused = Pane::Output;
            return true;
        }
        false
    }
}
