//! Dandelion: the mark is a dandelion clock. Work blows its seeds loose; they drift along the
//! caption on a gusty wind, lifting each letter they pass to white, and rise out of sight just past
//! the timer, a mote glinting now and then as a seed tumbles. A bare stalk flowers again, and every
//! plant flowers between bouts; at rest, a breeze now and then lifts a stray seed from the new head.

use std::f64::consts::TAU;

use unicode_width::UnicodeWidthStr;

use super::OPUS;
use crate::Spell;
use crate::kit::{Rng, bump, lag, noise, real, smooth};
use crate::paint::{BACKDROP, Ink, Palette, Rgb};
use crate::recipe::{Glyphs, Recipe};
use crate::stage::{Canvas, Shader, Stage, TICK_SECS};

pub(super) const RECIPE: Recipe = Recipe {
    name: "Dandelion",
    blurb: "work sets the seeds adrift",
    credit: OPUS,
    needs: Glyphs::BASE,
    listens: false,
    summon: |seed| Box::new(Dandelion::new(seed)),
};

const PAPPUS: Rgb = Rgb(238.0, 238.0, 230.0);
const BLOOM: Rgb = Rgb(255.0, 212.0, 60.0);
const STALK: Rgb = Rgb(126.0, 172.0, 96.0);
/// The head by seeds left, a glyph for each ten, from thin to full.
const CLOCKS: [char; 5] = ['·', '*', '✷', '✹', '✺'];
/// Seeds to a full head, and the quanta that free each.
const HEAD: usize = 48;
const FARE: u32 = 50;
/// Seeds aloft at once.
const DRIFTERS: usize = 12;
/// Columns before the caption, the head's and a space's, and columns past it through which a seed
/// rises out of sight.
const LEAD: f64 = 2.0;
const SKY: f64 = 30.0;
/// Seconds a bare stalk stands before it flowers, and a flower lasts between bouts and within one.
const BARE: f64 = 5.0;
const IDLE_BLOOM: f64 = 2.5;
const BUSY_BLOOM: f64 = 9.0;
/// Mean seconds between the stray seeds a breeze lifts from the resting head.
const STRAY: f64 = 9.0;

/// A seed aloft: its column, its share of the wind, and its tumble in radians.
#[derive(Clone, Copy)]
struct Seed {
    x: f64,
    v: f64,
    spin: f64,
}

/// The plant's head.
#[derive(Clone, Copy)]
enum Head {
    /// A clock of `left` seeds, `owed` quanta toward freeing the next.
    Clock { left: usize, owed: u32 },
    /// A bare stalk, `since` seconds bare.
    Bare { since: f64 },
    /// A flower whose `bloom` falls from 1, when it opens, to 0, when it seeds a full clock.
    Flower { bloom: f64 },
}

struct Dandelion {
    seed: i64,
    rng: Rng,
    head: Head,
    wind: f64,
    seeds: [Option<Seed>; DRIFTERS],
}

impl Dandelion {
    fn new(seed: u64) -> Self {
        Self {
            seed: seed.cast_signed(),
            rng: Rng::new(seed),
            head: Head::Clock {
                left: HEAD,
                owed: 0,
            },
            wind: 1.0,
            seeds: [None; DRIFTERS],
        }
    }

    /// Frees a seed from the head into the wind, taking `v` of it, when one more may rise.
    fn loose(&mut self, v: f64) {
        if let Some(slot) = self.seeds.iter_mut().find(|slot| slot.is_none()) {
            *slot = Some(Seed {
                x: 0.5 + self.rng.unit(),
                v,
                spin: TAU * self.rng.unit(),
            });
        }
    }

    fn aloft(&self) -> impl Iterator<Item = &Seed> {
        self.seeds.iter().flatten()
    }

    /// How far the seeds lift the cell at `column` to white: by the nearest, flickering as it
    /// tumbles.
    fn lift(&self, column: f64) -> f64 {
        self.aloft()
            .map(|seed| bump(column - seed.x, 1.3) * (0.75 + 0.25 * seed.spin.sin()))
            .fold(0.0, f64::max)
    }

