//! What a shader reads each tick and paints each frame: the interface between a shimmer and its
//! animation.

use unicode_segmentation::UnicodeSegmentation;

use crate::TICK;
use crate::paint::{Ink, Palette, Rgb};
use crate::shimmer::{Intensity, Spell};

/// Seconds per tick.
pub(crate) const TICK_SECS: f64 = TICK.as_secs_f64();
/// Ticks of quanta a stage remembers, a little over four seconds.
const HISTORY: usize = 128;
/// Samples of sound a stage keeps, the window a listening shader analyzes: about 43 ms.
pub(crate) const HEARD: usize = 2048;

/// A recipe's animation, written like a GPU shader against the [`Stage`] its shimmer hands it.
/// Colors exist only at paint: [`Shader::tick`] sees no palette.
pub(crate) trait Shader: Send + Sync {
    /// Advances the shader's own state by one tick.
    fn tick(&mut self, _stage: &Stage) {}
    /// Ends a bout: the spell entered [`Spell::Idle`].
    fn idle(&mut self, _stage: &Stage) {}
    /// Paints the status line.
    fn status(&self, stage: &Stage, palette: Palette, canvas: &mut Canvas);
    /// Paints the bullet in front of a running sub-task, one cell wide.
    fn bullet(&self, stage: &Stage, palette: Palette) -> Ink;
}

/// The status line as a shader paints it. A glyph stands in for the text beneath it only when as
/// wide: a letter's width among the letters, one cell in the mark and the tail.
#[derive(Default)]
pub(crate) struct Canvas {
    /// The mark, one ink per cell; an ink without a glyph paints `•`.
    pub(crate) mark: Vec<Ink>,
    /// One ink per caption grapheme, which the shimmer fills with default inks before painting.
    pub(crate) letters: Vec<Ink>,
    /// Cells after the letters, one ink each; an ink without a glyph paints a space.
    pub(crate) tail: Vec<Ink>,
    /// Background colors laid under the line from its first column, whatever text lands there.
    pub(crate) backdrop: Vec<Rgb>,
}

/// The caption, split into graphemes.
#[derive(Default)]
pub(crate) struct Caption {
    text: String,
    /// The byte offset at which each grapheme ends.
    ends: Vec<usize>,
}

impl Caption {
    fn new(text: &str) -> Self {
        Self {
            text: text.to_owned(),
            ends: text
                .grapheme_indices(/*is_extended*/ true)
                .map(|(at, letter)| at + letter.len())
                .collect(),
        }
    }

    /// The number of graphemes.
    pub(crate) fn len(&self) -> usize {
        self.ends.len()
    }

    /// Grapheme `i`.
    pub(crate) fn get(&self, i: usize) -> Option<&str> {
        let end = *self.ends.get(i)?;
        let start = i.checked_sub(1).map_or(0, |before| self.ends[before]);
        Some(&self.text[start..end])
    }

    /// The graphemes, in order.
    pub(crate) fn iter(&self) -> impl Iterator<Item = &str> {
        self.ends.iter().scan(0, |start, &end| {
            let letter = &self.text[*start..end];
            *start = end;
            Some(letter)
        })
    }
}

/// The uniforms of one tick.
pub(crate) struct Stage {
    /// Seconds of shimmer time.
    pub(crate) t: f64,
    pub(crate) spell: Spell,
    /// Seconds since the spell was cast.
    pub(crate) spell_t: f64,
    pub(crate) intensity: Intensity,
    /// Quanta counted so far.
    pub(crate) quanta: u64,
    /// Seconds since a quantum arrived.
    pub(crate) quiet: f64,
    pub(crate) caption: Caption,
    /// Seconds since this caption appeared.
    pub(crate) caption_t: f64,
    /// Captions seen so far, counting this one.
    pub(crate) captions: i64,
    /// Quanta counted in each recent tick; `newest` indexes the last.
    history: [u32; HISTORY],
    newest: usize,
    /// Seconds since the host last passed sound; infinite before it first does.
    pub(crate) unheard: f64,
    /// The latest `HEARD` samples of sound, a ring whose oldest sample `next` indexes, and whether
    /// any arrived since the last tick.
    sound: Vec<f32>,
    next: usize,
    heard: bool,
}

