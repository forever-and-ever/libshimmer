//! LFSR: the caption is a Fibonacci shift register that the quantum flow clocks rightward. Each caption
//! draws a random sparse primitive polynomial over GF(2), whose taps underline their letters. The
//! mark is the XOR gate: after each shift it flashes every tap holding a 1, inverting as the
//! feedback's parity flips, and it pays out as more taps hit.

use std::f64::consts::TAU;
use std::ops::Range;
use std::ops::RangeInclusive;

use unicode_width::UnicodeWidthStr;

use super::USER;
use crate::kit::{CORAL, CYAN, GOLD, LIME, MAGENTA, PINK, Rng, STEEL, SUN, TANGERINE, VIOLET, hsl};
use crate::paint::{Ink, Palette, Rgb};
use crate::recipe::{Glyphs, Recipe};
use crate::stage::{Canvas, Shader, Stage, TICK_SECS};

pub(super) const RECIPE: Recipe = Recipe {
    name: "LFSR",
    blurb: "a primitive polynomial shifts text",
    credit: USER,
    needs: Glyphs::BASE,
    listens: false,
    summon: |seed| Box::new(Lfsr::new(seed)),
};

/// The longest register, whose polynomial arithmetic fits a `u64`. Graphemes past it, like the
/// tail's cells, show bits already shifted out.
const DEGREES: usize = 64;
/// Quanta per shift, and the most shifts a second before the travel reads as static.
const FARE: f64 = 20.0;
const RUSH: f64 = 4.0;
/// Seconds of paced flow the clock reads.
const WINDOW: f64 = 0.5;
/// Seconds each hit holds the mark, and the least the settled feedback holds it before the shift.
const FLASH: f64 = 3.0 * TICK_SECS;
const HOLD: f64 = 2.0 * TICK_SECS;
/// Seconds a new polynomial takes to wire its taps, left to right.
const INTRO: f64 = 0.35;
/// Seconds a shifted run's head flares and its wake fades.
const STREAK: f64 = 0.16;
/// The opacity of each cell past the caption, where bits fall off the register.
const TAIL: [f64; 4] = [0.8, 0.55, 0.35, 0.18];
/// The mark's color for each hit of a pass: two are routine, then the machine heats.
const HEAT: [Rgb; 6] = [CYAN, CYAN, SUN, PINK, MAGENTA, CORAL];
/// Spark colors, seconds aloft, and cells a second per rank.
const SPARKS: [Rgb; 5] = [SUN, TANGERINE, PINK, LIME, CYAN];
const SPARK: f64 = 0.45;
const DRIFT: f64 = 18.0;
/// Degrees whose cycle of 2^n − 1 shifts comes home often enough to see, and fits the mark.
const HOMING: RangeInclusive<u32> = 5..=9;
/// Seconds the homecoming wave takes to cross the caption, and the cells it turns into their bits.
const HOMECOMING: f64 = 0.9;
const WAKE: usize = 3;

/// The primes modulo which 2 has order d, at index d − 1: the primitive prime divisors of 2^d − 1.
/// Those of 2^n − 1 are the ones listed under the divisors of n.
const ZSIGMONDY: [&[u64]; DEGREES] = [
    &[],
    &[3],
    &[7],
    &[5],
    &[31],
    &[],
    &[127],
    &[17],
    &[73],
    &[11],
    &[23, 89],
    &[13],
    &[8_191],
    &[43],
    &[151],
    &[257],
    &[131_071],
    &[19],
    &[524_287],
    &[41],
    &[337],
    &[683],
    &[47, 178_481],
    &[241],
    &[601, 1_801],
    &[2_731],
    &[262_657],
    &[29, 113],
    &[233, 1_103, 2_089],
    &[331],
    &[2_147_483_647],
    &[65_537],
    &[599_479],
    &[43_691],
    &[71, 122_921],
    &[37, 109],
    &[223, 616_318_177],
    &[174_763],
    &[79, 121_369],
    &[61_681],
    &[13_367, 164_511_353],
    &[5_419],
    &[431, 9_719, 2_099_863],
    &[397, 2_113],
    &[631, 23_311],
    &[2_796_203],
    &[2_351, 4_513, 13_264_529],
    &[97, 673],
    &[4_432_676_798_593],
    &[251, 4_051],
    &[103, 2_143, 11_119],
    &[53, 157, 1_613],
    &[6_361, 69_431, 20_394_401],
    &[87_211],
    &[881, 3_191, 201_961],
    &[15_790_321],
    &[32_377, 1_212_847],
    &[59, 3_033_169],
    &[179_951, 3_203_431_780_337],
    &[61, 1_321],
    &[2_305_843_009_213_693_951],
    &[715_827_883],
    &[92_737, 649_657],
    &[641, 6_700_417],
];

