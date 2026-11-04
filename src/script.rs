//! The preview script: a canned bout and the rest after it, playing in a loop, with which a picker
//! previews a recipe.
//!
//! The bout is the design swatches' cycle. The host works under three captions, then waits on a
//! background terminal, whose output arrives as the wait ends and drains as a [`Reservoir`]
//! drains it. Then it rests, so that the preview shows the recipe between bouts too. A song plays
//! throughout, for a recipe that listens.
//!
//! [`Reservoir`]: crate::Reservoir

use std::f64::consts::TAU;
use std::time::Duration;

use crate::kit::hash;
use crate::reservoir::DRAIN;
use crate::shimmer::QUANTA;
use crate::{Intensity, SAMPLE_RATE, Spell};

/// What the host does during a segment, in units that count as quanta up to brisk work.
#[derive(Clone, Copy)]
enum Work {
    /// It produces `rate` units a second.
    Busy { rate: f64 },
    /// It waits on a terminal, whose output of `lump` units arrives as the wait ends.
    Wait { lump: f64 },
    /// It rests between bouts.
    Idle,
}

struct Segment {
    caption: &'static str,
    /// Seconds.
    length: f64,
    work: Work,
}

static CYCLE: [Segment; 5] = [
    Segment {
        caption: "Working",
        length: 2.5,
        work: Work::Busy { rate: 60.0 },
    },
    Segment {
        caption: "Tracing shimmer cadence",
        length: 3.5,
        work: Work::Busy { rate: 80.0 },
    },
    Segment {
        caption: "Drafting the fix",
        length: 4.0,
        work: Work::Busy { rate: 50.0 },
    },
    Segment {
        caption: "Waiting for background terminal",
        length: 7.0,
        work: Work::Wait { lump: 600.0 },
    },
    Segment {
        caption: "Finished",
        length: 3.0,
        work: Work::Idle,
    },
];

/// Seconds in one cycle.
fn length() -> f64 {
    CYCLE.iter().map(|segment| segment.length).sum()
}

/// The segment under way `t` seconds into a cycle.
fn segment(t: f64) -> &'static Segment {
    let mut end = 0.0;
    for segment in &CYCLE[..CYCLE.len() - 1] {
        end += segment.length;
        if t < end {
            return segment;
        }
    }
    &CYCLE[CYCLE.len() - 1]
}

/// Units a second arriving `t` seconds into a cycle: the host's own output while it works, and
/// every lump draining.
fn inflow(t: f64) -> f64 {
    let length = length();
    // A lump drains on through later cycles, so each cycle meets every earlier one's remainder.
    let echoes = 1.0 / -(-length / DRAIN).exp_m1();
    let mut end = 0.0;
    let mut drained = 0.0;
    for segment in &CYCLE {
        end += segment.length;
        if let Work::Wait { lump } = segment.work {
            drained += lump / DRAIN * (-(t - end).rem_euclid(length) / DRAIN).exp() * echoes;
        }
    }
    match segment(t).work {
        Work::Busy { rate } => rate + drained,
        Work::Wait { .. } | Work::Idle => drained,
    }
}

/// What the script commands at one moment.
#[derive(Clone, Copy, Debug, PartialEq)]
#[non_exhaustive]
pub struct Cue {
    /// The spell to cast.
    pub spell: Spell,
    /// The caption to show.
    pub caption: &'static str,
    /// The intensity to set.
    pub intensity: Intensity,
}

/// The cue `at` into the looping script.
pub fn cue(at: Duration) -> Cue {
    let t = at.as_secs_f64() % length();
    let segment = segment(t);
    Cue {
        spell: match segment.work {
            Work::Busy { .. } => Spell::Busy,
            Work::Wait { .. } => Spell::Wait,
            Work::Idle => Spell::Idle,
        },
        caption: segment.caption,
        intensity: Intensity::new(inflow(t) / QUANTA),
    }
}

/// Samples a beat of the song lasts: 96 beats a minute.
const BEAT: u64 = 30_000;
/// Samples before the song repeats: eight bars, as long as the script's cycle.
const SONG: u64 = 32 * BEAT;
/// Each bar's chord in hertz: the bass's root, then the triad.
const CHORDS: [[f64; 4]; 4] = [
    [55.0, 220.0, 261.63, 329.63],
    [43.65, 174.61, 220.0, 261.63],
    [65.41, 261.63, 329.63, 392.0],
    [49.0, 196.0, 246.94, 293.66],
];

/// The script's song from `from` to `to`, as mono samples at [`SAMPLE_RATE`]: what a picker passes
/// to [`Shimmer::hear`] to preview a recipe that listens. Four chords over drums and bass, looping
/// with the script.
///
/// [`Shimmer::hear`]: crate::Shimmer::hear
pub fn sound(from: Duration, to: Duration) -> Vec<f32> {
    let sample = |at: Duration| at.as_nanos() * u128::from(SAMPLE_RATE) / 1_000_000_000;
    (sample(from)..sample(to))
        .map(|n| song(u64::try_from(n % u128::from(SONG)).unwrap_or_default()))
        .collect()
}

/// Sample `n` of the song's loop.
#[expect(
    clippy::cast_possible_truncation,
    reason = "A sample, from −1 to 1, narrows to f32 without overflow."
)]
fn song(n: u64) -> f32 {
    let (beat, eighth, sixteenth) = (n / BEAT, n / (BEAT / 2), n / (BEAT / 4));
    let into = seconds(n % BEAT);
    let chord = CHORDS[usize::try_from(beat / 4 % 4).unwrap_or_default()];
    let t = seconds(n);
    // a kick on every beat, a sine falling from 135 Hz to 45
    let kick = 0.9
        * (-into / 0.15).exp()
        * (TAU * (45.0 * into + 3.6 * (1.0 - (-into / 0.04).exp()))).sin();
    // a snare on the backbeat, and hats on the eighths
    let snare = if beat % 2 == 1 {
        0.3 * (-into / 0.08).exp() * hiss(n)
    } else {
        0.0
    };
    let hat = 0.06 * (-seconds(n % (BEAT / 2)) / 0.02).exp() * (hiss(n) - hiss(n.wrapping_sub(1)));
    // the bass on the eighths, root then octave, under the chord held above it
    let root = chord[0] * if eighth % 2 == 0 { 1.0 } else { 2.0 };
    let bass = 0.35
        * (-seconds(n % (BEAT / 2)) / 0.25).exp()
        * ((TAU * root * t).sin() + 0.3 * (TAU * 2.0 * root * t).sin());
    let pad: f64 = chord[1..]
        .iter()
        .map(|hertz| 0.06 * (TAU * hertz * t).sin())
        .sum();
    // an arpeggio on the sixteenths, an octave up, plucked bright: a sawtooth's first six harmonics
    let note = 2.0 * chord[1 + usize::try_from(sixteenth % 3).unwrap_or_default()];
    let pluck: f64 = (1..=6)
        .map(|k| (TAU * f64::from(k) * note * t).sin() / f64::from(k))
        .sum();
    let arpeggio = 0.1 * (-seconds(n % (BEAT / 4)) / 0.09).exp() * pluck;
    (0.8 * (kick + snare + hat + bass + pad + arpeggio)).tanh() as f32
}

/// `samples` of the song, in seconds.
#[expect(
    clippy::cast_precision_loss,
    reason = "The song's samples number far below 2^53."
)]
fn seconds(samples: u64) -> f64 {
    samples as f64 / f64::from(SAMPLE_RATE)
}

/// White noise for sample `n`, from −1 to 1.
fn hiss(n: u64) -> f64 {
    2.0 * hash(n.cast_signed()) - 1.0
}
