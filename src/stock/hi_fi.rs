//! Hi-fi: the caption is a hi-fi's spectrum analyzer, its letters the bands of whatever plays, bass
//! to treble, lit from blue through magenta to amber as they rise, and its mark a hi-fi's LED bars.
//! In silence it plays the host's work instead; at rest it plays on, quieter and grey.

use std::f64::consts::TAU;

use super::USER;
use crate::SAMPLE_RATE;
use crate::kit::{AZURE, MAGENTA, PINK, SUN, TANGERINE, VIOLET, hash, lag, noise, ramp, real};
use crate::paint::{Ink, Palette, Rgb};
use crate::recipe::{Glyphs, Recipe};
use crate::rest::DRAG;
use crate::shimmer::Spell;
use crate::stage::{Canvas, HEARD, Shader, Stage, TICK_SECS};

pub(super) const RECIPE: Recipe = Recipe {
    name: "Hi-fi",
    blurb: "the caption dances to your music",
    credit: USER,
    needs: Glyphs::BASE,
    listens: true,
    summon: |seed| Box::new(HiFi::new(seed)),
};

/// Bands the meter splits sound into, evenly spaced in pitch from `LOW` to `HIGH` hertz.
const BANDS: usize = 40;
const LOW: f64 = 40.0;
const HIGH: f64 = 16_000.0;
/// Decibels by which the meter lifts each band over the band an octave below, level at 1 kHz:
/// music's power falls as its pitch rises, and the slope lights the treble as brightly as the bass.
const SLOPE: f64 = 3.0;
/// The meter's scale in decibels below its ceiling, and the quietest sound it shows, in decibels
/// of a full-scale sine.
const RANGE: f64 = 36.0;
const GATE: f64 = -72.0;
/// Decibels a second the ceiling sinks after the loudest sound.
const SINK: f64 = 4.0;
/// Seconds without sound before the meter plays the host's work instead.
const DEAF: f64 = 0.25;
/// Seconds a rising band takes to reach its level, and the share of the scale a falling band
/// drops in a second.
const RISE: f64 = 0.02;
const FALL: f64 = 1.8;
/// Seconds a peak holds before it falls, and the share of the scale it falls in a second.
const HOLD: f64 = 0.4;
const DROP: f64 = 0.8;
/// A held peak that reached `FLASH` sparks under its letter while it stands `SPARK` above its band.
const FLASH: f64 = 0.7;
const SPARK: f64 = 0.35;
/// The share of its height each band keeps at rest, as though the music were turned down.
const QUIET: f64 = 0.6;
/// Quanta a second that drive the host's work to the top of the scale.
const BRIGHT: f64 = 90.0;
/// The lamps' colors from the bottom of the scale to the top.
const LAMPS: [Rgb; 6] = [AZURE, VIOLET, MAGENTA, PINK, TANGERINE, SUN];
/// The mark's bars, two to a cell, each a column of up to four braille dots lit like a hi-fi's
/// LEDs: the dots each column lights from the bottom up, left and right.
const BARS: usize = 6;
const LEFT: [u32; 5] = [0, 0x40, 0x44, 0x46, 0x47];
const RIGHT: [u32; 5] = [0, 0x80, 0xa0, 0xb0, 0xb8];

struct HiFi {
    /// The Hann window over the sound the meter analyzes, and the transform's twiddle factors.
    window: Vec<f64>,
    twiddles: Vec<(f64, f64)>,
    /// Each band's bins, with the share of each bin's power the band takes, lifted by the slope.
    bins: Vec<Vec<(usize, f64)>>,
    /// Each band's height on the meter, out of 1, and its peak, held a moment before it falls.
    heights: [f64; BANDS],
    peaks: [f64; BANDS],
    holds: [f64; BANDS],
    /// The top of the meter's scale, in decibels of a full-scale sine.
    ceiling: f64,
    seed: i64,
}