/// A primitive polynomial P over GF(2) wired into a Fibonacci register: bit j − 1 holds the
/// coefficient of x^j and taps the stage at cell j − 1. The constant term is implicit and the leading
/// term taps the last stage, so the highest bit is the degree; no taps wire an empty register.
#[derive(Clone, Copy, Default)]
struct Wiring {
    taps: u64,
}

impl Wiring {
    /// A polynomial of `degree`, uniform among the primitive ones with as many terms as its length
    /// warrants: trinomials while no pentanomial is primitive, pentanomials (four taps, a one-in-sixteen
    /// jackpot) for most captions, and heptanomials past 32 stages.
    fn draw(degree: usize, rng: &mut Rng) -> Self {
        let Some(top) = degree.checked_sub(1) else {
            return Self::default();
        };
        let middles = match degree {
            1 => 0,
            2..=4 => 1,
            5..=32 => 3,
            _ => 5,
        };
        loop {
            let wiring = Self {
                taps: rng
                    .sample(top, middles)
                    .into_iter()
                    .fold(1 << top, |taps, cell| taps | 1 << cell),
            };
            if wiring.primitive() {
                return wiring;
            }
        }
    }

    fn degree(self) -> u32 {
        u64::BITS - self.taps.leading_zeros()
    }

    /// The register's cells.
    fn mask(self) -> u64 {
        u64::MAX.checked_shr(u64::BITS - self.degree()).unwrap_or(0)
    }

    /// `a` times x modulo P, for `a` of lower degree: x^n is the sum of P's lower terms.
    fn times_x(self, a: u64) -> u64 {
        let shifted = (a << 1) & self.mask();
        if (a >> (self.degree() - 1)) & 1 == 1 {
            shifted ^ ((self.taps << 1 | 1) & self.mask())
        } else {
            shifted
        }
    }

    fn times(self, mut a: u64, mut b: u64) -> u64 {
        let mut product = 0;
        while b != 0 {
            if b & 1 == 1 {
                product ^= a;
            }
            a = self.times_x(a);
            b >>= 1;
        }
        product
    }

    fn power(self, mut base: u64, mut exponent: u64) -> u64 {
        let mut power = 1;
        while exponent != 0 {
            if exponent & 1 == 1 {
                power = self.times(power, base);
            }
            base = self.times(base, base);
            exponent >>= 1;
        }
        power
    }

    /// Whether x generates the multiplicative group of GF(2)[x]/(P), so that the register visits every
    /// nonzero state: x^(2^n) = x, and x^((2^n − 1)/p) ≠ 1 for each prime p dividing 2^n − 1. A unit of
    /// order 2^n − 1 leaves no room for zero divisors, so P is irreducible as well.
    fn primitive(self) -> bool {
        let degree = self.degree();
        let x = self.times_x(1);
        (0..degree).fold(x, |power, _| self.times(power, power)) == x
            && (1..=degree)
                .filter(|order| degree.is_multiple_of(*order))
                .flat_map(|order| ZSIGMONDY[order as usize - 1])
                .all(|prime| self.power(x, self.mask() / prime) != 1)
    }
}

struct Lfsr {
    rng: Rng,
    /// Recent bits, newest first: bit i lights cell i, the caption's graphemes and then the tail's.
    /// The register is the low `wiring.degree()` bits; older bits have been shifted out.
    trail: u128,
    wiring: Wiring,
    /// The caption the wiring serves, by [`Stage::captions`], and when it was drawn.
    captions: i64,
    wired: f64,
    /// The register as wired, to which it returns every 2^n − 1 shifts, and when it last did.
    home: u64,
    homed: f64,
    /// Progress toward the next shift, in shifts, and when the last one happened.
    phase: f64,
    shifted: f64,
    /// The gate's pass over the register as it stands, and the pass before, whose sparks still fly.
    pass: Pass,
    last: Pass,
}

