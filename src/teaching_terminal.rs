use crate::{teaching::Teacher, Lesson, Store};
use anyhow::Result;
use crossterm::{
    event::{
        self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind, KeyModifiers,
        MouseButton, MouseEventKind,
    },
    execute,
};

pub fn run(store: Store, lessons: Vec<Lesson>, deck: Option<&str>) -> Result<()> {
    let mut app = Teacher::new(store, lessons, deck)?;
    let mut terminal = ratatui::init();
    let result = execute!(std::io::stdout(), EnableMouseCapture)
        .map_err(anyhow::Error::from)
        .and_then(|()| run_loop(&mut terminal, &mut app));
    let cleanup = execute!(std::io::stdout(), DisableMouseCapture);
    ratatui::restore();
    result?;
    cleanup?;
    println!("Learning progress saved. Come back and we'll continue the conversation.");
    Ok(())
}

fn run_loop(terminal: &mut ratatui::DefaultTerminal, app: &mut Teacher) -> Result<()> {
    while !app.quit {
        if let Err(error) = app.tick() {
            app.say("Could not update the lesson", &error.to_string());
        }
        terminal.draw(|frame| crate::teaching_ui::draw(frame, app))?;
        if !event::poll(std::time::Duration::from_millis(100))? {
            continue;
        }
        if let Err(error) = handle_event(app, event::read()?) {
            app.say("Coach", &error.to_string());
        }
    }
    Ok(())
}

pub(super) fn handle_event(app: &mut Teacher, event: Event) -> Result<()> {
    match event {
        Event::Key(key) if key.kind != KeyEventKind::Release => input(app, key)?,
        Event::Mouse(mouse) => mouse_input(app, mouse),
        _ => {}
    }
    Ok(())
}

fn mouse_input(app: &mut Teacher, mouse: event::MouseEvent) {
    if !matches!(
        mouse.kind,
        MouseEventKind::ScrollUp
            | MouseEventKind::ScrollDown
            | MouseEventKind::Down(MouseButton::Left)
    ) {
        return;
    }
    if !app.view.focus_at(mouse.column, mouse.row) {
        return;
    }
    match mouse.kind {
        MouseEventKind::ScrollUp => app.view.selected().scroll(-3),
        MouseEventKind::ScrollDown => app.view.selected().scroll(3),
        _ => {}
    }
}

fn input(app: &mut Teacher, key: event::KeyEvent) -> Result<()> {
    if scroll_key(app, key.code) {
        return Ok(());
    }
    match key.code {
        KeyCode::Esc => app.quit = true,
        KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => app.quit = true,
        KeyCode::Enter => {
            let text = std::mem::take(&mut app.input);
            app.submit(&text)?;
        }
        KeyCode::Backspace => {
            app.input.pop();
        }
        KeyCode::Char(c) if !c.is_control() && app.input.chars().count() < 1500 => {
            app.input.push(c)
        }
        _ => {}
    }
    Ok(())
}

fn scroll_key(app: &mut Teacher, key: KeyCode) -> bool {
    let page = app.view.selected().area.height.saturating_sub(3).max(1) as isize;
    match key {
        KeyCode::Tab | KeyCode::BackTab => app.view.toggle(),
        KeyCode::PageUp => app.view.selected().scroll(-page),
        KeyCode::PageDown => app.view.selected().scroll(page),
        KeyCode::Up => app.view.selected().scroll(-1),
        KeyCode::Down => app.view.selected().scroll(1),
        KeyCode::Home => app.view.selected().home(),
        KeyCode::End => app.view.selected().end(),
        _ => return false,
    }
    true
}
