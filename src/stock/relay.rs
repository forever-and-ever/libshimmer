//! Relay: an eight-bit shift register walks a local bus across the caption.

use super::DEEPSEEK;
use crate::kit::{AZURE, CYAN, ICE, STEEL, SUN, WHITE};
use crate::paint::{Ink, Palette};
use crate::recipe::{Glyphs, Recipe};
use crate::stage::{Canvas, Shader, Stage};

pub(super) const RECIPE: Recipe = Recipe {
    name: "Relay",
    blurb: "a relay register walks the bus",
    credit: DEEPSEEK,
    needs: Glyphs::BASE,
    listens: false,
    summon: |_| Box::<Relay>::default(),
};

const SHIFT: u64 = 12;
const BUS_HALF_WIDTH: usize = 4;
const ARC: f64 = 0.36;

struct Relay {
    bits: u8,
    seen: u64,
    pulse: f64,
}

impl Default for Relay {
    fn default() -> Self {
        Self {
            bits: 1,
            seen: 0,
            pulse: -ARC,
        }
    }
}

impl Shader for Relay {
    fn tick(&mut self, stage: &Stage) {
        let target = stage.quanta / SHIFT;
        if !stage.busy() {
            self.seen = target;
            return;
        }
        while self.seen < target {
            self.seen += 1;
            let feedback = ((self.bits >> 7) ^ (self.bits >> 2)) & 1;
            let next = self.bits.wrapping_shl(1) | feedback;
            if next != self.bits {
                self.pulse = stage.t;
            }
            self.bits = next;
        }
    }

    fn status(&self, stage: &Stage, palette: Palette, canvas: &mut Canvas) {
        let busy = stage.busy();
        let grey = palette.grey(/*alpha*/ 0.6);
        let width = stage.caption.len();
        let head = if width == 0 {
            0
        } else {
            usize::try_from(stage.quanta / SHIFT).unwrap_or_default() % width
        };
        let arc = busy && stage.t - self.pulse < ARC;
        canvas.letters = stage.letters(|i, _| {
            let bit = (self.bits >> (i % 8)) & 1 == 1;
            let here = i == head;
            let near = i.abs_diff(head) <= BUS_HALF_WIDTH;
            let mut ink = Ink::default()
                .fg(if !busy {
                    grey
                } else if here {
                    WHITE
                } else if near && bit {
                    ICE
                } else {
                    grey
                })
                .bold(busy && (here || bit));
            if busy {
                if here {
                    ink = ink.bg(if arc { SUN } else { AZURE }.blend(palette.bg, /*alpha*/ 0.42));
                } else if near && bit {
                    ink = ink.bg(AZURE.blend(palette.bg, /*alpha*/ 0.13));
                }
            }
            ink
        });
        canvas.mark = (0..3)
            .map(|bit| {
                let on = (self.bits >> bit) & 1 == 1;
                Ink::of(if on { '●' } else { '○' })
                    .fg(if on { CYAN } else { STEEL })
                    .bold(busy && on)
            })
            .collect();
    }

    fn bullet(&self, stage: &Stage, palette: Palette) -> Ink {
        let busy = stage.busy();
        let arc = busy && stage.t - self.pulse < ARC;
        Ink::of(if arc { 'ϟ' } else { '⌁' })
            .fg(if busy {
                if arc { SUN } else { CYAN }
            } else {
                palette.grey(/*alpha*/ 0.6)
            })
            .bold(busy)
    }
}
