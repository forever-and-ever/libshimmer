//! Moth: a moth worries the lamp at the head of the line. Lamplight warms the first letters and
//! falls away along the caption; whenever the moth crosses between bulb and words, its shadow, huge
//! as it nears the glass, sweeps across them. Work burns the lamp full and drives the moth faster,
//! a burst of output now and then into the bulb. A wait sinks the lamp to half, and the moth rests
//! beside it, now and then taking a turn about it; between bouts the lamp is a breathing
//! night-light, beside which the moth rests, now and then fluttering in place.

use std::f64::consts::TAU;

use super::OPUS;
use crate::Spell;
use crate::kit::{Lottery, Rng, WHITE, bump, frac, glow, lag, noise, real, smooth};
use crate::paint::{Ink, Palette, Rgb};
use crate::recipe::{Glyphs, Recipe};
use crate::rest::LULL;
use crate::stage::{Canvas, Shader, Stage, TICK_SECS};

pub(super) const RECIPE: Recipe = Recipe {
    name: "Moth",
    blurb: "a moth and its shadow circle a lamp",
    credit: OPUS,
    needs: Glyphs::BASE,
    listens: false,
    summon: |seed| Box::new(Moth::new(seed)),
};

const LAMPLIGHT: Rgb = Rgb(255.0, 188.0, 104.0);
const DUSTY: Rgb = Rgb(182.0, 160.0, 128.0);
/// The night-light between bouts: its level, the share of it each breath swings, and a breath's
/// seconds.
const NIGHT: f64 = 0.16;
const BREATH: f64 = 0.6;
const BREATH_SECS: f64 = 5.0;
/// Mean seconds between a waiting moth's turns about the lamp, and the shortest turn's seconds.
const RESTLESS: f64 = 9.0;
const TURN: f64 = 1.2;
/// Between bouts the perched moth beats its wings twice in place, about every forty seconds.
const FLUTTER: Lottery = Lottery {
    every: 3.0 * LULL,
    odds: 0.4,
    span: 0.4 * LULL,
    seed: 9,
};

struct Moth {
    seed: i64,
    rng: Rng,
    /// The orbit's angle in radians, seen edge-on: its cosine picks the moth's cell of the mark,
    /// and its sine whether the moth is in front of the bulb.
    theta: f64,
    /// The lamp's light in [0, 1].
    light: f64,
    /// How settled the moth is beside the lamp, from 0 aloft to 1 perched.
    perch: f64,
    /// Wingbeats so far.
    flap: f64,
    /// When the moth last struck the bulb, −∞ before it first does.
    bumped: f64,
    /// When the resting moth's latest turn about the lamp ends.
    flight: f64,
}

impl Moth {
    fn new(seed: u64) -> Self {
        Self {
            seed: seed.cast_signed(),
            rng: Rng::new(seed),
            theta: 0.0,
            light: 0.0,
            perch: 1.0,
            flap: 0.0,
            bumped: f64::NEG_INFINITY,
            flight: f64::NEG_INFINITY,
        }
    }
}

impl Shader for Moth {
    fn tick(&mut self, stage: &Stage) {
        let t = stage.t;
        let busy = stage.busy();
        let light = match stage.spell {
            Spell::Busy => 1.0,
            Spell::Wait => 0.4,
            Spell::Idle => NIGHT * (1.0 + BREATH * (TAU * t / BREATH_SECS).sin()),
        };
        self.light = lag(self.light, light, if busy { 0.25 } else { 0.9 });
        if stage.spell == Spell::Wait
            && t >= self.flight
            && self.perch > 0.9
            && self.rng.chance(TICK_SECS / RESTLESS)
        {
            self.flight = t + TURN * (1.0 + 1.3 * self.rng.unit());
        }
        let aloft = busy || t < self.flight;
        self.perch = lag(self.perch, if aloft { 0.0 } else { 1.0 }, 0.4);
        let zeal = if busy {
            0.8 + 0.7 * (stage.flow(/*window*/ 0.5) / 70.0).min(1.5)
        } else if aloft {
            0.45
        } else {
            0.2
        };
        self.theta += TAU * TICK_SECS * zeal * (0.5 + noise(t * 0.8, self.seed));
        let beats = if busy {
            15.0
        } else if aloft {
            9.0
        } else {
            3.0
        };
        self.flap += TICK_SECS * beats;
        // a burst of work sometimes drives the moth into the bulb: tink
        if busy && stage.fresh() >= 4 && t - self.bumped > 1.5 && self.rng.chance(0.05) {
            self.bumped = t;
        }
    }

