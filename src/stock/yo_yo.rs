//! Yo-yo: a cherry-red yo-yo on a cotton string. Work throws it down the line, where it spins and
//! climbs back to the hand, and every 900 quanta it rocks the baby in a cradle of string. When the
//! host waits it throws a sleeper, spinning at the end of the string until work tugs it home;
//! between bouts it dozes at the string's end, turning a quarter every five seconds.

use std::f64::consts::TAU;
use std::mem;

use super::OPUS;
use crate::Spell;
use crate::kit::{Rng, bump, frac, real};
use crate::paint::{Ink, Palette, Rgb};
use crate::recipe::{Glyphs, Recipe};
use crate::rest::LULL;
use crate::stage::{Canvas, Shader, Stage, TICK_SECS};

pub(super) const RECIPE: Recipe = Recipe {
    name: "Yo-yo",
    blurb: "work throws it; waiting, it sleeps",
    credit: OPUS,
    needs: Glyphs::BASE,
    listens: false,
    summon: |seed| Box::new(YoYo::new(seed)),
};

const CHERRY: Rgb = Rgb(236.0, 52.0, 74.0);
const COTTON: Rgb = Rgb(228.0, 222.0, 204.0);
/// The spinning yo-yo by the quadrant it faces.
const SPINNING: [char; 4] = ['◐', '◓', '◑', '◒'];
/// Seconds down the string, at its end on a quick throw, rocking the baby, and back up.
const DOWN: f64 = 0.42;
const BOTTOM: f64 = 0.1;
const ROCK: f64 = 2.4;
const UP: f64 = 0.36;
/// The spin a throw lands with, in radians a second, and the spin at which a sleeper is tugged home.
const LANDING: f64 = 30.0;
const TUG: f64 = 1.5;
/// Quanta between rocks of the baby.
const LULLABY: u64 = 900;
/// The spin of a yo-yo dozing at the string's end between bouts, in radians a second: a quarter
/// turn each lull.
const DOZE: f64 = TAU / 4.0 / LULL;

/// Where the yo-yo is in a throw.
#[derive(Clone, Copy)]
enum Throw {
    /// In the hand, `hold` seconds before the next throw.
    Hand { hold: f64 },
    /// Unwinding down the string.
    Down,
    /// At the string's end on a quick throw.
    Bottom,
    /// Spinning at the string's end until tugged home, or dozing there between bouts.
    Sleep,
    /// Swinging in a cradle of string mid-caption.
    Rock,
    /// Climbing back to the hand.
    Up,
}

struct YoYo {
    rng: Rng,
    throw: Throw,
    /// Seconds into this part of the throw.
    since: f64,
    /// Spin in radians a second, and the angle it has turned.
    spin: f64,
    angle: f64,
    /// Lullabies so far, and whether one is owed.
    lullabies: u64,
    rock: bool,
}

impl YoYo {
    fn new(seed: u64) -> Self {
        Self {
            rng: Rng::new(seed),
            throw: Throw::Hand { hold: 0.5 },
            since: 0.0,
            spin: 0.0,
            angle: 0.0,
            lullabies: 0,
            rock: false,
        }
    }

    /// The yo-yo's place in letters: the hand is at −2, and the end of the string at `n`, the
    /// tail's cell.
    fn place(&self, n: f64) -> f64 {
        let reach = n + 2.0;
        match self.throw {
            Throw::Hand { .. } => -2.0,
            // gravity unwinds the string, and the climb mirrors it
            Throw::Down => -2.0 + reach * (self.since / DOWN).powi(2),
            Throw::Up => n - reach * (1.0 - (1.0 - self.since / UP).powi(2)),
            Throw::Rock => n / 2.0 + 1.6 * (TAU * 1.1 * self.since).sin(),
            Throw::Bottom | Throw::Sleep => n,
        }
    }

    /// The spinning yo-yo as seen: the quadrant its angle faces, so that a fast spin strobes and a
    /// slow one visibly turns.
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "A turn's fraction, times four, floors to a quadrant in 0..4."
    )]
    fn frame(&self) -> char {
        SPINNING[((frac(self.angle / TAU) * 4.0) as usize).min(SPINNING.len() - 1)]
    }

    fn strung(&self) -> bool {
        !matches!(self.throw, Throw::Hand { .. })
    }
}

