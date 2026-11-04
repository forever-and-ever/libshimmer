//! Forge: output stokes a forge, the caption glows from ember red to white heat, and waiting quenches it.

use std::f64::consts::TAU;

use super::OPUS;
use crate::kit::{GOLD, ICE, STEEL, SUN, TANGERINE, WHITE, chance, hash, lag, pick, ramp};
use crate::paint::{Ink, Palette, Rgb};
use crate::recipe::{Glyphs, Recipe};
use crate::stage::{Canvas, Shader, Stage};

pub(super) const RECIPE: Recipe = Recipe {
    name: "Forge",
    blurb: "output stokes a forge",
    credit: OPUS,
    needs: Glyphs::BASE,
    listens: false,
    summon: |_| Box::<Forge>::default(),
};

const BLACKBODY: [Rgb; 6] = [
    Rgb(40.0, 8.0, 8.0),
    Rgb(140.0, 22.0, 12.0),
    Rgb(225.0, 60.0, 20.0),
    TANGERINE,
    SUN,
    Rgb(255.0, 250.0, 225.0),
];
const SHADES: [char; 4] = ['░', '▒', '▓', '█'];
/// Quanta per gold ingot, and the seconds its glow lasts.
const INGOT_QUANTA: u64 = 1000;
const INGOT: f64 = 1.5;
/// Seconds the steam takes to clear off the quenched caption.
const QUENCH: f64 = 1.5;

#[derive(Default)]
struct Forge {
    /// A half-second running average of the quantum rate.
    heat: f64,
    /// The workpiece's temperature, trailing the heat by about four seconds.
    work: f64,
    /// When the last ingot was struck.
    struck: Option<f64>,
}

impl Shader for Forge {
    fn tick(&mut self, stage: &Stage) {
        let fire = if stage.busy() {
            stage.flow(/*window*/ 0.25) / 100.0
        } else {
            0.0
        };
        self.heat = lag(self.heat, fire, /*tau*/ 0.5);
        self.work = lag(self.work, self.heat, /*tau*/ 4.0);
        let before = stage.quanta - u64::from(stage.fresh());
        if stage.quanta / INGOT_QUANTA > before / INGOT_QUANTA {
            self.struck = Some(stage.t);
        }
    }

    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_possible_wrap,
        clippy::cast_precision_loss,
        clippy::cast_sign_loss,
        reason = "Clock buckets and nonnegative heat deliberately select integer noise/shade samples; caption coordinates are approximate visual scalars and allocated-slice indices fit i64."
    )]
    fn status(&self, stage: &Stage, palette: Palette, canvas: &mut Canvas) {
        let t = stage.t;
        let flicker = (t / 0.064).floor() as i64;
        canvas.tail = (0..3)
            .map(|j| {
                if stage.busy() && chance(3 * flicker + j, 3.0 * (self.heat - 0.85)) {
                    Ink::of(pick(5 * flicker + j, &['·', '*', '\'']))
                        .fg(pick(7 * flicker + j, &[SUN, WHITE, TANGERINE]))
                } else {
                    Ink::of(' ')
                }
            })
            .collect();
        if !stage.busy() {
            // Steam boils off the letters the moment waiting begins.
            let steam = if t > stage.spell_t {
                QUENCH - stage.spell_t
            } else {
                0.0
            };
            let banked = STEEL.blend(palette.grey(/*alpha*/ 0.6), /*alpha*/ 0.35);
            canvas.mark =
                vec![Ink::of('░').fg(ramp(&BLACKBODY, 0.2 + 0.1 * (TAU * t / 3.0).sin()))];
            canvas.letters = stage.letters(|i, _| {
                let i = i as i64;
                if QUENCH * hash(97 * i + stage.captions) < steam {
                    Ink::of(if (i + flicker) % 2 == 0 { '░' } else { '▒' }).fg(ICE)
                } else {
                    Ink::default().fg(banked)
                }
            });
            return;
        }
        if let Some(age) = self.struck.map(|at| t - at).filter(|age| *age < INGOT) {
            let tone = GOLD.blend(ramp(&BLACKBODY, /*x*/ 0.6), 1.0 - age / INGOT);
            canvas.mark = vec![Ink::of('■').fg(GOLD).bold(true)];
            canvas.letters = stage.letters(|_, _| Ink::default().fg(tone).bold(true));
            return;
        }
        let reach = stage.caption.len() as f64 + 6.0;
        let flare = (t / 0.2).floor() as i64;
        let shade = SHADES[((4.0 * self.heat).floor().max(0.0) as usize).min(SHADES.len() - 1)];
        canvas.mark = vec![
            Ink::of(shade)
                .fg(ramp(&BLACKBODY, 0.2 + 0.8 * self.heat))
                .bold(true),
        ];
        canvas.letters = stage.letters(|i, _| {
            let i = i as i64;
            let flared = if chance(131 * i + flare, /*odds*/ 0.04) {
                0.3
            } else {
                0.0
            };
            let glow =
                self.heat * (1.0 - i as f64 / reach) * (0.85 + 0.3 * hash(977 * i + flicker))
                    + flared;
            Ink::default()
                .fg(ramp(&BLACKBODY, 0.25 + 0.75 * glow))
                .bold(glow > 0.6)
        });
    }

    fn bullet(&self, _stage: &Stage, _palette: Palette) -> Ink {
        Ink::of('▪').fg(ramp(&BLACKBODY, 0.15 + self.work))
    }
}