impl Lfsr {
    fn new(seed: u64) -> Self {
        let mut rng = Rng::new(seed);
        let trail = u128::from(rng.bits()) | (u128::from(rng.bits()) << 64);
        Self {
            rng,
            trail,
            wiring: Wiring::default(),
            captions: 0,
            wired: f64::NEG_INFINITY,
            home: 0,
            homed: f64::NEG_INFINITY,
            phase: 0.0,
            shifted: f64::NEG_INFINITY,
            pass: Pass::default(),
            last: Pass::default(),
        }
    }

    /// The register: the trail's newest bits.
    #[expect(
        clippy::cast_possible_truncation,
        reason = "The register is at most 64 bits, all of which the truncated trail keeps."
    )]
    fn state(&self) -> u64 {
        self.trail as u64 & self.wiring.mask()
    }

    fn bit(&self, cell: usize) -> bool {
        holds(self.trail, cell)
    }

    fn tapped(&self, cell: usize) -> bool {
        holds(u128::from(self.wiring.taps), cell)
    }

    /// Whether four or more taps all hit. Taps come in even numbers, so the jackpot always feeds
    /// back 0: the house wins.
    fn jackpot(&self) -> bool {
        self.wiring.taps.count_ones() >= 4 && self.pass.hits == self.wiring.taps
    }

    /// Draws a polynomial for the caption on stage and starts the gate on it once its taps wire up.
    fn rewire(&mut self, stage: &Stage) {
        self.captions = stage.captions;
        self.wiring = Wiring::draw(stage.caption.len().min(DEGREES), &mut self.rng);
        // An all-zero register never leaves zero; light its first stage.
        if self.state() == 0 {
            self.trail |= u128::from(self.wiring.mask() & 1);
        }
        self.home = self.state();
        self.homed = f64::NEG_INFINITY;
        self.wired = stage.t;
        self.pass = Pass {
            hits: self.home & self.wiring.taps,
            at: stage.t + INTRO,
        };
    }

    /// Latches the feedback into the first stage and starts the gate on the shifted register.
    fn shift(&mut self, t: f64) {
        self.trail = self.trail << 1 | u128::from(self.pass.feedback());
        self.phase = 0.0;
        self.shifted = t;
        self.last = self.pass;
        let state = self.state();
        self.pass = Pass {
            hits: state & self.wiring.taps,
            at: t,
        };
        if state == self.home && HOMING.contains(&self.wiring.degree()) {
            self.homed = t;
        }
    }
}

impl Shader for Lfsr {
    fn tick(&mut self, stage: &Stage) {
        if stage.captions != self.captions {
            self.rewire(stage);
        }
        if !stage.busy() {
            return;
        }
        let rate = (stage.flow(WINDOW) / FARE).min(RUSH);
        self.phase = rate.mul_add(TICK_SECS, self.phase).min(1.0);
        if self.phase >= 1.0 && stage.t >= self.pass.settled() {
            self.shift(stage.t);
        }
    }

    fn status(&self, stage: &Stage, palette: Palette, canvas: &mut Canvas) {
        let paint = Paint::new(self, stage, palette);
        canvas.mark = paint.mark();
        canvas.letters = stage.letters(|cell, letter| paint.letter(cell, letter));
        canvas.tail = TAIL
            .iter()
            .enumerate()
            .map(|(k, fade)| paint.tail(stage.caption.len() + k, *fade))
            .collect();
    }

    fn bullet(&self, stage: &Stage, palette: Palette) -> Ink {
        let lit = self
            .pass
            .flash(stage.t)
            .map_or_else(|| self.pass.feedback(), |(order, _)| order % 2 == 0);
        let glow = if stage.busy() {
            if lit { 1.0 } else { 0.0 }
        } else {
            0.5 + 0.5 * (TAU * stage.t / 2.4).sin()
        };
        Ink::of('⊕')
            .fg(CYAN
                .blend(palette.fg, /*alpha*/ 0.65)
                .blend(palette.grey(/*alpha*/ 0.5), glow))
            .bold(lit && stage.busy())
    }
}

