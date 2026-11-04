//! Caterpillar: an inchworm measures the caption, arching and stretching along the underline and
//! nibbling the letter ahead. When the bout ends it pupates; the next bout ripens the chrysalis
//! until a monarch breaks out and fans the caption with its wings, and when that bout ends it lays
//! an egg, which the next bout hatches. Between bouts the leaf stirs, the egg pulses and the
//! chrysalis glints.

use std::f64::consts::TAU;

use super::OPUS;
use crate::Spell;
use crate::kit::{WHITE, bump, ease, frac, noise, real};
use crate::paint::{Ink, Palette, Rgb};
use crate::recipe::{Glyphs, Recipe};
use crate::stage::{Canvas, Shader, Stage, TICK_SECS};

pub(super) const RECIPE: Recipe = Recipe {
    name: "Caterpillar",
    blurb: "an inchworm crawls; rest gives wings",
    credit: OPUS,
    needs: Glyphs::BASE,
    listens: false,
    summon: |seed| Box::new(Caterpillar::new(seed)),
};

const LARVA: Rgb = Rgb(118.0, 204.0, 82.0);
const LARVA_HEAD: Rgb = Rgb(198.0, 240.0, 96.0);
const JADE: Rgb = Rgb(86.0, 200.0, 150.0);
const GILT: Rgb = Rgb(240.0, 202.0, 82.0);
const MONARCH: Rgb = Rgb(255.0, 142.0, 38.0);
const EGG: Rgb = Rgb(244.0, 236.0, 212.0);
const LEAF: Rgb = Rgb(96.0, 168.0, 84.0);
/// Quanta a larva eats to grow a segment, and its most segments.
const MEAL: u64 = 240;
const LONGEST: usize = 7;
/// Quanta that ripen a chrysalis.
const RIPENING: f64 = 650.0;
/// Gusts the butterfly keeps travelling at once.
const GUSTS: usize = 6;

/// The creature's life, which the end of each bout advances.
enum Life {
    /// Laid at a bout's end, hatching when the next begins.
    Egg,
    /// `size` segments long, `fed` quanta toward the next, its tail on letter `at`, counted around
    /// the caption, and `gait` of the way through a stride.
    Larva {
        size: usize,
        fed: u64,
        at: f64,
        gait: f64,
    },
    /// `ripe` of the way to breaking out.
    Pupa { ripe: f64 },
    /// Out since `eclosed`, `flap` wingbeats so far; `gusts` holds the latest launch times, the
    /// oldest at `next`.
    Imago {
        eclosed: f64,
        flap: f64,
        gusts: [f64; GUSTS],
        next: usize,
    },
}

struct Caterpillar {
    seed: i64,
    life: Life,
}

impl Caterpillar {
    fn new(seed: u64) -> Self {
        Self {
            seed: seed.cast_signed(),
            life: Life::Egg,
        }
    }
}

/// Letters a looper of `size` segments advances each stride.
fn stride(size: usize) -> usize {
    size.saturating_sub(2).max(1)
}

/// The looper's tail and head letters, and how high its middle arches, `gait` of the way through
/// a stride from tail letter `at`: the tail draws up to the head, then the head stretches forward.
fn body(size: usize, at: f64, gait: f64) -> (f64, f64, f64) {
    let stride = real(stride(size));
    let head = at + real(size - 1);
    if gait < 0.5 {
        let draw = ease(gait / 0.5);
        (at + stride * draw, head, draw)
    } else {
        let stretch = ease((gait - 0.5) / 0.5);
        (at + stride, head + stride * stretch, 1.0 - stretch)
    }
}

/// Whether wings `flap` beats along are open, as they are on every other beat.
fn open(flap: f64) -> bool {
    frac(flap / 2.0) < 0.5
}

/// The letter nearest position `x`, which is never negative.
#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "Positions along a caption are nonnegative and far inside usize."
)]
fn whole(x: f64) -> usize {
    x.round() as usize
}

impl Shader for Caterpillar {
    fn tick(&mut self, stage: &Stage) {
        let live = stage.spell != Spell::Idle;
        let busy = stage.busy();
        let zest = (stage.flow(/*window*/ 0.5) / 70.0).min(1.5);
        match &mut self.life {
            Life::Egg => {
                if live {
                    self.life = Life::Larva {
                        size: 2,
                        fed: 0,
                        at: 0.0,
                        gait: 0.0,
                    };
                }
            }
            Life::Larva {
                size,
                fed,
                at,
                gait,
            } => {
                *fed += u64::from(stage.fresh());
                if *fed >= MEAL && *size < LONGEST {
                    *fed -= MEAL;
                    *size += 1;
                }
                let pace = if busy {
                    0.7 + 0.7 * zest
                } else if live {
                    0.12
                } else {
                    0.0
                };
                *gait += TICK_SECS * pace;
                if *gait >= 1.0 {
                    *gait -= 1.0;
                    *at += real(stride(*size));
                }
            }
            Life::Pupa { ripe } => {
                if live {
                    *ripe += f64::from(stage.fresh()) / RIPENING;
                }
                if *ripe >= 1.0 && busy {
                    self.life = Life::Imago {
                        eclosed: stage.t,
                        flap: 0.0,
                        gusts: [f64::NEG_INFINITY; GUSTS],
                        next: 0,
                    };
                }
            }
            Life::Imago {
                flap, gusts, next, ..
            } => {
                let beat = flap.floor();
                let pace = if busy {
                    5.0 + 3.0 * zest
                } else if live {
                    0.7
                } else {
                    0.25
                };
                *flap += TICK_SECS * pace;
                // every downstroke sends a gust down the line
                if live && flap.floor() > beat && open(*flap) {
                    gusts[*next] = stage.t;
                    *next = (*next + 1) % GUSTS;
                }
            }
        }
    }

