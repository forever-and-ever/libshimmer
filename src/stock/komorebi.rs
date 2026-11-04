//! Komorebi (木漏れ日): sunlight filtering through a canopy. Pools of light drift across the caption
//! as the breeze moves the leaves overhead, and now and then a falling leaf's shadow zigzags past.
//! Work is the breeze at noon and a wait is golden hour; between bouts it is dusk, the pale light
//! still drifting.

use super::OPUS;
use crate::Spell;
use crate::kit::{Rng, bump, frac, lag, noise, real, smooth};
use crate::paint::{Ink, Palette, Rgb};
use crate::recipe::{Glyphs, Recipe};
use crate::stage::{Canvas, Shader, Stage, TICK_SECS};

pub(super) const RECIPE: Recipe = Recipe {
    name: "Komorebi",
    blurb: "sunlight dapples through leaves",
    credit: OPUS,
    needs: Glyphs::BASE,
    listens: false,
    summon: |seed| Box::new(Komorebi::new(seed)),
};

/// The sunlight and the leaf shade at noon, at golden hour and at dusk.
const SUNLIGHT: [Rgb; 3] = [
    Rgb(255.0, 236.0, 168.0),
    Rgb(255.0, 192.0, 112.0),
    Rgb(176.0, 182.0, 236.0),
];
const LEAF_SHADE: [Rgb; 3] = [
    Rgb(92.0, 122.0, 90.0),
    Rgb(124.0, 98.0, 82.0),
    Rgb(74.0, 80.0, 108.0),
];
/// Seconds a leaf takes to fall past, and mean seconds between falling leaves.
const FALL: f64 = 2.6;
const LEAVES: f64 = 6.0;

struct Komorebi {
    seed: i64,
    rng: Rng,
    /// The canopy's drift, integrated from the breeze so that no change of pace jumps the pattern.
    drift: f64,
    breeze: f64,
    /// How much the small leaves flutter, from fresh quanta.
    flutter: f64,
    /// The weights of noon, golden hour and dusk.
    hour: [f64; 3],
    /// When the latest leaf began to fall, −∞ before the first.
    leaf: f64,
}

impl Komorebi {
    fn new(seed: u64) -> Self {
        Self {
            seed: seed.cast_signed(),
            rng: Rng::new(seed),
            drift: 0.0,
            breeze: 0.3,
            flutter: 0.0,
            hour: [0.0, 0.0, 1.0],
            leaf: f64::NEG_INFINITY,
        }
    }

    /// How sunlit letter `x` is: three layers of leaves drifting at different speeds and headings,
    /// the parallax of a canopy, thresholded into pools of light.
    fn canopy(&self, x: f64) -> f64 {
        let (d, s) = (self.drift, self.seed);
        let v = 0.55 * noise(0.31 * x + 0.8 * d, s)
            + 0.3 * noise(0.67 * x - 1.5 * d, s.wrapping_add(1))
            + 0.15 * noise(1.7 * x + 3.3 * d + 2.0 * self.flutter, s.wrapping_add(2));
        smooth(0.5, 0.68, v)
    }

    /// The sun and the shade at this hour.
    fn light(&self, palette: Palette) -> (Rgb, Rgb) {
        let at = |stops: [Rgb; 3]| {
            stops
                .iter()
                .zip(self.hour)
                .fold(Rgb(0.0, 0.0, 0.0), |sum, (stop, weight)| {
                    Rgb(
                        sum.0 + stop.0 * weight,
                        sum.1 + stop.1 * weight,
                        sum.2 + stop.2 * weight,
                    )
                })
        };
        let sun = palette.legible(at(SUNLIGHT));
        let shade = at(LEAF_SHADE).blend(palette.grey(0.5), 0.55);
        (sun, shade)
    }
}

impl Shader for Komorebi {
    fn tick(&mut self, stage: &Stage) {
        let breeze = if stage.busy() {
            0.45 + 0.9 * (stage.flow(/*window*/ 0.8) / 70.0).min(1.5)
        } else {
            0.18
        };
        self.breeze = lag(self.breeze, breeze, 1.5);
        self.drift += TICK_SECS * self.breeze;
        self.flutter = lag(self.flutter, (f64::from(stage.fresh()) / 6.0).min(1.0), 0.2);
        let hour = match stage.spell {
            Spell::Busy => 0,
            Spell::Wait => 1,
            Spell::Idle => 2,
        };
        for (k, weight) in self.hour.iter_mut().enumerate() {
            *weight = lag(*weight, if k == hour { 1.0 } else { 0.0 }, 2.0);
        }
        if stage.spell != Spell::Idle
            && stage.t - self.leaf > FALL
            && self.rng.chance(TICK_SECS / LEAVES)
        {
            self.leaf = stage.t;
        }
    }

    fn status(&self, stage: &Stage, palette: Palette, canvas: &mut Canvas) {
        let (sun, shade) = self.light(palette);
        let n = real(stage.caption.len());
        let fall = stage.t - self.leaf;
        let falling = fall < FALL;
        let leaf = if falling {
            -3.0 + fall * (n + 6.0) / FALL + 0.6 * (fall * 5.0).sin()
        } else {
            f64::NEG_INFINITY
        };
        canvas.letters = stage.letters(|i, _| {
            let i = real(i);
            // a falling leaf's shadow
            let light = self.canopy(i) * (1.0 - 0.8 * bump(i - leaf, 1.6));
            Ink::default()
                .fg(sun.blend(shade, light))
                .bold(light > 0.88)
        });
        let turned = if falling {
            frac(fall * 2.0) >= 0.5
        } else {
            frac(self.drift / 4.0) >= 0.5
        };
        let glyph = if turned { '❧' } else { '❦' };
        canvas.mark = vec![Ink::of(glyph).fg(palette.legible(LEAF_SHADE[0]).blend(sun, 0.55))];
    }

    fn bullet(&self, _stage: &Stage, palette: Palette) -> Ink {
        let (sun, shade) = self.light(palette);
        Ink::of('•').fg(sun.blend(shade, self.canopy(-3.0)))
    }
}