    fn head(&self, palette: Palette) -> Ink {
        let pappus = palette.legible(PAPPUS);
        let yellow = palette.legible(BLOOM);
        match self.head {
            Head::Clock { left, .. } => {
                Ink::of(CLOCKS[(left / 10).min(CLOCKS.len() - 1)]).fg(pappus)
            }
            Head::Bare { .. } => Ink::of('ı').fg(palette.legible(STALK)),
            Head::Flower { bloom } if bloom > 0.45 => Ink::of('❋').fg(yellow).bold(true),
            Head::Flower { bloom } if bloom > 0.2 => {
                Ink::of('✾').fg(yellow.blend(palette.legible(STALK), 0.4))
            }
            Head::Flower { bloom } => {
                Ink::of('✺').fg(pappus.blend(palette.grey(0.3), 1.0 - bloom / 0.2))
            }
        }
    }
}

/// The caption's columns.
fn width(stage: &Stage) -> f64 {
    real(stage.caption.iter().map(UnicodeWidthStr::width).sum())
}

impl Shader for Dandelion {
    fn tick(&mut self, stage: &Stage) {
        let idle = stage.spell == Spell::Idle;
        let gust = 0.6 + 0.8 * noise(stage.t * 0.35, self.seed);
        let wind = if stage.busy() {
            3.0 + 4.0 * (stage.flow(/*window*/ 0.6) / 70.0).min(1.5)
        } else {
            1.2
        };
        self.wind = lag(self.wind, wind * gust, 0.6);
        // a seed rises out of sight just past the timer
        let sky = LEAD + width(stage) + SKY;
        let wind = self.wind;
        for slot in &mut self.seeds {
            *slot = slot.and_then(|mut seed| {
                seed.x += TICK_SECS * wind * seed.v;
                seed.spin += TICK_SECS * 3.0;
                (seed.x <= sky).then_some(seed)
            });
        }
        self.head = match self.head {
            Head::Clock { mut left, mut owed } => {
                owed += stage.fresh();
                while owed >= FARE && left > 0 {
                    owed -= FARE;
                    left -= 1;
                    let v = 0.6 + 0.7 * self.rng.unit();
                    self.loose(v);
                }
                if idle && self.rng.chance(TICK_SECS / STRAY) {
                    let v = 0.4 + 0.4 * self.rng.unit();
                    self.loose(v);
                }
                if left == 0 {
                    Head::Bare { since: 0.0 }
                } else {
                    Head::Clock { left, owed }
                }
            }
            Head::Bare { since } if since + TICK_SECS > BARE => Head::Flower { bloom: 1.0 },
            Head::Bare { since } => Head::Bare {
                since: since + TICK_SECS,
            },
            Head::Flower { bloom } => {
                let bloom = bloom - TICK_SECS / if idle { IDLE_BLOOM } else { BUSY_BLOOM };
                if bloom > 0.0 {
                    Head::Flower { bloom }
                } else {
                    Head::Clock {
                        left: HEAD,
                        owed: 0,
                    }
                }
            }
        };
    }

    fn idle(&mut self, _stage: &Stage) {
        // between bouts the plant flowers and seeds a new clock
        if !matches!(self.head, Head::Flower { .. }) {
            self.head = Head::Flower { bloom: 1.0 };
        }
    }

    fn status(&self, stage: &Stage, palette: Palette, canvas: &mut Canvas) {
        let white = palette.legible(PAPPUS);
        let base = palette.grey(if stage.busy() { 0.7 } else { 0.5 });
        canvas.mark = vec![self.head(palette)];
        let mut column = LEAD;
        canvas.letters = stage.letters(|_, letter| {
            let lift = self.lift(column);
            column += real(letter.width());
            Ink::default().fg(white.blend(base, lift)).bold(lift > 0.6)
        });
        // past the caption the seeds float on as faint motes in the backdrop: only a seed that
        // catches the light as it tumbles shows there, fading as it rises out of sight
        let end = column;
        let far = self
            .aloft()
            .filter(|seed| seed.x > end - 1.5)
            .map(|seed| (seed.x + 1.5).ceil())
            .fold(0.0, f64::max);
        if far > end {
            canvas.backdrop = (0..BACKDROP)
                .map(real)
                .take_while(|&c| c < far)
                .map(|c| {
                    if c < end {
                        return palette.bg;
                    }
                    let glint = self
                        .aloft()
                        .map(|seed| bump(c - seed.x, 0.8) * smooth(0.5, 0.95, seed.spin.sin()))
                        .fold(0.0, f64::max);
                    white.blend(palette.bg, 0.12 * glint * smooth(end + SKY, end + 6.0, c))
                })
                .collect();
        }
    }

    fn bullet(&self, _stage: &Stage, palette: Palette) -> Ink {
        self.head(palette)
    }
}
