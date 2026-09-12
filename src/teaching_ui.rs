use crate::{
    teaching::Teacher,
    teaching_palette as palette,
    teaching_view::{Pane, Viewport},
};
use palette::{CYAN, MUTED};
use ratatui::{
    layout::{Constraint, Layout, Margin, Rect},
    style::Style,
    text::Line,
    widgets::{
        Block, Borders, Padding, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState,
    },
    Frame,
};

pub fn draw(frame: &mut Frame, app: &mut Teacher) {
    if frame.area().width < 55 || frame.area().height < 18 {
        app.view.conversation.area = Rect::default();
        app.view.output.area = Rect::default();
        frame.render_widget(
            Paragraph::new("Enlarge the terminal to 55 columns and 18 rows. Esc saves and exits."),
            frame.area(),
        );
        return;
    }
    frame.render_widget(
        Block::default().style(Style::default().fg(palette::TEXT).bg(palette::BACKGROUND)),
        frame.area(),
    );
    let areas = Layout::vertical([
        Constraint::Length(6),
        Constraint::Min(8),
        Constraint::Length(3),
        Constraint::Length(2),
    ])
    .split(frame.area());
    crate::teaching_avatar::draw(frame, app, areas[0]);
    if app.output.is_empty() {
        app.view.output.area = Rect::default();
        app.view.focused = Pane::Conversation;
        dialogue(frame, app, areas[1]);
    } else {
        let panes = Layout::horizontal([Constraint::Percentage(40), Constraint::Percentage(60)])
            .split(areas[1]);
        output(frame, app, panes[0]);
        dialogue(frame, app, panes[1]);
    }
    input(frame, &app.input, areas[2]);
    frame.render_widget(Paragraph::new("Wheel scroll · Tab switch pane · PgUp/PgDn · ↑/↓ · Home/End\nEnter send/run · /practice · /hint · /next · /topics · Esc quit").style(Style::default().fg(MUTED)), areas[3]);
}

fn input(frame: &mut Frame, text: &str, area: Rect) {
    let chars: Vec<_> = text.chars().collect();
    let input: String = chars
        .iter()
        .skip(
            chars
                .len()
                .saturating_sub(area.width.saturating_sub(4) as usize),
        )
        .collect();
    frame.render_widget(
        Paragraph::new(if crate::teaching_syntax::is_command(text) {
            Line::from(crate::teaching_syntax::inline(&input))
        } else {
            Line::raw(input.clone())
        })
        .block(
            Block::default()
                .title(" Talk to Mercury, or type a lab command ")
                .borders(Borders::ALL),
        ),
        area,
    );
    frame.set_cursor_position((area.x + 1 + input.chars().count() as u16, area.y + 1));
}

fn dialogue(frame: &mut Frame, app: &mut Teacher, area: Rect) {
    let lines = app
        .view
        .messages
        .lines(&app.transcript, area.width.saturating_sub(4));
    app.view.conversation.update(lines.len(), area);
    let title = if app.coach.busy() {
        "Learning conversation · thinking…"
    } else {
        "Learning conversation"
    };
    let block = pane_block(title, app.view.focused == Pane::Conversation);
    pane(frame, &app.view.conversation, lines, block);
}

fn output(frame: &mut Frame, app: &mut Teacher, area: Rect) {
    let lines = crate::teaching_syntax::code(&app.output, "sh")
        .into_iter()
        .flat_map(|line| crate::teaching_wrap::wrap(line, area.width.saturating_sub(4), false))
        .collect::<Vec<_>>();
    app.view.output.update(lines.len(), area);
    let block = pane_block("Actual command output", app.view.focused == Pane::Output);
    pane(frame, &app.view.output, lines, block);
}

fn pane_block(title: &str, focused: bool) -> Block<'static> {
    let marker = if focused { "▶ " } else { "" };
    Block::default()
        .title(format!(" {marker}{title} "))
        .borders(Borders::ALL)
        .border_type(ratatui::widgets::BorderType::Rounded)
        .padding(Padding::horizontal(1))
        .style(Style::default().bg(palette::PANEL))
        .border_style(Style::default().fg(if focused { CYAN } else { MUTED }))
}

fn pane(frame: &mut Frame, view: &Viewport, lines: Vec<Line<'static>>, block: Block<'static>) {
    let visible = view.area.height.saturating_sub(2) as usize;
    let status = if view.lines > visible {
        format!(
            " {}–{} / {} · End: latest ",
            view.top + 1,
            (view.top + visible).min(view.lines),
            view.lines
        )
    } else {
        String::new()
    };
    frame.render_widget(
        Paragraph::new(lines)
            .scroll((view.top.min(u16::MAX as usize) as u16, 0))
            .block(block.title_bottom(status)),
        view.area,
    );
    if view.limit() == 0 {
        return;
    }
    let mut state = ScrollbarState::new(view.limit() + 1)
        .position(view.top)
        .viewport_content_length(visible);
    frame.render_stateful_widget(
        Scrollbar::new(ScrollbarOrientation::VerticalRight)
            .begin_symbol(Some("↑"))
            .end_symbol(Some("↓")),
        view.area.inner(Margin {
            vertical: 1,
            horizontal: 0,
        }),
        &mut state,
    );
}
