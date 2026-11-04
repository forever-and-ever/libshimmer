//! Tectonic: output loads a fault until the plates slip and a wave crosses the caption.

use super::DEEPSEEK;
use crate::kit::{AZURE, CORAL, GOLD, STEEL, WHITE, bump, chance};
use crate::paint::{Ink, Palette};
use crate::recipe::{Glyphs, Recipe};
use crate::stage::{Canvas, Shader, Stage, TICK_SECS};

pub(super) const RECIPE: Recipe = Recipe {
    name: "Tectonic",
    blurb: "two plates grind and slip",
    credit: DEEPSEEK,
    needs: Glyphs::BASE,
    listens: false,
    summon: |_| Box::<Tectonic>::default(),
};

const LOAD: u64 = 8;
const STRESS: f64 = 0.034;
const RELIEF: f64 = 0.012;
const SLIP: f64 = 0.9;
const WAVE: f64 = 55.0;
const BLOCKS: [char; 8] = ['▁', '▂', '▃', '▄', '▅', '▆', '▇', '█'];

struct Tectonic {
    stress: f64,
    seen: u64,
    quake: f64,
    fault: usize,
}

impl Default for Tectonic {
    fn default() -> Self {
        Self {
            stress: 0.0,
            seen: 0,
            quake: -SLIP,
            fault: 0,
        }
    }
}

impl Shader for Tectonic {
    fn tick(&mut self, stage: &Stage) {
        let target = stage.quanta / LOAD;
        if stage.busy() {
            if target > self.seen {
                let added = u32::try_from(target - self.seen).unwrap_or(u32::MAX);
                self.stress += STRESS * f64::from(added);
                self.seen = target;
            }
        } else {
            self.seen = target;
        }
        self.stress = (self.stress - RELIEF * TICK_SECS).max(0.0);
        if self.stress >= 1.0 {
            self.stress = 0.0;
            self.quake = stage.t;
            let gaps = stage.caption.len().saturating_sub(1);
            if gaps > 0 {
                self.fault %= gaps;
                self.fault = if chance(i64::try_from(self.seen).unwrap_or(i64::MAX), 0.5) {
                    (self.fault + 1) % gaps
                } else {
                    self.fault.checked_sub(1).unwrap_or(gaps - 1)
                };
            }
        }
    }

    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "The strain gauges deliberately quantize a clamped visual level into a block glyph."
    )]
    fn status(&self, stage: &Stage, palette: Palette, canvas: &mut Canvas) {
        let busy = stage.busy();
        let grey = palette.grey(/*alpha*/ 0.6);
        let width = stage.caption.len();
        let fault = if width > 1 {
            1 + self.fault % (width - 1)
        } else {
            width
        };
        let age = stage.t - self.quake;
        let wave = busy && (0.0..SLIP).contains(&age);
        let light = |column: usize| {
            if wave {
                bump(
                    f64::from(u32::try_from(column.abs_diff(fault)).unwrap_or(u32::MAX))
                        - age * WAVE,
                    /*width*/ 3.1,
                )
            } else {
                0.0
            }
        };
        canvas.letters = stage.letters(|i, _| {
            let side = if i < fault { CORAL } else { AZURE };
            let lit = light(i);
            let mut ink = Ink::default().fg(if busy {
                side.blend(STEEL, /*alpha*/ 0.38 - 0.18 * self.stress + 0.1 * lit)
            } else {
                grey
            });
            if busy {
                let alpha = if i == fault {
                    0.11 + 0.3 * lit
                } else {
                    0.035 + 0.04 * self.stress + 0.3 * lit
                };
                ink = ink.bg(if lit > 0.25 { WHITE } else { side }.blend(palette.bg, alpha));
                ink = ink.bold(lit > 0.48);
                if i == fault {
                    ink.glyph = Some('│');
                }
            }
            ink
        });
        canvas.mark = (0..3)
            .map(|gauge| {
                let level = (self.stress * 3.0 - f64::from(gauge)).clamp(0.0, 1.0);
                Ink::of(BLOCKS[(level * 7.0).round() as usize])
                    .fg(GOLD.blend(CORAL, level))
                    .bold(busy && level > 0.25)
            })
            .collect();
    }

    fn bullet(&self, stage: &Stage, palette: Palette) -> Ink {
        Ink::of('⌁')
            .fg(if stage.busy() {
                CORAL
            } else {
                palette.grey(/*alpha*/ 0.6)
            })
            .bold(stage.busy())
    }
}

