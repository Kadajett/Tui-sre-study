use crate::{guided::Guided, Store};
use anyhow::Result;
use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use std::time::Duration;

impl Guided {
    fn key(&mut self, key: crossterm::event::KeyEvent) -> Result<()> {
        match key.code {
            KeyCode::Esc => self.quit = true,
            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => self.quit = true,
            KeyCode::Char('n') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.submit("/next")?
            }
            KeyCode::Enter => {
                let text = std::mem::take(&mut self.input);
                self.submit(&text)?;
            }
            KeyCode::Backspace => {
                self.input.pop();
            }
            KeyCode::Up => self.input = self.previous_command().to_owned(),
            KeyCode::PageUp => self.scroll = self.scroll.saturating_add(8),
            KeyCode::PageDown => self.scroll = self.scroll.saturating_sub(8),
            KeyCode::Char(c) if !c.is_control() && self.input.chars().count() < 1000 => {
                self.input.push(c)
            }
            _ => {}
        }
        Ok(())
    }
}

pub fn run(store: Store) -> Result<()> {
    let mut app = Guided::new(store)?;
    let mut terminal = ratatui::init();
    let result = event_loop(&mut terminal, &mut app);
    ratatui::restore();
    result?;
    println!("Session saved. Return with docker compose run --rm trainer.");
    Ok(())
}

fn event_loop(terminal: &mut ratatui::DefaultTerminal, app: &mut Guided) -> Result<()> {
    while !app.quit {
        app.poll_coach();
        terminal.draw(|frame| crate::guided_ui::draw(frame, app))?;
        let Some(key) = poll_key()? else {
            continue;
        };
        if let Err(error) = app.key(key) {
            app.say("Could not save or continue", &error.to_string());
        }
    }
    Ok(())
}

fn poll_key() -> Result<Option<crossterm::event::KeyEvent>> {
    if !event::poll(Duration::from_millis(100))? {
        return Ok(None);
    }
    let Event::Key(key) = event::read()? else {
        return Ok(None);
    };
    Ok((key.kind == KeyEventKind::Press).then_some(key))
}
