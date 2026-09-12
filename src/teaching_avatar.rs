use crate::{teaching::Teacher, teaching_curriculum, teaching_palette as palette};
use ratatui::{
    layout::{Constraint, Layout, Rect},
    style::{Modifier, Style},
    text::Line,
    widgets::{Block, BorderType, Borders, Paragraph},
    Frame,
};

pub fn face(tick: u128, thinking: bool) -> &'static str {
    if thinking {
        return match tick % 4 {
            0 => "  .------.\n /| .  . |\\\n  |  ..  |\n  '------'",
            1 => "  .------.\n /| o  . |\\\n  |  ..  |\n  '------'",
            2 => "  .------.\n /| o  o |\\\n  |  ..  |\n  '------'",
            _ => "  .------.\n /| .  o |\\\n  |  ..  |\n  '------'",
        };
    }
    if (tick + 1).is_multiple_of(16) {
        "  .------.\n /| -  - |\\\n  |  __  |\n  '------'"
    } else {
        "  .------.\n /| o  o |\\\n  |  \\_/ |\n  '------'"
    }
}

pub fn draw(frame: &mut Frame, app: &Teacher, area: Rect) {
    let panes = Layout::horizontal([Constraint::Min(25), Constraint::Length(21)]).split(area);
    let step = format!(
        "Level {} · step {}/{}",
        teaching_curriculum::level(app.lesson()),
        app.progress.step + 1,
        teaching_curriculum::step_count(app.lesson())
    );
    let lines = vec![
        Line::styled(
            teaching_curriculum::title(app.lesson()),
            Style::default()
                .fg(palette::VIOLET)
                .add_modifier(Modifier::BOLD),
        ),
        Line::styled(step, Style::default().fg(palette::BLUE)),
        Line::styled(
            if app.progress.ready {
                "Step practiced · /next continues"
            } else {
                "Ask freely · /practice together"
            },
            Style::default().fg(palette::AMBER),
        ),
        Line::styled(
            crate::teaching_levels::name(teaching_curriculum::level(app.lesson())),
            Style::default().fg(palette::MUTED),
        ),
    ];
    frame.render_widget(Paragraph::new(lines).block(block(" SWE → SRE ")), panes[0]);
    let label = if app.coach.busy() {
        " Mercury · thinking "
    } else {
        " Mercury · ready "
    };
    frame.render_widget(
        Paragraph::new(face(
            app.view.started.elapsed().as_millis() / 350,
            app.coach.busy(),
        ))
        .centered()
        .style(Style::default().fg(palette::PINK))
        .block(block(label)),
        panes[1],
    );
}

fn block(title: &str) -> Block<'_> {
    Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(palette::VIOLET))
        .style(Style::default().bg(palette::PANEL))
}