impl HiFi {
    fn new(seed: u64) -> Self {
        let size = real(HEARD);
        let window = (0..HEARD)
            .map(|i| 0.5 - 0.5 * (TAU * real(i) / size).cos())
            .collect();
        let twiddles = (0..HEARD / 2)
            .map(|k| {
                let angle = -TAU * real(k) / size;
                (angle.cos(), angle.sin())
            })
            .collect();
        let spacing = f64::from(SAMPLE_RATE) / size;
        let edge = |band: usize| LOW * (HIGH / LOW).powf(real(band) / real(BANDS));
        let bins = (0..BANDS)
            .map(|band| {
                let (low, high) = (edge(band), edge(band + 1));
                let lift = 10f64.powf(SLOPE * ((low * high).sqrt() / 1_000.0).log2() / 10.0);
                (1..HEARD / 2)
                    .filter_map(|bin| {
                        let center = real(bin) * spacing;
                        let overlap =
                            (center + spacing / 2.0).min(high) - (center - spacing / 2.0).max(low);
                        (overlap > 0.0).then_some((bin, lift * overlap / spacing))
                    })
                    .collect()
            })
            .collect();
        Self {
            window,
            twiddles,
            bins,
            heights: [0.0; BANDS],
            peaks: [0.0; BANDS],
            holds: [0.0; BANDS],
            ceiling: GATE + RANGE,
            seed: seed.cast_signed(),
        }
    }

    /// The power in each bin of the latest sound below half the sample rate, out of a
    /// full-scale sine's.
    fn spectrum(&self, stage: &Stage) -> Vec<f64> {
        let mut real_part: Vec<f64> = stage
            .sound()
            .zip(&self.window)
            .map(|(sample, weight)| f64::from(sample) * weight)
            .collect();
        let mut imaginary = vec![0.0; HEARD];
        transform(&mut real_part, &mut imaginary, &self.twiddles);
        let full = (real(HEARD) / 4.0).powi(2);
        real_part
            .iter()
            .zip(&imaginary)
            .take(HEARD / 2)
            .map(|(re, im)| (re * re + im * im) / full)
            .collect()
    }

    /// The bands' heights on the meter's scale from the latest sound, out of 1, the ceiling sunk
    /// for `dt` seconds of the host's: none in silence.
    fn listen(&mut self, stage: &Stage, dt: f64) -> Option<[f64; BANDS]> {
        let power = self.spectrum(stage);
        let decibels: [f64; BANDS] = std::array::from_fn(|band| {
            let energy: f64 = self.bins[band]
                .iter()
                .map(|&(bin, share)| share * power[bin])
                .sum();
            10.0 * (energy + 1e-12).log10()
        });
        let loudest = decibels.into_iter().fold(f64::NEG_INFINITY, f64::max);
        if loudest < GATE {
            return None;
        }
        self.ceiling = loudest.max(self.ceiling - SINK * dt).max(GATE + RANGE);
        let floor = self.ceiling - RANGE;
        Some(decibels.map(|level| ((level - floor) / RANGE).clamp(0.0, 1.0)))
    }

    /// The host's work as a spectrum, for when nothing plays: hills that swell with the flow, and a
    /// band struck by each fresh quantum.
    fn imagine(&self, stage: &Stage) -> [f64; BANDS] {
        let t = stage.t;
        let drive = match stage.spell {
            Spell::Busy => (stage.flow(/*window*/ 0.5) / BRIGHT).clamp(0.0, 1.0),
            Spell::Wait => 0.18 + 0.08 * (TAU * t / 2.4).sin(),
            Spell::Idle => 0.08,
        };
        let mut heights: [f64; BANDS] = std::array::from_fn(|band| {
            drive * (0.25 + 0.6 * noise(0.22 * real(band) - 0.9 * t, self.seed))
        });
        if stage.busy() {
            for back in 0..u64::from(stage.fresh()).min(6) {
                let quantum = stage.quanta.wrapping_sub(back).cast_signed();
                let band = notch(hash(self.seed ^ quantum) * real(BANDS), BANDS);
                heights[band] = heights[band].max(0.5 + 0.45 * drive);
            }
        }
        heights
    }
}

