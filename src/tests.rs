//! The promises that hold across the roster.

use std::time::Duration;

use proptest::prelude::*;
use unicode_segmentation::UnicodeSegmentation;
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

use crate::recipe::repertoire;
use crate::{Glyphs, Ink, Motion, Palette, Rgb, Row, STALE, Shimmer, Spell, TICK, script, stock};

const PALETTE: Palette = Palette {
    fg: Rgb(208.0, 208.0, 208.0),
    bg: Rgb(21.0, 21.0, 21.0),
};
/// A caption mixing narrow and wide graphemes, among them the two-cell ideographic space.
const MIXED: &str = "Tracing 漢字\u{3000}shimmer 🦀 cadence";
/// Moving everything, still letters, still mark, and still everything.
const MOTIONS: [Motion; 4] = [
    Motion {
        mark: true,
        letters: true,
    },
    Motion {
        mark: true,
        letters: false,
    },
    Motion {
        mark: false,
        letters: true,
    },
    Motion {
        mark: false,
        letters: false,
    },
];
/// Ticks in a little more than one cycle of the script.
const TAKE: u32 = 540;

/// The symbols of `row`'s cells.
fn symbols(row: &Row) -> Vec<&str> {
    row.cells().map(|(symbol, _)| symbol).collect()
}

/// Checks P7, P8 and P11 on everything `shimmer` paints now.
fn inspect(shimmer: &Shimmer, caption: &str) {
    let recipe = shimmer.recipe();
    let name = recipe.name();
    let letters: Vec<&str> = caption.graphemes(/*is_extended*/ true).collect();
    let [moving, marked, lettered, still] = MOTIONS.map(|motion| shimmer.status(PALETTE, motion));
    assert_eq!(symbols(&still), letters, "{name}");
    assert!(
        still.cells().all(|(_, ink)| ink == Ink::default()),
        "{name}"
    );
    assert!(
        still.backdrop().is_empty() && marked.backdrop().is_empty(),
        "{name}"
    );
    let lead = symbols(&marked).len() - letters.len();
    assert_eq!(symbols(&marked)[lead..], letters, "{name}");
    let tail = symbols(&lettered).len() - letters.len();
    let columns: usize = moving.cells().map(|(symbol, _)| symbol.width()).sum();
    let width: usize = letters.iter().map(|letter| letter.width()).sum();
    assert_eq!(
        moving.cells().count(),
        lead + letters.len() + tail,
        "{name}"
    );
    assert_eq!(columns, lead + width + tail, "{name}");
    let bullet = shimmer.bullet(PALETTE).glyph;
    assert!(
        bullet.is_none_or(|glyph| glyph.width() == Some(1)),
        "{name}"
    );
    for glyph in moving
        .cells()
        .flat_map(|(symbol, _)| symbol.chars())
        .chain(bullet)
    {
        assert!(
            recipe.needs().contains(repertoire(glyph)),
            "{name} paints {glyph}"
        );
    }
}

/// P7, P8 and P11 across the roster, through one take of the script with its own captions and
/// song, one with a caption mixing narrow and wide graphemes in silence, and a while idle.
#[test]
fn every_recipe_keeps_its_promises_through_the_script() {
    for recipe in stock::ROSTER {
        assert!(std::ptr::eq(stock::named(recipe.name()).unwrap(), recipe));
        let mut shimmer: Shimmer = Shimmer::new(recipe, 0x5eed, Glyphs::OCTANTS).unwrap();
        for mixed in [false, true] {
            for tick in 0..TAKE {
                let cue = script::cue(TICK * tick);
                let caption = if mixed { MIXED } else { cue.caption };
                shimmer.set_spell(cue.spell);
                shimmer.show(caption);
                shimmer.set_intensity(cue.intensity);
                if !mixed {
                    shimmer.hear(&script::sound(TICK * tick, TICK * (tick + 1)));
                }
                shimmer.advance(TICK);
                inspect(&shimmer, caption);
            }
        }
        shimmer.set_spell(Spell::Idle);
        for _ in 0..TAKE / 4 {
            shimmer.advance(TICK);
            inspect(&shimmer, MIXED);
        }
    }
}

