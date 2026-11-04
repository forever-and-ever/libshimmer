//! A recipe's shader running on its own stage and clock, and the commands a host gives it.

use std::fmt;
use std::time::Duration;

use crate::paint::{Ink, Motion, Palette, Row};
use crate::recipe::{Glyphs, Recipe, Unfit};
use crate::stage::{Canvas, Shader, Stage, TICK_SECS};
use crate::{STALE, TICK};

/// Quanta a second at [`Intensity::BRISK`].
pub(crate) const QUANTA: f64 = 100.0;

/// What the host is doing.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Spell {
    /// Between bouts.
    #[default]
    Idle,
    /// Producing.
    Busy,
    /// Blocked on something external.
    Wait,
}

/// How hard the host works, measuring whatever the host chooses: at least 0, where
/// [`Intensity::BRISK`] is the most a stage shows linearly.
#[derive(Clone, Copy, Debug, Default, PartialEq, PartialOrd)]
pub struct Intensity(f64);

impl Intensity {
    /// Brisk work, which a shimmer counts as 100 quanta a second.
    pub const BRISK: Self = Self(1.0);

    /// `level`, saturated: NaN and negative levels read as zero, and infinity as the greatest
    /// finite level.
    pub fn new(level: f64) -> Self {
        Self(if level > 0.0 {
            level.min(f64::MAX)
        } else {
            0.0
        })
    }

    /// The level, finite and at least 0.
    pub fn level(self) -> f64 {
        self.0
    }

    /// Quanta a second: [`QUANTA`] for each unit up to brisk, and logarithmic beyond, so that a
    /// surge quickens the stage without swamping it.
    fn pace(self) -> f64 {
        QUANTA
            * if self.0 <= 1.0 {
                self.0
            } else {
                1.0 + self.0.ln()
            }
    }
}

/// One recipe's shader running on its own stage and clock: what a host holds.
pub struct Shimmer {
    recipe: &'static Recipe,
    shader: Box<dyn Shader>,
    stage: Stage,
    clock: Clock,
}

impl Shimmer {
    /// A shimmer of `recipe`, whose randomness `seed` determines, idle and with an empty caption.
    ///
    /// # Errors
    ///
    /// [`Unfit`] when the terminal lacks glyphs the recipe needs.
    pub fn new(recipe: &'static Recipe, seed: u64, glyphs: Glyphs) -> Result<Self, Unfit> {
        recipe.fits(glyphs)?;
        Ok(Self {
            recipe,
            shader: (recipe.summon)(seed),
            stage: Stage::default(),
            clock: Clock::default(),
        })
    }

    /// The recipe this shimmer runs.
    pub fn recipe(&self) -> &'static Recipe {
        self.recipe
    }

    /// Dresses `caption`, restarting its age when it changes.
    pub fn show(&mut self, caption: &str) {
        self.stage.show(caption);
    }

    /// Casts `spell`, restarting its age when it changes. Entering [`Spell::Idle`] ends the bout.
    pub fn set_spell(&mut self, spell: Spell) {
        if self.stage.cast(spell) && spell == Spell::Idle {
            self.shader.idle(&self.stage);
        }
    }

    /// Sets how hard the host works.
    pub fn set_intensity(&mut self, intensity: Intensity) {
        self.stage.intensity = intensity;
    }

    /// Hears `sound`, what the host heard since it last called: mono samples at [`SAMPLE_RATE`],
    /// nominally from −1 to 1, of which samples that are not finite are silent. A recipe that
    /// [listens](Recipe::listens) paints the latest, and the host's work once none arrive for a
    /// moment.
    ///
    /// [`SAMPLE_RATE`]: crate::SAMPLE_RATE
    pub fn hear(&mut self, sound: &[f32]) {
        if self.recipe.listens {
            self.stage.hear(sound);
        }
    }

    /// Runs the whole ticks that `dt` completes, carrying the remainder to the next call. A gap
    /// longer than [`STALE`] runs one tick and drops the remainders. Between bouts, shimmer time
    /// runs at a quarter of the host's.
    pub fn advance(&mut self, dt: Duration) {
        for _ in 0..self.clock.due(dt, self.stage.spell) {
            let quanta = self.clock.quanta(self.stage.intensity);
            self.stage.advance(quanta);
            self.shader.tick(&self.stage);
        }
    }

    /// Paints the status line's lead and backdrop. A still mark is absent, and still letters are
    /// the caption verbatim, without styling, tail, or backdrop.
    pub fn status(&self, palette: Palette, motion: Motion) -> Row {
        let mut canvas = Canvas {
            letters: vec![Ink::default(); self.stage.caption.len()],
            ..Canvas::default()
        };
        self.shader.status(&self.stage, palette, &mut canvas);
        crate::rest::settle(&mut canvas, &self.stage, palette);
        Row::paint(&self.stage.caption, canvas, motion)
    }

    /// Paints the bullet in front of a running sub-task, one cell wide.
    pub fn bullet(&self, palette: Palette) -> Ink {
        self.shader.bullet(&self.stage, palette)
    }
}

