//! Sandpile: grains enter six piles and avalanche into their neighbours.

use super::DEEPSEEK;
use crate::kit::{CORAL, GOLD, STEEL, TANGERINE, bump, hash, ramp};
use crate::paint::{Ink, Palette};
use crate::recipe::{Glyphs, Recipe};
use crate::stage::{Canvas, Shader, Stage};

pub(super) const RECIPE: Recipe = Recipe {
    name: "Sandpile",
    blurb: "grains topple into an avalanche",
    credit: DEEPSEEK,
    needs: Glyphs::BASE,
    listens: false,
    summon: |_| Box::<Sandpile>::default(),
};

const PILES: usize = 6;
const GRAIN: u64 = 8;
/// Seconds an avalanche lights the caption.
const SLIDE: f64 = 0.9;
const WAVE: f64 = 50.0;
const BLOCKS: [char; 8] = ['▁', '▂', '▃', '▄', '▅', '▆', '▇', '█'];

struct Sandpile {
    heights: [u16; PILES],
    seen: u64,
    at: f64,
}

impl Default for Sandpile {
    fn default() -> Self {
        Self {
            heights: [0; PILES],
            seen: 0,
            at: -SLIDE,
        }
    }
}

impl Shader for Sandpile {
    fn tick(&mut self, stage: &Stage) {
        let target = stage.quanta / GRAIN;
        if !stage.busy() {
            self.seen = target;
            return;
        }
        while self.seen < target {
            self.seen += 1;
            let seed = i64::try_from(self.seen)
                .unwrap_or(i64::MAX)
                .saturating_mul(3)
                .saturating_add(11);
            let pile = if hash(seed) < 0.5 { 2 } else { 3 };
            self.heights[pile] += 1;
            let mut moved = 0;
            for _ in 0..32 {
                let mut any = false;
                for i in 0..PILES {
                    if self.heights[i] >= 4 {
                        self.heights[i] -= 4;
                        moved += 1;
                        any = true;
                        if i > 0 {
                            self.heights[i - 1] += 1;
                        }
                        if i + 1 < PILES {
                            self.heights[i + 1] += 1;
                        }
                    }
                }
                if !any {
                    break;
                }
            }
            if moved > 0 {
                self.at = stage.t;
            }
        }
    }

    fn status(&self, stage: &Stage, palette: Palette, canvas: &mut Canvas) {
        let busy = stage.busy();
        let grey = palette.grey(/*alpha*/ 0.6);
        let age = stage.t - self.at;
        let sliding = busy && (0.0..SLIDE).contains(&age);
        let span = f64::from(u32::try_from(stage.caption.len()).unwrap_or(u32::MAX));
        canvas.mark = self
            .heights
            .iter()
            .map(|&height| {
                let level = f64::from(height) / 5.0;
                Ink::of(BLOCKS[usize::from(height.min(7))])
                    .fg(if busy {
                        ramp(&[STEEL, TANGERINE, CORAL], level)
                    } else {
                        grey
                    })
                    .bold(busy && height >= 3)
            })
            .collect();
        canvas.letters = stage.letters(|i, _| {
            let distance =
                (f64::from(u32::try_from(i).unwrap_or(u32::MAX)) - (span - 1.0) / 2.0).abs();
            let lit = if sliding {
                bump(distance - age * WAVE, /*width*/ 3.2)
            } else {
                0.0
            };
            let base = if lit > 0.0 { CORAL } else { STEEL };
            let mut ink = Ink::default()
                .fg(base.blend(grey, /*alpha*/ 0.24 + 0.54 * (1.0 - lit)))
                .bold(busy && lit > 0.55);
            if busy && lit > 0.0 {
                ink = ink.bg(CORAL.blend(palette.bg, /*alpha*/ 0.36 * lit));
            }
            ink
        });
    }

    fn bullet(&self, stage: &Stage, palette: Palette) -> Ink {
        Ink::of('▾')
            .fg(if stage.busy() {
                GOLD
            } else {
                palette.grey(/*alpha*/ 0.6)
            })
            .bold(stage.busy())
    }
}