impl Shader for HiFi {
    fn tick(&mut self, stage: &Stage) {
        // At rest a tick spans more of the host's time, which the meter keeps, and the music
        // plays as though turned down.
        let (dt, gain) = if stage.spell == Spell::Idle {
            (TICK_SECS * f64::from(DRAG), QUIET)
        } else {
            (TICK_SECS, 1.0)
        };
        let heard = if stage.unheard < DEAF {
            self.listen(stage, dt)
        } else {
            None
        };
        let targets = heard.unwrap_or_else(|| self.imagine(stage));
        for (band, target) in targets.into_iter().enumerate() {
            let target = gain * target;
            let height = &mut self.heights[band];
            *height = if target > *height {
                lag(*height, target, RISE)
            } else {
                (*height - FALL * dt).max(target)
            };
            if *height >= self.peaks[band] {
                self.peaks[band] = *height;
                self.holds[band] = HOLD;
            } else if self.holds[band] > 0.0 {
                self.holds[band] -= dt;
            } else {
                self.peaks[band] = (self.peaks[band] - DROP * dt).max(*height);
            }
        }
    }

    fn status(&self, stage: &Stage, palette: Palette, canvas: &mut Canvas) {
        // at rest the lamps go grey
        let resting = stage.spell == Spell::Idle;
        let hue = |height: f64| {
            if resting {
                palette.fg
            } else {
                ramp(&LAMPS, height)
            }
        };
        let bar = |bar: usize| {
            let span = &self.heights[bar * BANDS / BARS..(bar + 1) * BANDS / BARS];
            let height = span.iter().sum::<f64>() / real(span.len());
            ((height - 0.2) / 0.7).clamp(0.0, 1.0)
        };
        canvas.mark = (0..BARS / 2)
            .map(|cell| {
                let (left, right) = (bar(2 * cell), bar(2 * cell + 1));
                let dots = |level: f64| notch(1.5 + 3.0 * level, LEFT.len());
                let glyph = char::from_u32(0x2800 + LEFT[dots(left)] + RIGHT[dots(right)]);
                let level = left.max(right);
                Ink::of(glyph.unwrap_or(' ')).fg(palette
                    .legible(hue(0.2 + 0.7 * level))
                    .blend(palette.grey(0.5), 0.4 + 0.6 * level))
            })
            .collect();
        let count = real(stage.caption.len().max(1));
        canvas.letters = stage.letters(|index, _| {
            let x = (real(index) + 0.5) / count;
            lamp(
                sample(&self.heights, x),
                sample(&self.peaks, x),
                hue,
                palette,
            )
        });
    }

    fn bullet(&self, _stage: &Stage, palette: Palette) -> Ink {
        let loudness = self.heights.iter().sum::<f64>() / real(BANDS);
        let glow = palette.legible(ramp(&LAMPS, loudness));
        Ink::of('♪').fg(glow.blend(palette.grey(0.5), 0.3 + 0.7 * loudness))
    }
}

/// A letter lit in `hue` to `height` on the meter, sparking beneath while its peak, held at
/// `peak`, stands well above it.
fn lamp(height: f64, peak: f64, hue: impl Fn(f64) -> Rgb, palette: Palette) -> Ink {
    let lit = hue(height);
    let cell = lit.blend(palette.bg, 0.06 + 0.74 * height);
    let fg = if height > 0.55 {
        palette.ink_on(cell)
    } else {
        palette.legible(lit).blend(palette.fg, 0.8 * height)
    };
    let ink = Ink::default().fg(fg).bg(cell).bold(height > 0.72);
    if peak > FLASH && peak > height + SPARK {
        ink.underline(palette.legible(hue(peak)))
    } else {
        ink
    }
}