/// The XOR gate folding the tapped stages into the next feedback bit, one hit at a time.
#[derive(Clone, Copy, Default)]
struct Pass {
    /// The tapped stages holding 1; each flips the feedback, left to right.
    hits: u64,
    /// When the pass began: at a shift, or as a new polynomial's taps finish wiring up.
    at: f64,
}

impl Pass {
    fn count(self) -> u32 {
        self.hits.count_ones()
    }

    fn feedback(self) -> bool {
        self.count() % 2 == 1
    }

    /// When the last flash has settled long enough for the register to shift.
    fn settled(self) -> f64 {
        f64::from(self.count()).mul_add(FLASH, self.at) + HOLD
    }

    /// The hit flashing at `t`, by its order in the pass, and its cell.
    fn flash(self, t: f64) -> Option<(u32, usize)> {
        let since = t - self.at;
        let order = (0..self.count()).find(|order| since < f64::from(order + 1) * FLASH)?;
        (since >= 0.0).then_some(())?;
        Some((order, cells(self.hits).nth(order as usize)?))
    }

    /// The sparks aloft at `t`: each hit past the second throws one more than the hit before.
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        clippy::cast_precision_loss,
        reason = "Hit orders and ranks are tiny; a spark's flight from a nonnegative age floors to whole cells."
    )]
    fn sparks(self, t: f64) -> impl Iterator<Item = Spark> {
        cells(self.hits)
            .enumerate()
            .skip(2)
            .flat_map(move |(order, cell)| {
                let age = (order as f64).mul_add(-FLASH, t - self.at);
                (1..order)
                    .filter(move |_| (0.0..SPARK).contains(&age))
                    .map(move |rank| Spark {
                        cell: cell + (DRIFT * rank as f64 * age) as usize,
                        color: SPARKS[(order + rank) % SPARKS.len()],
                        heat: 1.0 - age / SPARK,
                    })
            })
    }
}

/// A spark thrown rightward, with the register, by a hit past the second.
struct Spark {
    cell: usize,
    color: Rgb,
    /// What remains of its flight, from 1 down to 0.
    heat: f64,
}

/// One frame of the register, as `status` paints it.
struct Paint<'a> {
    lfsr: &'a Lfsr,
    stage: &'a Stage,
    palette: Palette,
    /// A 1's color, a 0's, and a tap's underline.
    one: Rgb,
    zero: Rgb,
    tap: Rgb,
    /// Cells whose taps have wired up.
    wired: usize,
    /// The hit flashing in the mark, by its order in the pass, and its cell.
    flash: Option<(u32, usize)>,
    /// Cells the homecoming wave turns into their bits.
    wave: Range<usize>,
    sparks: Vec<Spark>,
}

