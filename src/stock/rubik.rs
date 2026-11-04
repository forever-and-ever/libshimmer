//! Rubik: two quadrant cells are a two-color puzzle that scrambles and solves itself as quanta arrive and,
//! more slowly, while the host waits; between bouts it holds still.

use super::OPUS;
use crate::Spell;
use crate::kit::{
    AZURE, CORAL, CYAN, GOLD, LIME, MAGENTA, MINT, PINK, QUADRANTS, SUN, TANGERINE, VIOLET, WHITE,
    chance, hash,
};
use crate::paint::{Ink, Palette, Rgb};
use crate::recipe::{Glyphs, Recipe};
use crate::stage::{Canvas, Shader, Stage, TICK_SECS};

pub(super) const RECIPE: Recipe = Recipe {
    name: "Rubik",
    blurb: "a two-cell puzzle solves itself",
    credit: OPUS,
    needs: Glyphs::BASE,
    listens: false,
    summon: |_| Box::<Rubik>::default(),
};

/// The five moves as sticker permutations, `after[i] = before[move[i]]`: turn the left cell either
/// way, turn the right cell either way, swap the inner columns.
const TWISTS: [[usize; 8]; 5] = [
    [2, 0, 3, 1, 4, 5, 6, 7],
    [1, 3, 0, 2, 4, 5, 6, 7],
    [0, 1, 2, 3, 6, 4, 7, 5],
    [0, 1, 2, 3, 5, 7, 4, 6],
    [0, 4, 2, 6, 1, 5, 3, 7],
];
const TWIST_SIGNS: [char; 5] = ['↻', '↺', '↻', '↺', '⇄'];
const UNDO: [usize; 5] = [1, 0, 3, 2, 4];
/// A sticker's quadrant bit within its cell, in sticker order.
const QUADRANT_BITS: [usize; 4] = [8, 4, 2, 1];
const SPLITS: [(Rgb, Rgb); 5] = [
    (PINK, CYAN),
    (SUN, VIOLET),
    (LIME, MAGENTA),
    (TANGERINE, AZURE),
    (CORAL, MINT),
];
/// Moves per round: scramble, hold, solve, flash, pause.
const ROUND: u64 = 40;

#[derive(Default)]
struct Rubik {
    /// Moves made while waiting, two a second.
    waited: f64,
}

/// One moment of the puzzle.
struct Puzzle {
    /// Which of the two colors each sticker shows.
    stickers: [bool; 8],
    colors: (Rgb, Rgb),
    /// The move under way, if one is.
    moving: Option<usize>,
    /// A solved round's flash.
    flash: Option<Rgb>,
}

impl Rubik {
    #[expect(
        clippy::cast_precision_loss,
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        clippy::cast_possible_wrap,
        reason = "The visual puzzle deliberately quantizes nonnegative quantum/time progress into moves; round = moves / 40 fits i64 and scramble indices are below 12."
    )]
    fn puzzle(&self, stage: &Stage) -> Puzzle {
        let moves = (stage.quanta as f64 / 10.0 + self.waited).floor() as u64;
        let round = moves / ROUND;
        let step = (moves % ROUND) as usize;
        let seed = round as i64;
        let lucky = chance(11 * seed + 3, /*odds*/ 0.125);
        let length = if lucky { 3 } else { 12 };
        let mut scramble: Vec<usize> = Vec::with_capacity(length);
        for q in 0..length {
            let twist = (hash(97 * seed + q as i64 + 5) * 5.0) as usize;
            let repeats_undo = scramble.last().is_some_and(|&last| UNDO[last] == twist);
            scramble.push(if repeats_undo { (twist + 1) % 5 } else { twist });
        }
        let done = match step {
            0..12 => (step + 1).min(length),
            12..18 => length,
            18..30 => length.saturating_sub(step - 17),
            _ => 0,
        };
        let mut stickers = [false, false, false, false, true, true, true, true];
        for &twist in &scramble[..done] {
            stickers = TWISTS[twist].map(|from| stickers[from]);
        }
        let moving = if step < length {
            Some(scramble[step])
        } else if (18..18 + length).contains(&step) {
            Some(UNDO[scramble[length - 1 - (step - 18)]])
        } else {
            None
        };
        Puzzle {
            stickers,
            colors: SPLITS[(round % 5) as usize],
            moving,
            flash: (30..33)
                .contains(&step)
                .then_some(if lucky { GOLD } else { WHITE }),
        }
    }
}

impl Shader for Rubik {
    fn tick(&mut self, stage: &Stage) {
        if stage.spell == Spell::Wait {
            self.waited += 2.0 * TICK_SECS;
        }
    }

    fn status(&self, stage: &Stage, palette: Palette, canvas: &mut Canvas) {
        let Puzzle {
            stickers,
            colors: (a, b),
            flash,
            ..
        } = self.puzzle(stage);
        let dim = |color: Rgb| {
            if stage.busy() {
                color
            } else {
                color.blend(palette.bg, /*alpha*/ 0.6)
            }
        };
        canvas.mark = [0, 4]
            .map(|first| {
                if let Some(flash) = flash {
                    Ink::of('█').fg(flash)
                } else {
                    let bits = (0..4)
                        .filter(|&q| !stickers[first + q])
                        .map(|q| QUADRANT_BITS[q])
                        .sum::<usize>();
                    Ink::of(QUADRANTS[bits]).fg(dim(a)).bg(dim(b))
                }
            })
            .to_vec();
        canvas.letters = stage.letters(|i, _| match flash {
            Some(flash) => Ink::default().fg(flash).bold(true),
            None => Ink::default().fg(dim(if stickers[i % 8] { b } else { a })),
        });
    }

    fn bullet(&self, stage: &Stage, palette: Palette) -> Ink {
        match self.puzzle(stage).moving {
            Some(twist) => Ink::of(TWIST_SIGNS[twist]).fg(WHITE),
            None => Ink::of('·').fg(palette.grey(/*alpha*/ 0.4)),
        }
    }
}