/// `heights` at `x` across the bands, from 0 to 1, linear between the bands' centers.
fn sample(heights: &[f64; BANDS], x: f64) -> f64 {
    let at = (x * real(BANDS) - 0.5).clamp(0.0, real(BANDS - 1));
    let band = notch(at, BANDS);
    let next = (band + 1).min(BANDS - 1);
    heights[band] + (heights[next] - heights[band]) * (at - real(band))
}

/// The notch at `x` on a scale of `notches` counted from 0, clamped to the scale.
#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "A coordinate clamped to the scale floors to one of its notches."
)]
fn notch(x: f64, notches: usize) -> usize {
    x.clamp(0.0, real(notches - 1)) as usize
}

/// Fourier-transforms `re` and `im` in place, radix 2, with `twiddles` for their length.
fn transform(re: &mut [f64], im: &mut [f64], twiddles: &[(f64, f64)]) {
    let size = re.len();
    let mut mirror = 0;
    for slot in 1..size {
        let mut bit = size >> 1;
        while mirror & bit != 0 {
            mirror ^= bit;
            bit >>= 1;
        }
        mirror |= bit;
        if slot < mirror {
            re.swap(slot, mirror);
            im.swap(slot, mirror);
        }
    }
    let mut span = 2;
    while span <= size {
        let stride = size / span;
        for start in (0..size).step_by(span) {
            for offset in 0..span / 2 {
                let (wr, wi) = twiddles[offset * stride];
                let (top, bottom) = (start + offset, start + offset + span / 2);
                let (xr, xi) = (
                    re[bottom] * wr - im[bottom] * wi,
                    re[bottom] * wi + im[bottom] * wr,
                );
                re[bottom] = re[top] - xr;
                im[bottom] = im[top] - xi;
                re[top] += xr;
                im[top] += xi;
            }
        }
        span *= 2;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Motion, Shimmer, TICK};

    const PALETTE: Palette = Palette {
        fg: Rgb(208.0, 208.0, 208.0),
        bg: Rgb(21.0, 21.0, 21.0),
    };

    /// A pure tone lights the letters at its own pitch: the transform's bins, the bands and the
    /// letters agree on where each frequency falls along the caption.
    #[test]
    #[expect(
        clippy::cast_possible_truncation,
        reason = "Test tones are samples from −1 to 1."
    )]
    fn a_tone_lights_its_own_pitch() {
        let caption = "abcdefghijklmnopqrst";
        let per_tick = HEARD * 3 / 4;
        for hertz in [80.0, 1_000.0, 8_000.0] {
            let mut shimmer = Shimmer::new(&RECIPE, 0, Glyphs::BASE).unwrap();
            shimmer.set_spell(Spell::Busy);
            shimmer.show(caption);
            for tick in 0..40 {
                let tone: Vec<f32> = (0..per_tick)
                    .map(|i| {
                        let t = real(tick * per_tick + i) / f64::from(SAMPLE_RATE);
                        (0.5 * (TAU * hertz * t).sin()) as f32
                    })
                    .collect();
                shimmer.hear(&tone);
                shimmer.advance(TICK);
            }
            let row = shimmer.status(
                PALETTE,
                Motion {
                    mark: true,
                    letters: true,
                },
            );
            let lit: Vec<f64> = row
                .cells()
                .skip(4)
                .map(|(_, ink)| ink.bg.map_or(0.0, Rgb::luma))
                .collect();
            let brightest = (0..lit.len())
                .max_by(|&a, &b| lit[a].total_cmp(&lit[b]))
                .unwrap();
            let pitch = (hertz / LOW).ln() / (HIGH / LOW).ln() * real(caption.len());
            assert!(
                (real(brightest) + 0.5 - pitch).abs() <= 1.5,
                "{hertz} Hz lit letter {brightest}, not near {pitch:.1}"
            );
        }
    }
}
