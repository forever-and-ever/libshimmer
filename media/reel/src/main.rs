//! Dumps the README reel's frames as JSON lines: every stock recipe paints its own name, driven
//! through the preview script and its song, rendered into a ratatui buffer exactly as a host draws it.
//!
//! usage: reel WARM_CYCLES > frames.jsonl, as ../roster.py runs it

use std::fmt::Write as _;
use std::io::{self, BufWriter, Write};
use std::time::Duration;

use libshimmer::{Glyphs, Motion, Palette, Rgb, Shimmer, TICK, script, stock};
use libshimmer_ratatui::RowExt as _;
use ratatui_core::buffer::Buffer;
use ratatui_core::layout::Rect;
use ratatui_core::style::{Color, Modifier};
use ratatui_core::text::Line;
use ratatui_core::widgets::Widget;

const WIDTH: u16 = 80;
/// The preview script's cycle.
const CYCLE: Duration = Duration::from_secs(20);

fn json(text: &str) -> String {
    let mut out = String::from("\"");
    for c in text.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            c if (c as u32) < 0x20 => {
                let _ = write!(out, "\\u{:04x}", c as u32);
            }
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

fn hex(color: Color) -> String {
    match color {
        Color::Rgb(r, g, b) => format!("{r:02x}{g:02x}{b:02x}"),
        _ => "-".into(),
    }
}

fn main() -> io::Result<()> {
    let warm: u32 = std::env::args().nth(1).and_then(|a| a.parse().ok()).unwrap_or(1);
    let palette = Palette { fg: Rgb::new(208, 208, 208), bg: Rgb::new(14, 14, 17) };
    let motion = Motion { mark: true, letters: true };
    let glyphs = Glyphs::BASE.union(Glyphs::OCTANTS);
    let mut shimmers = Vec::new();
    for (i, recipe) in stock::ROSTER.iter().enumerate() {
        let mut shimmer: Shimmer = Shimmer::new(recipe, 11 + i as u64, glyphs)
            .map_err(|unfit| io::Error::other(format!("{}: {unfit:?}", recipe.name())))?;
        shimmer.show(recipe.name());
        shimmers.push(shimmer);
    }
    let mut out = BufWriter::new(io::stdout().lock());
    let names: Vec<String> = stock::ROSTER.iter().map(|r| json(r.name())).collect();
    let octants: Vec<String> = (0..=255u8).map(|p| format!("{}:{p}", json(&Glyphs::octant(p).to_string()))).collect();
    writeln!(out, "{{\"tick_ms\":{},\"names\":[{}],\"octants\":{{{}}}}}", TICK.as_secs_f64() * 1000.0, names.join(","), octants.join(","))?;
    let per_cycle = (CYCLE.as_secs_f64() / TICK.as_secs_f64()).round() as u32;
    let area = Rect::new(0, 0, WIDTH, 1);
    for n in 0..per_cycle * (warm + 1) {
        let cue = script::cue(TICK * n);
        let sound = script::sound(TICK * n, TICK * (n + 1));
        for shimmer in &mut shimmers {
            shimmer.set_spell(cue.spell);
            shimmer.set_intensity(cue.intensity);
            shimmer.hear(&sound);
            shimmer.advance(TICK);
        }
        if n < per_cycle * warm {
            continue;
        }
        let mut rows = Vec::new();
        for shimmer in &shimmers {
            let row = shimmer.status(palette, motion);
            let mut buf = Buffer::empty(area);
            Line::from(row.spans()).render(area, &mut buf);
            row.shade(area, &mut buf);
            let cells: Vec<String> = (0..WIDTH)
                .map(|x| {
                    let cell = &buf[(x, 0)];
                    let underline = if cell.modifier.contains(Modifier::UNDERLINED) { hex(cell.underline_color) } else { "-".into() };
                    format!("[{},\"{}\",\"{}\",{},\"{}\"]", json(cell.symbol()), hex(cell.fg), hex(cell.bg), u8::from(cell.modifier.contains(Modifier::BOLD)), underline)
                })
                .collect();
            rows.push(format!("[{}]", cells.join(",")));
        }
        writeln!(out, "{{\"spell\":\"{:?}\",\"rows\":[{}]}}", cue.spell, rows.join(","))?;
    }
    out.flush()
}
