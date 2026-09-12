use crate::{teaching_markdown, teaching_palette as palette, teaching_wrap};
use ratatui::{
    style::{Modifier, Style},
    text::{Line, Span},
};

#[derive(Default)]
pub struct MessageCache {
    messages: Vec<String>,
    width: u16,
    lines: Vec<Line<'static>>,
}

impl MessageCache {
    pub fn lines(&mut self, messages: &[String], width: u16) -> Vec<Line<'static>> {
        if self.messages != messages || self.width != width {
            self.messages = messages.to_vec();
            self.width = width;
            self.lines = messages
                .iter()
                .flat_map(|message| message_lines(message, width))
                .collect();
        }
        self.lines.clone()
    }
}

fn message_lines(message: &str, width: u16) -> Vec<Line<'static>> {
    let (speaker, body) = message.split_once('\n').unwrap_or(("Coach", message));
    let color = match speaker {
        "You" => palette::CYAN,
        "Mercury" => palette::VIOLET,
        _ => palette::AMBER,
    };
    let heading = format!("── {} ", speaker.to_uppercase());
    let rule = "─".repeat((width as usize).saturating_sub(Span::raw(&heading).width()));
    let mut lines = vec![
        Line::styled(
            format!("{heading}{rule}"),
            Style::default().fg(color).add_modifier(Modifier::BOLD),
        ),
        Line::default(),
    ];
    for rich in teaching_markdown::render(body) {
        let wrapped = teaching_wrap::wrap(rich.line, width.saturating_sub(2), !rich.code);
        lines.extend(wrapped.into_iter().map(|mut line| {
            line.spans
                .insert(0, Span::styled("│ ", Style::default().fg(color)));
            line
        }));
    }
    lines.push(Line::default());
    lines
}
