use ratatui::{
    style::Style,
    text::{Line, Span},
};

#[derive(Clone)]
struct Glyph {
    character: char,
    style: Style,
}

fn width(glyphs: &[Glyph]) -> usize {
    glyphs
        .iter()
        .map(|g| Span::raw(g.character.to_string()).width())
        .sum()
}

pub fn wrap(line: Line<'static>, columns: u16, words: bool) -> Vec<Line<'static>> {
    let mut result = Vec::new();
    let mut pending = Vec::new();
    let columns = columns.max(1) as usize;
    for span in line.spans {
        for character in span
            .content
            .replace('\t', "    ")
            .chars()
            .filter(|c| !c.is_control())
        {
            let glyph = Glyph {
                character,
                style: line.style.patch(span.style),
            };
            if !pending.is_empty()
                && width(&pending) + width(std::slice::from_ref(&glyph)) > columns
            {
                flush_wrapped(&mut result, &mut pending, words);
            }
            pending.push(glyph);
        }
    }
    result.push(render(pending));
    result
}

fn flush_wrapped(result: &mut Vec<Line<'static>>, pending: &mut Vec<Glyph>, words: bool) {
    let boundary = if words {
        pending
            .iter()
            .rposition(|g| g.character.is_whitespace())
            .filter(|index| *index > 0)
            .map(|index| index + 1)
    } else {
        None
    };
    let rest = pending.split_off(boundary.unwrap_or(pending.len()));
    if words {
        while pending.last().is_some_and(|g| g.character.is_whitespace()) {
            pending.pop();
        }
    }
    result.push(render(std::mem::replace(pending, rest)));
}

fn render(glyphs: Vec<Glyph>) -> Line<'static> {
    let mut spans: Vec<Span<'static>> = Vec::new();
    for glyph in glyphs {
        if let Some(last) = spans.last_mut().filter(|span| span.style == glyph.style) {
            last.content.to_mut().push(glyph.character);
        } else {
            spans.push(Span::styled(glyph.character.to_string(), glyph.style));
        }
    }
    Line::from(spans)
}