impl<'a> Paint<'a> {
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        clippy::cast_precision_loss,
        reason = "Elapsed fractions of the caption are floored to whole cells; an unwired register saturates."
    )]
    fn new(lfsr: &'a Lfsr, stage: &'a Stage, palette: Palette) -> Self {
        let t = stage.t;
        let zero = palette.grey(/*alpha*/ 0.42);
        let lit = CYAN.blend(palette.fg, /*alpha*/ 0.45);
        let wire = VIOLET.blend(palette.fg, /*alpha*/ 0.65);
        let (one, tap) = if stage.busy() {
            (lit, wire)
        } else {
            // Waiting freezes the register: its bits and taps dim.
            (
                lit.blend(zero, /*alpha*/ 0.5),
                wire.blend(zero, /*alpha*/ 0.5),
            )
        };
        let wired = ((t - lfsr.wired) / INTRO * f64::from(lfsr.wiring.degree())) as usize;
        let homing = t - lfsr.homed;
        let front = (homing / HOMECOMING * (stage.caption.len() + WAKE) as f64) as usize;
        Self {
            lfsr,
            stage,
            palette,
            one,
            zero,
            tap,
            wired,
            flash: lfsr.pass.flash(t),
            wave: if homing < HOMECOMING {
                front.saturating_sub(WAKE)..front
            } else {
                0..0
            },
            sparks: [lfsr.last, lfsr.pass]
                .into_iter()
                .flat_map(|pass| pass.sparks(t))
                .collect(),
        }
    }

    /// `accent` as text, pulled toward the terminal's foreground so that it reads on any theme.
    fn ink(&self, accent: Rgb) -> Rgb {
        accent.blend(self.palette.fg, /*alpha*/ 0.65)
    }

    fn paper(&self, accent: Rgb, alpha: f64) -> Rgb {
        accent.blend(self.palette.bg, alpha)
    }

    /// `glyph` in `accent`, inverted when the gate's parity is 1.
    fn parity(&self, glyph: char, accent: Rgb, odd: bool) -> Ink {
        if odd {
            Ink::of(glyph)
                .fg(self.palette.bg)
                .bg(self.ink(accent))
                .bold(true)
        } else {
            Ink::of(glyph)
                .fg(self.ink(accent))
                .bg(self.paper(accent, /*alpha*/ 0.15))
                .bold(true)
        }
    }

    fn mark(&self) -> Vec<Ink> {
        let Self { lfsr, stage, .. } = *self;
        let label = |text: String, accent: Rgb, odd: bool| {
            text.chars()
                .map(|glyph| self.parity(glyph, accent, odd))
                .collect()
        };
        if stage.t - lfsr.homed < HOMECOMING {
            // Home again after 2^n − 1 shifts.
            return label(format!("{:>3}", lfsr.wiring.mask()), GOLD, true);
        }
        if stage.t < lfsr.pass.at {
            // A new polynomial announces its degree while its taps wire up.
            return label(format!("x{:02}", lfsr.wiring.degree()), VIOLET, false);
        }
        if let Some((order, cell)) = self.flash {
            // The hit's term, inverted while an odd number of hits has accumulated.
            return label(format!("x{:02}", cell + 1), heat(order), order % 2 == 0);
        }
        let feedback = lfsr.pass.feedback();
        let accent = if !stage.busy() || lfsr.pass.hits == 0 {
            STEEL
        } else if lfsr.jackpot() {
            GOLD
        } else {
            CYAN
        };
        vec![
            Ink::of('⊕').fg(self.ink(accent)),
            Ink::of('▸').fg(self.ink(accent)),
            self.parity(if feedback { '1' } else { '0' }, accent, feedback),
        ]
    }

    fn letter(&self, cell: usize, letter: &str) -> Ink {
        let Self {
            lfsr,
            stage,
            palette,
            ..
        } = *self;
        let bit = lfsr.bit(cell);
        let mut fg = if bit { self.one } else { self.zero };
        let since = stage.t - lfsr.shifted;
        if since < STREAK {
            // The pattern moves right: each run's head flares and its wake fades.
            let glow = 1.0 - since / STREAK;
            match (bit, lfsr.bit(cell + 1)) {
                (true, false) => fg = palette.fg.blend(fg, 0.6 * glow),
                (false, true) => fg = self.one.blend(fg, glow),
                _ => {}
            }
        }
        let mut ink = Ink::default().fg(fg);
        let blank = letter.trim().is_empty();
        let tapped = cell < self.wired && lfsr.tapped(cell);
        if tapped {
            // Taps stay underlined in one color, so the eye can read the bits about to land on them.
            ink = ink.underline(self.tap);
            if blank {
                ink.glyph = Some('·');
            }
        }
        if let Some((order, _)) = self.flash.filter(|(_, at)| *at == cell) {
            ink = ink
                .fg(palette.fg)
                .bg(self.paper(heat(order), /*alpha*/ 0.45))
                .bold(true);
        } else if lfsr.jackpot() && self.flash.is_none() && stage.t >= lfsr.pass.at {
            // The house wins: every tap hit, and the feedback is 0.
            ink = ink.bg(self.paper(GOLD, /*alpha*/ 0.12));
        }
        if let Some(spark) = self.sparks.iter().find(|spark| spark.cell == cell) {
            let under = ink.bg.unwrap_or(palette.bg);
            ink = ink
                .bg(spark.color.blend(under, 0.4 * spark.heat))
                .bold(true);
        }
        if blank || letter.width() != 1 {
            return ink;
        }
        let last = self
            .flash
            .is_some_and(|(order, _)| order + 1 == lfsr.pass.count());
        if lfsr.jackpot() && last && tapped {
            // The jackpot's last hit lights every tap as a 1.
            ink = Ink {
                glyph: Some('1'),
                ..ink
                    .fg(self.ink(PINK))
                    .bg(self.paper(PINK, /*alpha*/ 0.35))
                    .bold(true)
            };
        }
        if self.wave.contains(&cell) {
            #[expect(
                clippy::cast_precision_loss,
                reason = "A caption's cells are few; the hue is a fraction of its length."
            )]
            let hue = cell as f64 / stage.caption.len().max(1) as f64;
            ink = Ink {
                glyph: Some(if bit { '1' } else { '0' }),
                ..ink.fg(self.ink(hsl(hue, 0.85, 0.6))).bold(true)
            };
        }
        ink
    }

    fn tail(&self, cell: usize, fade: f64) -> Ink {
        let Self { lfsr, palette, .. } = *self;
        if let Some(spark) = self.sparks.iter().find(|spark| spark.cell == cell) {
            let glyph = if spark.heat > 0.6 {
                '✦'
            } else if spark.heat > 0.3 {
                '*'
            } else {
                '·'
            };
            return Ink::of(glyph).fg(spark.color.blend(palette.bg, spark.heat));
        }
        let bit = lfsr.bit(cell);
        let color = if bit { self.one } else { self.zero };
        Ink::of(if bit { '•' } else { '·' }).fg(color.blend(palette.bg, fade))
    }
}

