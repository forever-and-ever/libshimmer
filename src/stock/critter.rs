//! Critter: a three-cell face at the bullet that reacts to the quantum stream and dozes while waiting; between bouts it
//! sleeps, now and then half-waking for a moment.

use super::OPUS;
use crate::Spell;
use crate::kit::{AZURE, CORAL, CYAN, LIME, Lottery, MINT, PINK, STEEL, SUN, VIOLET, chance, frac};
use crate::paint::{Ink, Palette, Rgb};
use crate::recipe::{Glyphs, Recipe};
use crate::rest::LULL;
use crate::stage::{Canvas, Shader, Stage, TICK_SECS};

pub(super) const RECIPE: Recipe = Recipe {
    name: "Critter",
    blurb: "a small face reacts to the output",
    credit: OPUS,
    needs: Glyphs::BASE,
    listens: false,
    summon: |_| Box::<Critter>::default(),
};

const SNEEZE: Lottery = Lottery {
    every: 13.0,
    odds: 0.15,
    span: 0.9,
    seed: 5,
};
/// Between bouts the sleeper half-wakes, about once a minute, for two and a half seconds.
const STIR: Lottery = Lottery {
    every: 4.0 * LULL,
    odds: 0.3,
    span: 0.5 * LULL,
    seed: 11,
};
const WINK: Lottery = Lottery {
    every: 7.0,
    odds: 0.3,
    span: 0.5,
    seed: 3,
};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Mood {
    Happy,
    Content,
    Surprised,
    Alarmed,
    Pondering,
    Wink,
    Sneeze,
    Sleepy,
    Asleep,
}

impl Mood {
    fn face(self) -> (&'static str, Rgb) {
        match self {
            Mood::Happy => ("◕‿◕", SUN),
            Mood::Content => ("–‿–", LIME),
            Mood::Surprised => ("◕o◕", CYAN),
            Mood::Alarmed => ("⊙ο⊙", CORAL),
            Mood::Pondering => ("◔_◔", AZURE),
            Mood::Wink => ("◕‿-", PINK),
            Mood::Sneeze => (">ω<", MINT),
            Mood::Sleepy => ("˘‿˘", VIOLET),
            Mood::Asleep => ("–_–", STEEL),
        }
    }
}

#[derive(Default)]
struct Critter {
    /// Happy above 45 quanta a second until the rate falls below 30.
    happy: bool,
    /// Blend weights for each mood's tint, waiting grey, and the initial foreground.
    tint: Option<[f64; 11]>,
}

impl Critter {
    /// Resolves the smoothed weights against the palette only when painting.
    fn tint(&self, palette: Palette) -> Rgb {
        let Some(weights) = self.tint else {
            return palette.fg;
        };
        let moods = [
            Mood::Happy,
            Mood::Content,
            Mood::Surprised,
            Mood::Alarmed,
            Mood::Pondering,
            Mood::Wink,
            Mood::Sneeze,
            Mood::Sleepy,
            Mood::Asleep,
        ];
        let tones = moods.map(|mood| mood.face().1.blend(palette.fg, /*alpha*/ 0.4));
        tones
            .into_iter()
            .chain([palette.grey(/*alpha*/ 0.6), palette.fg])
            .zip(weights)
            .fold(Rgb(0.0, 0.0, 0.0), |sum, (tone, weight)| {
                Rgb(
                    sum.0 + tone.0 * weight,
                    sum.1 + tone.1 * weight,
                    sum.2 + tone.2 * weight,
                )
            })
    }
    fn mood(&self, stage: &Stage) -> Mood {
        if !stage.busy() {
            return if stage.spell_t < 3.0
                || (stage.spell == Spell::Idle && STIR.at(stage.t).is_some())
            {
                Mood::Sleepy
            } else {
                Mood::Asleep
            };
        }
        if let Some(sneeze) = SNEEZE.at(stage.t) {
            return if sneeze < 0.45 {
                Mood::Surprised
            } else {
                Mood::Sneeze
            };
        }
        let now = stage.flow(/*window*/ 0.3);
        let then = (stage.quanta_in(/*window*/ 0.6) - stage.quanta_in(/*window*/ 0.3)) / 0.3;
        if stage.quiet > 4.0 {
            Mood::Pondering
        } else if stage.quiet > 1.2 {
            Mood::Alarmed
        } else if now - then > 35.0 {
            Mood::Surprised
        } else if WINK.at(stage.t).is_some() {
            Mood::Wink
        } else if self.happy {
            Mood::Happy
        } else {
            Mood::Content
        }
    }
}

impl Shader for Critter {
    fn tick(&mut self, stage: &Stage) {
        let now = stage.flow(/*window*/ 0.3);
        self.happy = now > 45.0 || self.happy && now >= 30.0;
        let target = if stage.busy() {
            self.mood(stage) as usize
        } else {
            9
        };
        let mut tint = self.tint.unwrap_or_else(|| {
            let mut weights = [0.0; 11];
            weights[10] = 1.0;
            weights
        });
        let retained = (-TICK_SECS / 0.25).exp();
        for (i, weight) in tint.iter_mut().enumerate() {
            *weight = *weight * retained + f64::from(i == target) * (1.0 - retained);
        }
        self.tint = Some(tint);
    }

    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_possible_wrap,
        reason = "Sneeze noise quantizes the nonnegative animation clock; caption indices fit i64 because they index an allocated slice."
    )]
    fn status(&self, stage: &Stage, palette: Palette, canvas: &mut Canvas) {
        let mood = self.mood(stage);
        let (face, tone) = face_now(mood, stage.t);
        canvas.mark = face
            .chars()
            .map(|glyph| Ink::of(glyph).fg(tone).bold(true))
            .collect();
        let tint = self.tint(palette);
        let jostle = (stage.t / 0.1).floor() as i64;
        canvas.letters = stage.letters(|i, _| {
            // A sneeze jostles letters into their neighbors' cells.
            let shoved = (mood == Mood::Sneeze && chance(7 * i as i64 + jostle, /*odds*/ 0.3))
                .then(|| stage.caption.get(i + 1))
                .flatten()
                .and_then(|next| {
                    let mut chars = next.chars();
                    chars.next().filter(|_| chars.next().is_none())
                });
            Ink {
                glyph: shoved,
                ..Ink::default().fg(tint)
            }
        });
    }

    #[expect(
        clippy::cast_possible_truncation,
        reason = "The sleep glyph deliberately alternates on whole animation-clock buckets."
    )]
    fn bullet(&self, stage: &Stage, palette: Palette) -> Ink {
        if stage.t % 60.0 < 2.0 {
            Ink::of('♪').fg(SUN).bold(true)
        } else if self.mood(stage) == Mood::Asleep {
            Ink::of(if (stage.t / 0.7).floor() as i64 % 2 == 1 {
                'Z'
            } else {
                'z'
            })
            .fg(VIOLET)
        } else {
            Ink::of('·').fg(palette.grey(/*alpha*/ 0.4))
        }
    }
}

/// The face to show: a happy critter blinks every 3.7 seconds.
fn face_now(mood: Mood, t: f64) -> (&'static str, Rgb) {
    let (face, tone) = mood.face();
    if mood == Mood::Happy && frac(t / 3.7) < 0.035 {
        ("–‿–", tone)
    } else {
        (face, tone)
    }
}
