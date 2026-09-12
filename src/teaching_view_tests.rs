use crate::{teaching::Teacher, teaching_terminal::handle_event, teaching_view::Pane, Store};
use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers, MouseEvent, MouseEventKind};
use ratatui::{backend::TestBackend, style::Modifier, Terminal};
use tempfile::TempDir;

fn app(dir: &TempDir) -> Teacher {
    let lessons = serde_json::from_str(include_str!("../data/lessons.json")).unwrap();
    let mut app = Teacher::new(
        Store::open(&dir.path().join("view.db")).unwrap(),
        lessons,
        Some("du"),
    )
    .unwrap();
    app.online = false;
    app.queue.clear();
    app.transcript = (0..12).map(|i| format!("Mercury\nMessage {i}. A helpful explanation with **bold ideas** and *gentle emphasis*.\n\n```bash\ndu -h logs\n```" )).collect();
    app.output = (0..80).map(|i| format!("output line {i}\n")).collect();
    app
}

fn draw(app: &mut Teacher) -> ratatui::buffer::Buffer {
    let mut terminal = Terminal::new(TestBackend::new(110, 36)).unwrap();
    terminal
        .draw(|frame| crate::teaching_ui::draw(frame, app))
        .unwrap();
    terminal.backend().buffer().clone()
}

fn key(app: &mut Teacher, code: KeyCode) {
    handle_event(app, Event::Key(KeyEvent::new(code, KeyModifiers::NONE))).unwrap();
}

#[test]
fn panes_scroll_independently_with_keyboard_and_mouse() {
    let dir = TempDir::new().unwrap();
    let mut app = app(&dir);
    draw(&mut app);
    let bottom = app.view.conversation.top;
    key(&mut app, KeyCode::PageUp);
    assert!(app.view.conversation.top < bottom);
    let conversation = app.view.conversation.top;
    key(&mut app, KeyCode::Tab);
    key(&mut app, KeyCode::PageDown);
    assert!(app.view.output.top > 0);
    assert_eq!(app.view.conversation.top, conversation);
    let output = app.view.output.top;
    let area = app.view.conversation.area;
    handle_event(
        &mut app,
        Event::Mouse(MouseEvent {
            kind: MouseEventKind::ScrollUp,
            column: area.x + 2,
            row: area.y + 2,
            modifiers: KeyModifiers::NONE,
        }),
    )
    .unwrap();
    assert!(app.view.focused == Pane::Conversation);
    assert!(app.view.conversation.top < conversation);
    assert_eq!(app.view.output.top, output);
    key(&mut app, KeyCode::Home);
    assert_eq!(app.view.conversation.top, 0);
    key(&mut app, KeyCode::End);
    assert_eq!(app.view.conversation.top, bottom);
}

#[test]
fn new_replies_preserve_reading_position_until_end_is_requested() {
    let dir = TempDir::new().unwrap();
    let mut app = app(&dir);
    draw(&mut app);
    key(&mut app, KeyCode::PageUp);
    let position = app.view.conversation.top;
    app.say("Mercury", "A new reply arrived while you were reading.");
    draw(&mut app);
    assert_eq!(app.view.conversation.top, position);
    key(&mut app, KeyCode::End);
    app.say("Mercury", "Following the newest reply again.");
    draw(&mut app);
    assert_eq!(app.view.conversation.top, app.view.conversation.limit());
}

#[test]
fn markdown_styles_code_and_message_dividers_are_rendered() {
    let dir = TempDir::new().unwrap();
    let mut app = app(&dir);
    app.transcript = vec!["Mercury\n**Bold** and *italic* with <amber>attention</amber>.\n\n```bash\necho \"hello\"\n```".into(), "You\ndu -h logs".into()];
    let buffer = draw(&mut app);
    let text: String = buffer.content.iter().map(|cell| cell.symbol()).collect();
    assert!(text.contains("── MERCURY"));
    assert!(text.contains("── YOU"));
    assert!(!text.contains("**Bold**"));
    assert!(!text.contains("<amber>"));
    assert!(buffer
        .content
        .iter()
        .any(|cell| cell.modifier.contains(Modifier::ITALIC)));
    assert!(buffer
        .content
        .iter()
        .any(|cell| cell.fg == crate::teaching_palette::AMBER));
    assert!(buffer
        .content
        .iter()
        .any(|cell| cell.bg == crate::teaching_palette::CODE));
}

#[test]
fn highlighting_and_emphasis_survive_wrapping_without_green_or_control_codes() {
    use crate::{teaching_markdown, teaching_palette as palette, teaching_wrap};
    let rich = teaching_markdown::render("**Important words remain readable** and *italic*. <amber>Check this</amber>.\n\n```json\n{\"message\": \"hello\", \"count\": 42}\n```\n\n```yaml\napiVersion: v1\nkind: Pod\n```\n\n```python\ndef hello():\n    return \"world\"\n```");
    let lines: Vec<_> = rich
        .into_iter()
        .flat_map(|rich| teaching_wrap::wrap(rich.line, 20, !rich.code))
        .collect();
    assert!(lines.iter().all(|line| line.width() <= 20));
    let spans: Vec<_> = lines.iter().flat_map(|line| &line.spans).collect();
    assert!(spans
        .iter()
        .any(|span| span.style.add_modifier.contains(Modifier::BOLD)));
    assert!(spans
        .iter()
        .any(|span| span.style.add_modifier.contains(Modifier::ITALIC)));
    assert!(spans
        .iter()
        .any(|span| span.style.fg == Some(palette::PINK)));
    for span in spans {
        assert!(!span.content.contains('\u{1b}'));
        if let Some(ratatui::style::Color::Rgb(r, g, b)) = span.style.fg {
            assert!(!(g > r && g > b));
        }
    }
}

#[test]
fn bot_animates_and_screen_can_be_inspected_without_a_model_call() {
    assert_ne!(
        crate::teaching_avatar::face(0, true),
        crate::teaching_avatar::face(1, true)
    );
    let dir = TempDir::new().unwrap();
    let mut app = app(&dir);
    app.transcript = vec![
        "Mercury\n### Readable sizes\n`du -h` measures the same disk usage, but **formats sizes for people**. The flag changes *display*, not measurement.\n\n<amber>Remember:</amber> plain `du` uses KiB. `4096` becomes `4.0M`.\n\n```bash\ndu -h logs\n```".into(),
        "You\ndu -h logs".into(),
        "Mercury\nThe logs total **includes its archive**. <cyan>Do not add overlapping directory totals.</cyan>\n\nTry `/next` when you're ready to add one more option.".into(),
    ];
    app.output = "$ du -h logs\n32K  logs/archive\n44K  logs\n\n[exit 0]".into();
    app.progress.step = 1;
    let mut terminal = Terminal::new(TestBackend::new(110, 42)).unwrap();
    terminal
        .draw(|frame| crate::teaching_ui::draw(frame, &mut app))
        .unwrap();
    let buffer = terminal.backend().buffer();
    if let Ok(path) = std::env::var("SRE_UI_SNAPSHOT") {
        let cells: Vec<_> = buffer.content.iter().map(|cell| serde_json::json!({"text":cell.symbol(), "fg":format!("{:?}", cell.fg), "bg":format!("{:?}", cell.bg), "bold":cell.modifier.contains(Modifier::BOLD), "italic":cell.modifier.contains(Modifier::ITALIC)})).collect();
        std::fs::write(
            path,
            serde_json::json!({"width":110,"height":42,"cells":cells}).to_string(),
        )
        .unwrap();
    }
}
