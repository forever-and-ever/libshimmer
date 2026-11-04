//! Frost: waiting is cold. While the host waits, frost creeps in from both ends of the caption in
//! fern-like fingers, turning letters to rime and growing frost flowers in the gaps between words,
//! and it goes on growing between bouts, glinting. Work is warmth: it thaws the line, leaving
//! letters wet and glistening for a moment, and steady work shows as a faint heat shimmer.

use super::OPUS;
use crate::Spell;
use crate::kit::{Rng, WHITE, hash, lag, noise, real, smooth};
use crate::paint::{Ink, Palette, Rgb};
use crate::recipe::{Glyphs, Recipe};
use crate::stage::{Canvas, Shader, Stage, TICK_SECS};

pub(super) const RECIPE: Recipe = Recipe {
    name: "Frost",
    blurb: "frost creeps in; work thaws it",
    credit: OPUS,
    needs: Glyphs::BASE,
    listens: false,
    summon: |seed| Box::new(Frost::new(seed)),
};

const RIME: Rgb = Rgb(188.0, 224.0, 255.0);
const HOARFROST: Rgb = Rgb(236.0, 246.0, 255.0);
const HEARTH: Rgb = Rgb(255.0, 186.0, 116.0);
/// Letters that freeze; any past them stay clear.
const LETTERS: usize = 160;
/// Draws a second for a glint while the host waits and between bouts, and a glint's seconds.
const GLINTS: f64 = 3.0;
const IDLE_GLINTS: f64 = 1.0;
const GLINT: f64 = 0.3;

struct Frost {
    seed: i64,
    rng: Rng,
    /// Each letter's ice in [0, 1], and its meltwater.
    ice: [f64; LETTERS],
    wet: [f64; LETTERS],
    /// The warmth of work.
    warm: f64,
    /// The mean ice over the caption.
    mean: f64,
    /// The letter that last glinted, and when.
    glint: Option<(usize, f64)>,
}

impl Frost {
    fn new(seed: u64) -> Self {
        Self {
            seed: seed.cast_signed(),
            rng: Rng::new(seed),
            ice: [0.0; LETTERS],
            wet: [0.0; LETTERS],
            warm: 0.0,
            mean: 0.0,
            glint: None,
        }
    }
}

/// The frost flower standing in for a space under `ice`, once there is enough.
fn flower(ice: f64) -> Option<char> {
    if ice > 0.9 {
        Some('✻')
    } else if ice > 0.65 {
        Some('*')
    } else if ice > 0.35 {
        Some('·')
    } else {
        None
    }
}

impl Shader for Frost {
    fn tick(&mut self, stage: &Stage) {
        let n = stage.caption.len().min(LETTERS);
        let cold = !stage.busy();
        let warmth = if cold {
            0.0
        } else {
            (stage.flow(/*window*/ 0.5) / 60.0).min(1.5)
        };
        self.warm = lag(self.warm, warmth, 0.4);
        let melt = (-TICK_SECS / 0.6).exp();
        let mut sum = 0.0;
        for (i, salt) in (0..n).zip(0_i64..) {
            let was = self.ice[i];
            self.ice[i] = if cold {
                // ice nucleates at the ends and creeps inward; each letter has its own crystal
                // habit, so the front advances in fingers
                let edge = if i == 0 || i + 1 == n { 0.55 } else { 0.0 };
                let left = if i > 0 { self.ice[i - 1] } else { 0.0 };
                let right = if i + 1 < n { self.ice[i + 1] } else { 0.0 };
                let habit = 0.3 + 1.4 * hash(self.seed.wrapping_mul(977).wrapping_add(31 * salt));
                (was + TICK_SECS * habit * (edge + smooth(0.4, 1.0, left.max(right)))).min(1.0)
            } else {
                let jitter = 0.6 + 0.8 * hash(self.seed.wrapping_add(7 * salt));
                (was - TICK_SECS * (0.5 + 1.8 * self.warm) * jitter).max(0.0)
            };
            if was > 0.25 && self.ice[i] <= 0.25 {
                // it drips
                self.wet[i] = 1.0;
            }
            self.wet[i] *= melt;
            sum += self.ice[i];
        }
        self.mean = if n > 0 { sum / real(n) } else { 0.0 };
        let glints = if stage.spell == Spell::Idle {
            IDLE_GLINTS
        } else {
            GLINTS
        };
        if cold && n > 0 && self.rng.chance(TICK_SECS * glints) {
            let letter = self.rng.below(n);
            if self.ice[letter] > 0.8 {
                self.glint = Some((letter, stage.t));
            }
        }
    }

    fn status(&self, stage: &Stage, palette: Palette, canvas: &mut Canvas) {
        let t = stage.t;
        let rime = palette.legible(RIME);
        let hoar = palette.legible(HOARFROST);
        let hearth = palette.legible(HEARTH);
        let white = palette.legible(WHITE);
        let base = palette.grey(if stage.busy() { 0.72 } else { 0.52 });
        let petals = rime.blend(palette.grey(0.3), 0.55);
        canvas.letters = stage.letters(|i, letter| {
            let ice = self.ice.get(i).copied().unwrap_or(0.0);
            // frost flowers grow in the gaps between words
            if let Some(glyph) = flower(ice).filter(|_| letter == " ") {
                return Ink::of(glyph).fg(petals);
            }
            let haze = 0.22 * self.warm.min(1.0) * noise(real(i) * 0.45 - t * 1.4, self.seed);
            let frozen = if ice > 0.85 {
                // the crystals catch the light as it drifts across them
                let drift = noise(real(i) * 0.7 + t * 0.5, self.seed.wrapping_add(5));
                hoar.blend(rime, smooth(0.4, 0.8, drift))
            } else {
                rime
            };
            let mut fg = frozen.blend(hearth.blend(base, haze), 0.9 * smooth(0.0, 1.0, ice));
            let wet = self.wet.get(i).copied().unwrap_or(0.0);
            if wet > 0.02 {
                // meltwater glistens
                fg = rime.blend(fg, 0.6 * wet);
            }
            let glint = self
                .glint
                .filter(|&(at, _)| at == i)
                .map_or(0.0, |(_, since)| 1.0 - (t - since) / GLINT);
            if glint > 0.0 {
                fg = white.blend(fg, glint);
            }
            Ink::default().fg(fg).bold(glint > 0.5)
        });
        let mark = if self.mean > 0.5 { '❆' } else { '❅' };
        canvas.mark = vec![Ink::of(mark).fg(rime.blend(hearth, smooth(0.0, 0.6, self.mean)))];
    }

    fn bullet(&self, _stage: &Stage, palette: Palette) -> Ink {
        let thaw = smooth(0.0, 0.6, self.mean);
        Ink::of('•').fg(palette.legible(RIME).blend(palette.legible(HEARTH), thaw))
    }
}