    fn idle(&mut self, _stage: &Stage) {
        match self.life {
            Life::Larva { .. } => self.life = Life::Pupa { ripe: 0.0 },
            Life::Imago { .. } => self.life = Life::Egg,
            Life::Egg | Life::Pupa { .. } => {}
        }
    }

    fn status(&self, stage: &Stage, palette: Palette, canvas: &mut Canvas) {
        let t = stage.t;
        let base = palette.grey(if stage.busy() { 0.66 } else { 0.5 });
        let monarch = palette.legible(MONARCH);
        // the leaf stirs now and then in the breeze
        let stirred = noise(t * 0.5, self.seed) > 0.62;
        let leaf = Ink::of(if stirred { '❧' } else { '❦' }).fg(palette.legible(LEAF));
        match self.life {
            Life::Egg => {
                canvas.letters = stage.letters(|_, _| Ink::default().fg(base));
                // the egg pulses, slow as a sleeper's heart
                let pulse = 0.8 + 0.2 * (TAU * t / 3.2).sin();
                let egg = palette.legible(EGG).blend(palette.grey(0.35), pulse);
                canvas.mark = vec![leaf, Ink::of('∘').fg(egg)];
            }
            Life::Larva { size, at, gait, .. } => {
                let larva = palette.legible(LARVA);
                let head = palette.legible(LARVA_HEAD);
                let (tail, front, arch) = body(size, at, gait);
                let period = stage.caption.len() + size + 3;
                let first = whole(tail);
                let reach = whole(front) - first;
                let first = first % period;
                let nibbled = palette
                    .grey(0.3)
                    .blend(base, 0.5 + 0.5 * (TAU * 5.0 * t).sin());
                canvas.letters = stage.letters(|i, _| {
                    let rel = (i + period - first) % period;
                    if rel > reach {
                        // the letter it is nibbling
                        let nibbling = rel == reach + 1 && arch > 0.3;
                        Ink::default().fg(if nibbling { nibbled } else { base })
                    } else if rel == reach {
                        Ink::default().fg(head).bold(true).underline(head)
                    } else if rel == 0 || arch <= 0.3 {
                        Ink::default().fg(base).underline(larva)
                    } else {
                        Ink::default().fg(larva.blend(base, 0.4 + 0.6 * arch))
                    }
                });
                canvas.mark = vec![leaf, Ink::of(' ')];
            }
            Life::Pupa { ripe } => {
                canvas.letters = stage.letters(|_, _| Ink::default().fg(base));
                // the ripening chrysalis shows the wings through its jade
                let shell = monarch
                    .blend(palette.grey(0.25), 0.7)
                    .blend(palette.legible(JADE), ripe.clamp(0.0, 1.0));
                let glint = frac(t / 2.3) < 0.06;
                let girdle = palette.legible(if glint { WHITE } else { GILT });
                canvas.mark = vec![leaf, Ink::of('θ').fg(shell).underline(girdle)];
            }
            Life::Imago {
                eclosed,
                flap,
                gusts,
                ..
            } => {
                let burst = t - eclosed;
                canvas.letters = stage.letters(|i, _| {
                    let i = real(i);
                    let sweep = if burst < 1.2 {
                        0.9 * bump(i - burst * 25.0, 3.0)
                    } else {
                        0.0
                    };
                    let wind = gusts
                        .iter()
                        .map(|launch| t - launch)
                        .filter(|age| *age < 3.0)
                        .map(|age| 0.75 * bump(i - age * 12.0, 2.0) * (-age / 0.9).exp())
                        .fold(sweep, f64::max);
                    Ink::default()
                        .fg(monarch.blend(base, wind))
                        .bold(wind > 0.6)
                });
                let (left, right) = if open(flap) {
                    ('ʚ', 'ɞ')
                } else {
                    ('ɛ', 'ɜ')
                };
                canvas.mark = vec![
                    Ink::of(left).fg(monarch).bold(true),
                    Ink::of(right).fg(monarch).bold(true),
                ];
            }
        }
    }

    fn bullet(&self, _stage: &Stage, palette: Palette) -> Ink {
        let (glyph, tone) = match self.life {
            Life::Egg => ('∘', EGG),
            Life::Larva { .. } => ('•', LARVA),
            Life::Pupa { .. } => ('•', JADE),
            Life::Imago { flap, .. } => ('•', if open(flap) { GILT } else { MONARCH }),
        };
        Ink::of(glyph).fg(palette.legible(tone))
    }
}