/// The mark's color for the hit of `order` in a pass.
fn heat(order: u32) -> Rgb {
    HEAT[(order as usize).min(HEAT.len() - 1)]
}

/// The cells set in `mask`, leftmost first.
fn cells(mut mask: u64) -> impl Iterator<Item = usize> {
    std::iter::from_fn(move || {
        let cell = mask.trailing_zeros() as usize;
        (mask != 0).then(|| {
            mask &= mask - 1;
            cell
        })
    })
}

fn holds(bits: u128, cell: usize) -> bool {
    u32::try_from(cell)
        .ok()
        .and_then(|cell| bits.checked_shr(cell))
        .is_some_and(|shifted| shifted & 1 == 1)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A non-primitive draw would trap the register in a short cycle that no one would notice: every
    /// drawn polynomial of a small degree must carry the register through all 2^n − 1 nonzero states.
    #[test]
    fn drawn_polynomials_cycle_through_every_nonzero_state() {
        let mut rng = Rng::new(/*state*/ 0x1f5a);
        for degree in 1..=16 {
            for _ in 0..4 {
                let wiring = Wiring::draw(degree, &mut rng);
                let mut state = 1;
                let mut period = 0;
                loop {
                    let feedback = u64::from((state & wiring.taps).count_ones() % 2);
                    state = (state << 1 | feedback) & wiring.mask();
                    period += 1;
                    if state == 1 {
                        break;
                    }
                }
                assert_eq!(period, wiring.mask(), "{:#b} cycles early", wiring.taps);
            }
        }
    }

    /// Past the reach of brute force, primitivity rests on this table listing every prime of 2^n − 1.
    #[test]
    fn zsigmondy_primes_factor_every_mersenne_number() {
        let mersenne = |exponent: usize| u64::MAX >> (DEGREES - exponent);
        for degree in 1..=DEGREES {
            let mut cofactor = mersenne(degree);
            for order in (1..=degree).filter(|order| degree.is_multiple_of(*order)) {
                for prime in ZSIGMONDY[order - 1] {
                    assert_eq!(mersenne(order) % prime, 0, "{prime} ∤ 2^{order} − 1");
                    while cofactor % prime == 0 {
                        cofactor /= prime;
                    }
                }
            }
            assert_eq!(cofactor, 1, "2^{degree} − 1 keeps an unlisted prime");
        }
    }
}
