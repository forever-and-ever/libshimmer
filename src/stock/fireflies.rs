//! Fireflies: every caption letter flashes on its own slow rhythm, and fast output pulls them into step.

use std::f64::consts::TAU;

use super::OPUS;
use crate::kit::{GOLD, MINT, SUN, WHITE, frac, hash};
use crate::paint::{Ink, Palette, Rgb};
use crate::recipe::{Glyphs, Recipe};
use crate::stage::{Canvas, Shader, Stage, TICK_SECS};

pub(super) const RECIPE: Recipe = Recipe {
    name: "Fireflies",
    blurb: "letters flash and sync with output",
    credit: OPUS,
    needs: Glyphs::BASE,
    listens: false,
    summon: |_| Box::new(Fireflies::new()),
};

/// Oscillators; letters beyond them share.
const SWARM: usize = 32;
/// The swarm's speed: oscillators run near half a hertz.
const RATE: f64 = 0.5;
/// The share of a cycle a letter glows, and seconds the whole swarm glows when it locks.
const FLASH: f64 = 0.14;
const BURST: f64 = 1.5;
const DARK: Rgb = Rgb(112.0, 124.0, 94.0);
const FIELD: Rgb = Rgb(205.0, 255.0, 90.0);
const NIGHT: Rgb = Rgb(60.0, 70.0, 40.0);

struct Fireflies {
    /// Each oscillator's phase in turns.
    phases: [f64; SWARM],
    /// The swarm's coherence in [0, 1] and mean phase in turns.
    coherence: f64,
    mean: f64,
    /// When the swarm last locked into step.
    burst: Option<f64>,
}

impl Fireflies {
    #[expect(
        clippy::cast_possible_wrap,
        reason = "Oscillator indices are bounded by SWARM = 32."
    )]
    fn new() -> Self {
        Self {
            phases: std::array::from_fn(|i| hash(i as i64 + 500)),
            coherence: 0.0,
            mean: 0.0,
            burst: None,
        }
    }

    fn bursting(&self, t: f64) -> bool {
        self.burst.is_some_and(|at| t - at < BURST)
    }
}

impl Shader for Fireflies {
    /// The Kuramoto model: each oscillator is pulled toward the swarm's mean phase.
    #[expect(
        clippy::cast_possible_wrap,
        reason = "Oscillator indices are bounded by the 32-element phase array."
    )]
    fn tick(&mut self, stage: &Stage) {
        let lit = |i: usize| stage.caption.get(i).is_some_and(|letter| letter != " ");
        let pull = if stage.busy() {
            2.2 * (1.0 + 0.3 * (TAU * stage.t / 71.0).sin())
                * (stage.flow(/*window*/ 0.5) / 70.0).min(1.5)
        } else {
            0.0
        };
        let (mut x, mut y, mut n) = (0.0, 0.0, 0.0);
        for (i, phase) in self.phases.iter().enumerate() {
            if lit(i) {
                x += (TAU * phase).cos();
                y += (TAU * phase).sin();
                n += 1.0;
            }
        }
        let coherence = if n > 0.0 { x.hypot(y) / n } else { 0.0 };
        let mean = y.atan2(x) / TAU;
        for (i, phase) in self.phases.iter_mut().enumerate() {
            let drift = 0.9 + 0.3 * hash(17 * i as i64 + 3);
            let coupling = if lit(i) {
                pull * coherence / TAU * (TAU * (mean - *phase)).sin()
            } else {
                0.0
            };
            *phase = frac(*phase + RATE * TICK_SECS * (drift + coupling));
        }
        if coherence > 0.98
            && self.coherence <= 0.98
            && self.burst.is_none_or(|at| stage.t - at > 2.0 / RATE)
        {
            self.burst = Some(stage.t);
        }
        self.coherence = coherence;
        self.mean = mean;
    }

    fn status(&self, stage: &Stage, _palette: Palette, canvas: &mut Canvas) {
        let bursting = self.bursting(stage.t);
        let glow = if stage.busy() { FIELD } else { MINT }.blend(DARK, /*alpha*/ 0.5);
        canvas.mark = vec![
            Ink::of('◉')
                .fg(if bursting {
                    WHITE.blend(DARK, /*alpha*/ 0.5)
                } else {
                    SUN.blend(NIGHT, self.coherence)
                })
                .bold(true),
        ];
        let gold = GOLD.blend(DARK, /*alpha*/ 0.5);
        canvas.letters = stage.letters(|i, _| {
            if bursting {
                return Ink::default().fg(gold).bold(true);
            }
            let flash = (1.0 - self.phases[i % SWARM] / FLASH).max(0.0);
            Ink::default().fg(glow.blend(DARK, flash)).bold(flash > 0.5)
        });
    }

    fn bullet(&self, stage: &Stage, _palette: Palette) -> Ink {
        let glow = if stage.busy() { FIELD } else { MINT }.blend(DARK, /*alpha*/ 0.5);
        Ink::of('•').fg(if frac(self.mean) < FLASH { glow } else { DARK })
    }
}