/// Captions the property tests show: empty, narrow, wide, and a lone combining mark.
static CAPTIONS: [&str; 5] = [
    "",
    "Working",
    MIXED,
    "\u{301}",
    "Waiting for background terminal",
];

#[derive(Clone, Debug)]
enum Command {
    Show(&'static str),
    Cast(Spell),
    Set(f64),
    Hear(Vec<f32>),
    Advance(Duration),
}

impl Command {
    fn apply(&self, shimmer: &mut Shimmer) {
        match *self {
            Command::Show(caption) => shimmer.show(caption),
            Command::Cast(spell) => shimmer.set_spell(spell),
            Command::Set(level) => shimmer.set_intensity(crate::Intensity::new(level)),
            Command::Hear(ref sound) => shimmer.hear(sound),
            Command::Advance(dt) => shimmer.advance(dt),
        }
    }
}

fn commands() -> impl Strategy<Value = Vec<Command>> {
    let command = prop_oneof![
        prop::sample::select(&CAPTIONS[..]).prop_map(Command::Show),
        prop_oneof![Just(Spell::Idle), Just(Spell::Busy), Just(Spell::Wait)]
            .prop_map(Command::Cast),
        any::<f64>().prop_map(Command::Set),
        prop::collection::vec(any::<f32>(), 0..3_000).prop_map(Command::Hear),
        (0..2_000_000_000_u64).prop_map(|nanos| Command::Advance(Duration::from_nanos(nanos))),
    ];
    prop::collection::vec(command, 0..48)
}

/// Everything `shimmer` paints, as text, so that NaN channels compare equal.
fn paint(shimmer: &Shimmer) -> String {
    let rows = MOTIONS.map(|motion| shimmer.status(PALETTE, motion));
    format!("{rows:?} {:?}", shimmer.bullet(PALETTE))
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]

    /// P2 and P9: twins under one command sequence paint alike, and no command panics.
    #[test]
    fn twins_paint_alike(recipe in 0..stock::ROSTER.len(), seed: u64, commands in commands()) {
        let summon = || Shimmer::new(&stock::ROSTER[recipe], seed, Glyphs::OCTANTS).unwrap();
        let mut twins: [Shimmer; 2] = [summon(), summon()];
        for command in &commands {
            for twin in &mut twins {
                command.apply(twin);
            }
            prop_assert_eq!(paint(&twins[0]), paint(&twins[1]));
        }
    }

    /// P3: `advance(a)` then `advance(b)` paints as `advance(a + b)` while `a + b` stays within
    /// `STALE`.
    #[test]
    fn split_gaps_paint_alike(
        recipe in 0..stock::ROSTER.len(),
        seed: u64,
        commands in commands(),
        cut: u64,
    ) {
        let summon = || Shimmer::new(&stock::ROSTER[recipe], seed, Glyphs::OCTANTS).unwrap();
        let (mut whole, mut split): (Shimmer, Shimmer) = (summon(), summon());
        for command in &commands {
            command.apply(&mut whole);
            match *command {
                Command::Advance(dt) if dt <= STALE => {
                    let nanos = u64::try_from(dt.as_nanos()).unwrap();
                    let first = Duration::from_nanos(cut % (nanos + 1));
                    split.advance(first);
                    split.advance(dt.checked_sub(first).unwrap());
                }
                _ => command.apply(&mut split),
            }
            prop_assert_eq!(paint(&whole), paint(&split));
        }
    }
}

/// P12: a picker shows every blurb on one line.
#[test]
fn every_blurb_fits_one_line() {
    for recipe in stock::ROSTER {
        let width = recipe.blurb().width();
        assert!(
            width <= 36,
            "{}'s blurb spans {width} columns",
            recipe.name()
        );
    }
}