impl Default for Stage {
    fn default() -> Self {
        Self {
            t: 0.0,
            spell: Spell::Idle,
            spell_t: 0.0,
            intensity: Intensity::default(),
            quanta: 0,
            quiet: 0.0,
            caption: Caption::default(),
            caption_t: 0.0,
            captions: 0,
            history: [0; HISTORY],
            newest: 0,
            unheard: f64::INFINITY,
            sound: vec![0.0; HEARD],
            next: 0,
            heard: false,
        }
    }
}

impl Stage {
    pub(crate) fn busy(&self) -> bool {
        self.spell == Spell::Busy
    }

    /// Quanta counted in the last tick.
    pub(crate) fn fresh(&self) -> u32 {
        self.history[self.newest]
    }

    /// Quanta counted in the last `window` seconds, at most the stage's history.
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "A window selects whole ticks of history; negative windows select none and long ones all."
    )]
    pub(crate) fn quanta_in(&self, window: f64) -> f64 {
        let ticks = ((window / TICK_SECS).round() as usize).min(HISTORY);
        (0..ticks)
            .map(|back| f64::from(self.history[(self.newest + HISTORY - back) % HISTORY]))
            .sum()
    }

    /// Quanta a second over the last `window` seconds.
    pub(crate) fn flow(&self, window: f64) -> f64 {
        self.quanta_in(window) / window
    }

    /// One ink per caption grapheme, from its index and text.
    pub(crate) fn letters(&self, mut paint: impl FnMut(usize, &str) -> Ink) -> Vec<Ink> {
        self.caption
            .iter()
            .enumerate()
            .map(|(i, letter)| paint(i, letter))
            .collect()
    }

    /// Casts `spell`, restarting its age; whether it differs from the spell before.
    pub(crate) fn cast(&mut self, spell: Spell) -> bool {
        let changed = spell != self.spell;
        if changed {
            self.spell = spell;
            self.spell_t = 0.0;
        }
        changed
    }

    /// Shows `caption`, restarting its age when it changes.
    pub(crate) fn show(&mut self, caption: &str) {
        if self.caption.text != caption {
            self.caption = Caption::new(caption);
            self.caption_t = 0.0;
            self.captions += 1;
        }
    }

    /// Keeps the latest of `sound`, silencing samples that are not finite.
    pub(crate) fn hear(&mut self, sound: &[f32]) {
        for &sample in &sound[sound.len().saturating_sub(HEARD)..] {
            self.sound[self.next] = if sample.is_finite() {
                sample.clamp(-1.0, 1.0)
            } else {
                0.0
            };
            self.next = (self.next + 1) % HEARD;
        }
        self.heard |= !sound.is_empty();
    }

    /// The latest `HEARD` samples of sound, oldest first.
    pub(crate) fn sound(&self) -> impl Iterator<Item = f32> + '_ {
        self.sound[self.next..]
            .iter()
            .chain(&self.sound[..self.next])
            .copied()
    }

    /// Moves the stage one tick, in which `quanta` quanta arrived.
    pub(crate) fn advance(&mut self, quanta: u32) {
        self.t += TICK_SECS;
        self.spell_t += TICK_SECS;
        self.caption_t += TICK_SECS;
        self.quanta += u64::from(quanta);
        self.quiet = if quanta > 0 {
            0.0
        } else {
            self.quiet + TICK_SECS
        };
        self.newest = (self.newest + 1) % HISTORY;
        self.history[self.newest] = quanta;
        self.unheard = if self.heard {
            0.0
        } else {
            self.unheard + TICK_SECS
        };
        self.heard = false;
    }
}