#[cfg(test)]
mod tests {
    use unicode_segmentation::UnicodeSegmentation;
    use unicode_width::UnicodeWidthStr;

    use crate::kit::WHITE;
    use crate::{Glyphs, Intensity, Motion, Palette, Rgb, Shimmer, Spell, TICK, stock};

    #[test]
    fn tectonic_preserves_graphemes_across_wrapped_faults() {
        let palette = Palette {
            fg: Rgb(208.0, 208.0, 208.0),
            bg: Rgb(21.0, 21.0, 21.0),
        };
        let recipe = stock::named("Tectonic").unwrap();
        for caption in ["", "X", "Working", "e\u{301}界👩‍💻ing"] {
            let mut shimmer: Shimmer = Shimmer::new(recipe, 0, Glyphs::BASE).unwrap();
            shimmer.show(caption);
            shimmer.set_spell(Spell::Busy);
            // Enough quanta for one quake per tick, without changing its production cadence.
            shimmer.set_intensity(Intensity::new(
                (240.0 / (100.0 * TICK.as_secs_f64()) - 1.0).exp(),
            ));
            let letters: Vec<&str> = caption.graphemes(true).collect();
            let gaps = letters.len().saturating_sub(1);
            let mut previous: Option<usize> = None;
            let mut visited = 0_u64;
            let mut wrapped = false;
            for _ in 0..256 {
                shimmer.advance(TICK);
                let row = shimmer.status(
                    palette,
                    Motion {
                        mark: false,
                        letters: true,
                    },
                );
                let cells: Vec<_> = row.cells().collect();
                assert_eq!(cells.len(), letters.len());
                assert_eq!(
                    cells
                        .iter()
                        .map(|(symbol, _)| symbol.width())
                        .sum::<usize>(),
                    caption.width()
                );
                // The width guard hides the crack on wide letters, but its paint still identifies
                // the fault. At a quake's first tick, its light is one and stress has been relieved.
                let fault = cells
                    .iter()
                    .position(|(_, ink)| ink.bg == Some(WHITE.blend(palette.bg, 0.11 + 0.3)));
                if gaps == 0 {
                    assert_eq!(fault, None);
                    assert_eq!(
                        cells.iter().map(|(symbol, _)| *symbol).collect::<String>(),
                        caption
                    );
                    continue;
                }
                let fault = fault.unwrap();
                for (i, ((symbol, _), letter)) in cells.iter().zip(&letters).enumerate() {
                    assert_eq!(
                        *symbol,
                        if i == fault && letter.width() == 1 {
                            "│"
                        } else {
                            *letter
                        }
                    );
                }
                let gap = fault - 1;
                assert!(gap < gaps);
                if let Some(previous) = previous {
                    assert!((previous + 1) % gaps == gap || (gap + 1) % gaps == previous);
                    wrapped |= previous.abs_diff(gap) == gaps - 1;
                }
                previous = Some(gap);
                visited |= 1 << gap;
            }
            if gaps > 2 {
                assert!(visited.count_ones() > 2, "fault stayed near the centre");
                assert!(wrapped, "fault never wrapped");
            }
            for (spell, letters) in [
                (Spell::Wait, true),
                (Spell::Idle, true),
                (Spell::Busy, false),
            ] {
                shimmer.set_spell(spell);
                shimmer.advance(TICK);
                let row = shimmer.status(
                    palette,
                    Motion {
                        mark: false,
                        letters,
                    },
                );
                let text: String = row.cells().map(|(symbol, _)| symbol).collect();
                assert_eq!(text, caption);
            }
        }
    }
}
