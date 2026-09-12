use ratatui::style::Color;

pub const BACKGROUND: Color = Color::Rgb(14, 17, 29);
pub const PANEL: Color = Color::Rgb(21, 25, 40);
pub const CODE: Color = Color::Rgb(29, 33, 51);
pub const TEXT: Color = Color::Rgb(226, 229, 243);
pub const MUTED: Color = Color::Rgb(139, 147, 177);
pub const VIOLET: Color = Color::Rgb(189, 155, 255);
pub const BLUE: Color = Color::Rgb(124, 174, 255);
pub const CYAN: Color = Color::Rgb(112, 212, 245);
pub const AMBER: Color = Color::Rgb(255, 201, 112);
pub const PINK: Color = Color::Rgb(246, 154, 201);
pub const RED: Color = Color::Rgb(255, 132, 147);

pub fn named(name: &str) -> Option<Color> {
    match name {
        "violet" => Some(VIOLET),
        "blue" => Some(BLUE),
        "cyan" => Some(CYAN),
        "amber" => Some(AMBER),
        "pink" => Some(PINK),
        "red" => Some(RED),
        _ => None,
    }
}
