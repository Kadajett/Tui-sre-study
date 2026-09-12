use crate::{teaching_palette as palette, teaching_syntax as syntax};
use pulldown_cmark::{CodeBlockKind, Event, Options, Parser, Tag, TagEnd};
use ratatui::{
    style::{Modifier, Style},
    text::{Line, Span},
};

pub struct RichLine {
    pub line: Line<'static>,
    pub code: bool,
}

#[derive(Default)]
struct Markdown {
    lines: Vec<RichLine>,
    current: Vec<Span<'static>>,
    styles: Vec<Style>,
    block: Option<(String, String)>,
}

pub fn render(text: &str) -> Vec<RichLine> {
    if syntax::is_command(text) && !text.contains('\n') {
        return syntax::code(text, "sh")
            .into_iter()
            .map(|line| RichLine { line, code: true })
            .collect();
    }
    let prepared = text
        .lines()
        .map(|line| {
            if let Some(command) = line.strip_prefix("Type: ") {
                return format!("Type: `{command}`");
            }
            line.to_owned()
        })
        .collect::<Vec<_>>()
        .join("\n");
    let mut markdown = Markdown::default();
    for event in Parser::new_ext(&prepared, Options::ENABLE_STRIKETHROUGH) {
        markdown.event(event);
    }
    markdown.flush();
    markdown.lines
}

impl Markdown {
    fn style(&self) -> Style {
        self.styles.last().copied().unwrap_or_default()
    }

    fn push_style(&mut self, style: Style) {
        self.styles.push(self.style().patch(style));
    }

    fn text(&mut self, text: &str) {
        self.current
            .push(Span::styled(text.to_owned(), self.style()));
    }

    fn flush(&mut self) {
        if !self.current.is_empty() {
            self.lines.push(RichLine {
                line: Line::from(std::mem::take(&mut self.current)),
                code: false,
            });
        }
    }

    fn blank(&mut self) {
        self.flush();
        self.lines.push(RichLine {
            line: Line::default(),
            code: false,
        });
    }

    fn event(&mut self, event: Event<'_>) {
        match event {
            Event::Start(tag) => self.start(tag),
            Event::End(tag) => self.end(tag),
            Event::Text(text) => {
                if let Some((_, code)) = &mut self.block {
                    code.push_str(&text);
                } else {
                    self.text(&text);
                }
            }
            Event::Code(text) => self.current.extend(
                syntax::inline(&text)
                    .into_iter()
                    .map(|span| {
                        let style = self.style().patch(span.style);
                        span.style(style)
                    })
                    .collect::<Vec<_>>(),
            ),
            Event::SoftBreak => self.text(" "),
            Event::HardBreak => self.flush(),
            Event::Rule => {
                self.blank();
                self.text("────────────────────────");
                self.blank();
            }
            Event::Html(html) | Event::InlineHtml(html) => self.html(&html),
            _ => {}
        }
    }

    fn start(&mut self, tag: Tag<'_>) {
        match tag {
            Tag::Strong => self.push_style(Style::default().add_modifier(Modifier::BOLD)),
            Tag::Emphasis => self.push_style(Style::default().add_modifier(Modifier::ITALIC)),
            Tag::Strikethrough => {
                self.push_style(Style::default().add_modifier(Modifier::CROSSED_OUT))
            }
            Tag::Heading { .. } => {
                self.flush();
                self.push_style(
                    Style::default()
                        .fg(palette::VIOLET)
                        .add_modifier(Modifier::BOLD),
                );
            }
            Tag::Link { .. } => self.push_style(
                Style::default()
                    .fg(palette::BLUE)
                    .add_modifier(Modifier::UNDERLINED),
            ),
            Tag::Item => {
                self.flush();
                self.text("• ");
            }
            Tag::CodeBlock(kind) => {
                self.blank();
                let language = match kind {
                    CodeBlockKind::Fenced(language) => language.to_string(),
                    _ => "sh".into(),
                };
                self.lines.push(RichLine {
                    line: Line::styled(
                        format!(
                            " {} ",
                            if language.is_empty() {
                                "shell"
                            } else {
                                &language
                            }
                        ),
                        Style::default().fg(palette::MUTED).bg(palette::CODE),
                    ),
                    code: true,
                });
                self.block = Some((language, String::new()));
            }
            _ => {}
        }
    }

    fn end(&mut self, tag: TagEnd) {
        match tag {
            TagEnd::Strong | TagEnd::Emphasis | TagEnd::Strikethrough | TagEnd::Link => {
                self.styles.pop();
            }
            TagEnd::Heading(_) => {
                self.styles.pop();
                self.blank();
            }
            TagEnd::Paragraph => self.blank(),
            TagEnd::Item => self.flush(),
            TagEnd::CodeBlock => {
                if let Some((language, code)) = self.block.take() {
                    self.lines.extend(
                        syntax::code(&code, &language)
                            .into_iter()
                            .map(|line| RichLine { line, code: true }),
                    );
                }
                self.blank();
            }
            _ => {}
        }
    }

    fn html(&mut self, html: &str) {
        let tag = html.trim().trim_start_matches('<').trim_end_matches('>');
        if let Some(color) = palette::named(tag) {
            self.push_style(Style::default().fg(color));
            return;
        }
        if tag
            .strip_prefix('/')
            .is_some_and(|name| palette::named(name).is_some())
        {
            self.styles.pop();
            return;
        }
        match tag {
            "u" => self.push_style(Style::default().add_modifier(Modifier::UNDERLINED)),
            "/u" => {
                self.styles.pop();
            }
            _ => self.text(html),
        }
    }
}