impl Shader for YoYo {
    fn tick(&mut self, stage: &Stage) {
        let busy = stage.busy();
        let idle = stage.spell == Spell::Idle;
        self.since += TICK_SECS;
        self.angle += TICK_SECS * self.spin;
        let sleeping = matches!(self.throw, Throw::Sleep);
        self.spin = if idle && sleeping {
            DOZE
        } else {
            self.spin * (-TICK_SECS / if sleeping { 6.0 } else { 1.2 }).exp()
        };
        let lullabies = stage.quanta / LULLABY;
        if lullabies > self.lullabies {
            self.lullabies = lullabies;
            self.rock = true;
        }
        let done = match self.throw {
            Throw::Hand { hold } => idle || self.since > hold,
            Throw::Down => self.since >= DOWN,
            Throw::Bottom => self.since >= BOTTOM,
            Throw::Sleep => busy || (!idle && self.spin < TUG),
            Throw::Rock => idle || self.since >= ROCK,
            Throw::Up => self.since >= UP,
        };
        if !done {
            return;
        }
        self.throw = match self.throw {
            Throw::Hand { .. } => Throw::Down,
            Throw::Down => {
                self.spin = if idle { DOZE } else { LANDING };
                if busy {
                    if mem::take(&mut self.rock) {
                        Throw::Rock
                    } else {
                        Throw::Bottom
                    }
                } else {
                    Throw::Sleep
                }
            }
            Throw::Bottom if idle => Throw::Sleep,
            Throw::Bottom | Throw::Sleep | Throw::Rock => Throw::Up,
            Throw::Up => Throw::Hand {
                hold: if busy {
                    0.1 + 0.7 * self.rng.unit()
                        / (0.5 + (stage.flow(/*window*/ 0.6) / 70.0).min(1.5))
                } else {
                    1.2
                },
            },
        };
        self.since = 0.0;
    }

    fn status(&self, stage: &Stage, palette: Palette, canvas: &mut Canvas) {
        let n = real(stage.caption.len());
        let x = self.place(n);
        let at = x.round();
        let cherry = palette.legible(CHERRY);
        let cotton = palette.legible(COTTON);
        let cradle = cherry.blend(cotton, 0.3);
        let base = palette.grey(if stage.busy() { 0.68 } else { 0.5 });
        let strung = self.strung();
        let rocking = matches!(self.throw, Throw::Rock);
        let moving = matches!(self.throw, Throw::Down | Throw::Up);
        let middle = n / 2.0;
        canvas.letters = stage.letters(|i, _| {
            let i = real(i);
            if (i - at).abs() < 0.5 {
                return Ink::default().fg(cherry).bold(true).underline(cherry);
            }
            // the yo-yo smears faint red as it passes
            let smear = if moving { 0.35 * bump(i - x, 2.2) } else { 0.0 };
            let ink = Ink::default().fg(cherry.blend(base, smear));
            let string = strung && if rocking { i < middle + 2.2 } else { i < x };
            match (string, rocking && (i - middle).abs() < 2.2) {
                (false, _) => ink,
                (true, true) => ink.underline(cradle),
                (true, false) => ink.underline(cotton),
            }
        });
        let down = strung && !rocking && x >= n - 0.5;
        canvas.tail = vec![if down {
            let glyph = if self.spin > 0.8 { self.frame() } else { '●' };
            let fade = 0.5 + 0.5 * (self.spin / 6.0).min(1.0);
            Ink::of(glyph)
                .fg(cherry.blend(palette.grey(0.3), fade))
                .bold(true)
        } else {
            Ink::of(' ')
        }];
        canvas.mark = vec![if strung {
            Ink::of('◦').fg(cotton)
        } else {
            Ink::of('●').fg(cherry).bold(true)
        }];
    }

    fn bullet(&self, _stage: &Stage, palette: Palette) -> Ink {
        let spinning = self.strung() && self.spin > 0.8;
        Ink::of(if spinning { self.frame() } else { '●' }).fg(palette.legible(CHERRY))
    }
}