impl fmt::Debug for Shimmer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Shimmer")
            .field("recipe", &self.recipe.name)
            .field("spell", &self.stage.spell)
            .field("t", &self.stage.t)
            .finish_non_exhaustive()
    }
}

/// Converts the host's Δt into whole ticks, and intensity into whole quanta.
#[derive(Default)]
struct Clock {
    /// Shimmer time short of a whole tick, in nanoseconds of the host's at rest, `DRAG` to a
    /// nanosecond of shimmer time: every Δt adds whole units, so splitting it changes nothing (P3).
    owed: u128,
    /// Quanta short of a whole one.
    carry: f64,
}

impl Clock {
    /// The ticks due after `dt` of the host's time under `spell`.
    fn due(&mut self, dt: Duration, spell: Spell) -> u32 {
        if dt > STALE {
            *self = Self::default();
            return 1;
        }
        let drag = u128::from(crate::rest::DRAG);
        let tick = TICK.as_nanos() * drag;
        self.owed += dt.as_nanos() * if spell == Spell::Idle { 1 } else { drag };
        let mut due = 0;
        while self.owed >= tick {
            self.owed -= tick;
            due += 1;
        }
        due
    }

    /// The whole quanta that one tick at `intensity` completes.
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "The carry is finite and nonnegative, and logarithmic pacing holds one tick's quanta below 3000."
    )]
    fn quanta(&mut self, intensity: Intensity) -> u32 {
        self.carry += intensity.pace() * TICK_SECS;
        let whole = self.carry.floor();
        self.carry -= whole;
        whole as u32
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Spell::{Busy, Idle, Wait};
    use crate::paint::Rgb;

    const PALETTE: Palette = Palette {
        fg: Rgb(208.0, 208.0, 208.0),
        bg: Rgb(21.0, 21.0, 21.0),
    };

    /// Counts the bouts it was told ended, and shows the count as its bullet.
    #[derive(Default)]
    struct Recorder {
        ended: u32,
    }

    impl Shader for Recorder {
        fn idle(&mut self, _stage: &Stage) {
            self.ended += 1;
        }

        fn status(&self, _stage: &Stage, _palette: Palette, _canvas: &mut Canvas) {}

        fn bullet(&self, _stage: &Stage, _palette: Palette) -> Ink {
            Ink::of(char::from_digit(self.ended, 10).unwrap())
        }
    }

    static RECORDER: Recipe = Recipe {
        name: "Recorder",
        blurb: "",
        credit: "",
        needs: Glyphs::BASE,
        listens: false,
        summon: |_| Box::<Recorder>::default(),
    };

    #[test]
    fn each_entry_into_idle_ends_one_bout() {
        let mut shimmer: Shimmer = Shimmer::new(&RECORDER, 0, Glyphs::BASE).unwrap();
        let script = [
            (Idle, '0'),
            (Busy, '0'),
            (Wait, '0'),
            (Idle, '1'),
            (Idle, '1'),
            (Wait, '1'),
            (Busy, '1'),
            (Idle, '2'),
        ];
        for (spell, ended) in script {
            shimmer.set_spell(spell);
            shimmer.advance(STALE);
            assert_eq!(shimmer.bullet(PALETTE).glyph, Some(ended), "{spell:?}");
        }
    }

    #[test]
    fn a_stale_gap_runs_one_tick() {
        let mut clock = Clock::default();
        assert_eq!(clock.due(TICK / 2, Busy), 0);
        assert_eq!(clock.due(STALE + Duration::from_nanos(1), Busy), 1);
        assert_eq!(
            clock.due(TICK / 2, Busy),
            0,
            "a stale gap drops the time owed"
        );
        assert_eq!(clock.due(STALE, Busy), 31);
    }

    /// P3 at rest: shimmer time keeps every nanosecond of the host's.
    #[test]
    fn rest_keeps_every_nanosecond() {
        let mut clock = Clock::default();
        let ns = Duration::from_nanos;
        assert_eq!(clock.due(ns(1), Idle), 0);
        assert_eq!(
            clock.due((TICK * crate::rest::DRAG).checked_sub(ns(2)).unwrap(), Idle),
            0
        );
        assert_eq!(clock.due(ns(1), Idle), 1);
    }
}
