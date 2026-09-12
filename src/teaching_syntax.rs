use crate::teaching_palette as palette;
use ratatui::{
    style::{Color, Modifier, Style},
    text::{Line, Span},
};
use std::sync::LazyLock;
use syntect::{
    easy::HighlightLines,
    highlighting::{Color as SyntaxColor, FontStyle, StyleModifier, Theme, ThemeItem},
    parsing::SyntaxSet,
    util::LinesWithEndings,
};

static SYNTAXES: LazyLock<SyntaxSet> = LazyLock::new(SyntaxSet::load_defaults_newlines);
static THEME: LazyLock<Theme> = LazyLock::new(theme);

fn syntax_color(color: Color) -> SyntaxColor {
    let Color::Rgb(r, g, b) = color else {
        return SyntaxColor::WHITE;
    };
    SyntaxColor { r, g, b, a: 255 }
}

fn theme() -> Theme {
    let mut theme = Theme::default();
    theme.settings.foreground = Some(syntax_color(palette::TEXT));
    theme.settings.background = Some(syntax_color(palette::CODE));
    theme.scopes = [
        ("comment", palette::MUTED, FontStyle::ITALIC),
        ("string", palette::AMBER, FontStyle::empty()),
        ("constant", palette::PINK, FontStyle::empty()),
        ("keyword, storage", palette::VIOLET, FontStyle::BOLD),
        ("entity.name", palette::BLUE, FontStyle::BOLD),
        ("variable", palette::CYAN, FontStyle::empty()),
        ("support", palette::PINK, FontStyle::empty()),
        ("punctuation", palette::MUTED, FontStyle::empty()),
    ]
    .into_iter()
    .map(|(scope, color, font)| ThemeItem {
        scope: scope.parse().expect("built-in syntax selectors are valid"),
        style: StyleModifier {
            foreground: Some(syntax_color(color)),
            font_style: Some(font),
            ..StyleModifier::default()
        },
    })
    .collect();
    theme
}

pub fn code(text: &str, language: &str) -> Vec<Line<'static>> {
    let language = match language.split_whitespace().next().unwrap_or("sh") {
        "bash" | "shell" | "console" | "terminal" | "zsh" | "" => "sh",
        "yml" => "yaml",
        other => other,
    };
    let syntax = SYNTAXES
        .find_syntax_by_token(language)
        .or_else(|| SYNTAXES.find_syntax_by_first_line(text.lines().next().unwrap_or("")))
        .or_else(|| SYNTAXES.find_syntax_by_extension("sh"))
        .unwrap_or_else(|| SYNTAXES.find_syntax_plain_text());
    let mut highlighter = HighlightLines::new(syntax, &THEME);
    LinesWithEndings::from(text)
        .map(|line| match highlighter.highlight_line(line, &SYNTAXES) {
            Ok(tokens) => Line::from(
                tokens
                    .into_iter()
                    .map(|(style, token)| {
                        let mut rendered = Style::default()
                            .fg(Color::Rgb(
                                style.foreground.r,
                                style.foreground.g,
                                style.foreground.b,
                            ))
                            .bg(palette::CODE);
                        if style.font_style.contains(FontStyle::BOLD) {
                            rendered = rendered.add_modifier(Modifier::BOLD);
                        }
                        if style.font_style.contains(FontStyle::ITALIC) {
                            rendered = rendered.add_modifier(Modifier::ITALIC);
                        }
                        Span::styled(token.trim_end_matches(['\r', '\n']).to_owned(), rendered)
                    })
                    .collect::<Vec<_>>(),
            ),
            Err(_) => Line::styled(
                line.trim_end().to_owned(),
                Style::default().fg(palette::AMBER).bg(palette::CODE),
            ),
        })
        .collect()
}

pub fn inline(text: &str) -> Vec<Span<'static>> {
    code(text, "sh")
        .into_iter()
        .flat_map(|line| line.spans)
        .collect()
}

pub fn is_command(text: &str) -> bool {
    matches!(
        text.split_whitespace().next(),
        Some(
            "du" | "grep"
                | "jq"
                | "chmod"
                | "ps"
                | "kubectl"
                | "docker"
                | "curl"
                | "dig"
                | "ss"
                | "ls"
                | "cat"
                | "df"
                | "find"
                | "aws"
                | "gcloud"
                | "sudo"
                | "tail"
                | "head"
                | "systemctl"
                | "journalctl"
                | "echo"
                | "printf"
        )
    )
}
