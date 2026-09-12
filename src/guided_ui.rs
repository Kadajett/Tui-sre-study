use ratatui::{
    layout::{Constraint, Layout},
    style::{Color, Style},
    widgets::{Block, Borders, Paragraph},
    Frame,
};

use crate::{
    du_course,
    guided::{Guided, Mode},
    ACCENT, CYAN, MUTED,
};

pub fn draw(frame: &mut Frame, app: &Guided) {
    if frame.area().height < 20 || frame.area().width < 55 {
        frame.render_widget(Paragraph::new("Please enlarge your terminal to at least 55 columns and 20 rows. Esc still saves and exits."), frame.area());
        return;
    }
    let areas = Layout::vertical([
        Constraint::Length(3),
        Constraint::Length(8),
        Constraint::Min(4),
        Constraint::Length(3),
        Constraint::Length(2),
    ])
    .split(frame.area());
    let status = match app.mode {
        Mode::Learning => format!(
            "LEARN DU · step {}/6 · {}",
            app.progress.step + 1,
            if app.progress.ready {
                "complete — /next when ready"
            } else {
                "type it and see"
            }
        ),
        Mode::Review => "DU REVIEW · demonstrate it from memory".into(),
        Mode::Practice => format!(
            "DU PRACTICE · next review {}",
            app.next_review.as_deref().unwrap_or("not yet scheduled")
        ),
    };
    frame.render_widget(
        Paragraph::new(status)
            .style(Style::default().fg(ACCENT))
            .block(Block::default().title(" SWE → SRE ").borders(Borders::ALL)),
        areas[0],
    );
    let step = du_course::step(app.progress.step);
    let task = if app.mode == Mode::Practice {
        "Explore: compare du -h ., du -sh logs, du -hd1 . and du -ah logs. Ask why with /ask QUESTION. /topics opens the SRE roadmap. This is free practice; it does not change your review schedule."
    } else {
        step.introduction
    };
    frame.render_widget(
        Paragraph::new(wrap(task, areas[1].width.saturating_sub(2)).join("\n")).block(
            Block::default()
                .title(format!(" {} ", step.title))
                .borders(Borders::ALL)
                .border_style(Style::default().fg(CYAN)),
        ),
        areas[1],
    );
    draw_practice(frame, app, areas[2]);
    draw_input(frame, app, areas[3]);
    frame.render_widget(Paragraph::new("Enter run/send · /next · /hint · /ask QUESTION · /topics · Up reuse command · Esc quit\nReal files in a disposable container lab. Mercury can consult your DevDocs and SearXNG.").style(Style::default().fg(MUTED)), areas[4]);
}

fn draw_practice(frame: &mut Frame, app: &Guided, area: ratatui::layout::Rect) {
    let panes =
        Layout::horizontal([Constraint::Percentage(42), Constraint::Percentage(58)]).split(area);
    let output = if app.last_output().is_empty() {
        "Run a command below. Its actual output will stay here while you read the coaching."
    } else {
        app.last_output()
    };
    let lines = wrap(output, panes[0].width.saturating_sub(2));
    let overflow = lines
        .len()
        .saturating_sub(panes[0].height.saturating_sub(2).into())
        .min(u16::MAX as usize) as u16;
    frame.render_widget(
        Paragraph::new(lines.join("\n"))
            .scroll((app.scroll.min(overflow), 0))
            .block(
                Block::default()
                    .title(" Actual output ")
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(ACCENT)),
            ),
        panes[0],
    );
    draw_log(frame, app, panes[1]);
}

fn draw_log(frame: &mut Frame, app: &Guided, area: ratatui::layout::Rect) {
    let transcript = wrap(&app.transcript.join("\n\n"), area.width.saturating_sub(2));
    let bottom = transcript
        .len()
        .saturating_sub(area.height.saturating_sub(2).into())
        .min(u16::MAX as usize) as u16;
    let title = if app.coach.busy() {
        " Mercury is thinking… "
    } else {
        " Coach · PgUp/PgDn scroll "
    };
    frame.render_widget(
        Paragraph::new(transcript.join("\n"))
            .scroll((bottom.saturating_sub(app.scroll), 0))
            .block(Block::default().title(title).borders(Borders::ALL)),
        area,
    );
}

fn draw_input(frame: &mut Frame, app: &Guided, area: ratatui::layout::Rect) {
    let width = area.width.saturating_sub(4) as usize;
    let chars: Vec<char> = app.input.chars().collect();
    let visible: String = chars
        .iter()
        .skip(chars.len().saturating_sub(width))
        .collect();
    frame.render_widget(
        Paragraph::new(visible.as_str())
            .style(Style::default().fg(Color::White))
            .block(
                Block::default()
                    .title(" Type a command, or /ask a question ")
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(ACCENT)),
            ),
        area,
    );
    frame.set_cursor_position((area.x + 1 + visible.chars().count() as u16, area.y + 1));
}

pub(super) fn wrap(text: &str, width: u16) -> Vec<String> {
    text.lines()
        .flat_map(|line| wrap_line(line, width.max(1) as usize))
        .collect()
}

fn wrap_line(line: &str, width: usize) -> Vec<String> {
    let expanded = line.replace('\t', "    ");
    let chars: Vec<char> = expanded
        .chars()
        .filter(|character| !character.is_control())
        .collect();
    if chars.is_empty() {
        return vec![String::new()];
    }
    chars
        .chunks(width)
        .map(|chunk| chunk.iter().collect())
        .collect()
}