    fn status(&self, stage: &Stage, palette: Palette, canvas: &mut Canvas) {
        let t = stage.t;
        let amber = palette.legible(LAMPLIGHT);
        let since = t - self.bumped;
        let flash = if since < 0.25 {
            1.0 - since / 0.25
        } else {
            0.0
        };
        let light =
            self.light * (0.9 + 0.1 * noise(t * 12.0, self.seed.wrapping_add(7))) + 0.5 * flash;
        let radius = if since < 0.4 {
            0.12
        } else {
            0.45 + 0.55 * noise(t * 1.6, self.seed.wrapping_add(3))
        };
        let (front, cos) = self.theta.sin_cos();
        let side = radius * cos;
        let aloft = self.perch < 0.5;
        let open = if aloft {
            frac(self.flap / 2.0) < 0.5
        } else if stage.spell == Spell::Idle {
            // two wingbeats: open, shut, open, shut
            FLUTTER.at(t).is_some_and(|beat| frac(2.0 * beat) < 0.5)
        } else {
            frac(t / 3.7) < 0.1
        };
        let wing = if open { 'ж' } else { 'х' };
        let lamp = if self.light < 0.04 {
            Ink::of('○').fg(palette.grey(0.35))
        } else {
            let bulb = if flash > 0.0 {
                palette.legible(WHITE)
            } else {
                amber
            };
            Ink::of('☼')
                .fg(bulb.blend(palette.grey(0.3), (0.3 + light).min(1.0)))
                .bold(light > 0.6)
        };
        let gap = Ink::of(' ');
        let mut mark = vec![gap, lamp, gap];
        let cell = if !aloft || side < -0.3 {
            0
        } else if side > 0.3 {
            2
        } else {
            1
        };
        match cell {
            1 if front > 0.0 => {
                // in front of the bulb: a silhouette against the glare
                let glare = amber.blend(palette.bg, 0.5 * light.min(1.0));
                mark[1] = Ink::of(wing).fg(palette.ink_on(glare)).bg(glare).bold(true);
            }
            1 => {}
            _ => {
                let presence = if aloft {
                    (0.45 + 0.55 * light).min(1.0)
                } else {
                    0.6
                };
                mark[cell] =
                    Ink::of(wing).fg(palette.legible(DUSTY).blend(palette.grey(0.35), presence));
            }
        }
        canvas.mark = mark;
        // lamplight falls off along the caption; the moth's shadow, huge as it nears the bulb,
        // sweeps it
        let n = real(stage.caption.len());
        let base = palette.grey(if stage.busy() { 0.52 } else { 0.45 });
        let reach = 4.0 + 6.0 * self.light;
        let shade = if aloft && side > 0.05 {
            0.75 * light.min(1.0) * smooth(0.05, 0.4, side)
        } else {
            0.0
        };
        let centre = (n - 1.0) * (0.5 + 0.5 * (self.theta + 0.6).sin());
        let span = (n / 2.0 + 1.0).min(1.0 + 0.9 / side.max(0.12));
        let dusk = palette.grey(0.2);
        canvas.letters = stage.letters(|i, _| {
            let i = real(i);
            let lit = (light * glow(i + 3.0, reach)).min(1.0);
            let dark = shade * bump(i - centre, span);
            Ink::default()
                .fg(dusk.blend(amber.blend(base, lit), dark))
                .bold(lit > 0.7 && dark < 0.3)
        });
    }

    fn bullet(&self, _stage: &Stage, palette: Palette) -> Ink {
        let amber = palette.legible(LAMPLIGHT);
        Ink::of('•').fg(amber.blend(palette.grey(0.3), (0.25 + self.light).min(1.0)))
    }
}
