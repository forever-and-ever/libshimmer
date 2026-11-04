//! Supernova: a star grows with the output, lighting the caption farther as it heats, until it blows.

use super::OPUS;
use crate::kit::{AZURE, CORAL, CYAN, ICE, SUN, TANGERINE, VIOLET, WHITE, chance, frac};
use crate::paint::{Ink, Palette, Rgb};
use crate::recipe::{Glyphs, Recipe};
use crate::stage::{Canvas, Shader, Stage};

pub(super) const RECIPE: Recipe = Recipe {
    name: "Supernova",
    blurb: "a star grows with output, then blows",
    credit: OPUS,
    needs: Glyphs::BASE,
    listens: false,
    summon: |_| Box::<Supernova>::default(),
};

const STELLAR: [Rgb; 7] = [CORAL, TANGERINE, SUN, ICE, CYAN, AZURE, VIOLET];
const STARS: [char; 7] = ['·', '✦', '✶', '✷', '✸', '✹', '✺'];
/// Quanta per stage of a star's life, when it goes nova, when the remnant settles, and when a new
/// star is born.
const STAGE: u64 = 40;
const NOVA: u64 = 280;
const REMNANT: u64 = 320;
const LIFE: u64 = 360;
/// Letters per second the shock front crosses, and seconds a letter takes to fade behind it.
const FRONT: f64 = 30.0;
const FADE: f64 = 0.7;

#[derive(Default)]
struct Supernova {
    /// When the current star went nova.
    nova: f64,
}

impl Shader for Supernova {
    fn tick(&mut self, stage: &Stage) {
        let crossings = |quanta: u64| (quanta + LIFE - NOVA) / LIFE;
        if crossings(stage.quanta) > crossings(stage.quanta - u64::from(stage.fresh())) {
            self.nova = stage.t;
        }
    }

    #[expect(
        clippy::cast_possible_wrap,
        clippy::cast_possible_truncation,
        clippy::cast_precision_loss,
        reason = "Star = quanta / 360 fits i64; visual noise deliberately uses clock buckets and caption coordinates are approximate scalars."
    )]
    fn status(&self, stage: &Stage, palette: Palette, canvas: &mut Canvas) {
        let t = stage.t;
        let life = stage.quanta % LIFE;
        let star = (stage.quanta / LIFE) as i64;
        let hyper = chance(31 * star + 7, /*odds*/ 0.25);
        if life < NOVA {
            let phase = (life / STAGE) as usize;
            // A waiting star twinkles: now and then it shows the stage before.
            let shown =
                if stage.busy() || !chance(7 * (t / 0.3).floor() as i64 + 1, /*odds*/ 0.3) {
                    phase
                } else {
                    phase.saturating_sub(1)
                };
            let tone = STELLAR[phase].blend(WHITE, /*alpha*/ 0.75);
            let reach = 3.0 + 4.0 * phase as f64;
            let heat = STELLAR[phase];
            canvas.mark = vec![
                Ink::of(STARS[shown])
                    .fg(if stage.busy() {
                        heat
                    } else {
                        heat.blend(palette.bg, /*alpha*/ 0.5)
                    })
                    .bold(true),
            ];
            let resting = palette.grey(/*alpha*/ 0.6);
            canvas.letters = stage.letters(|i, _| {
                Ink::default().fg(if stage.busy() {
                    tone.blend(palette.grey(/*alpha*/ 0.42), (-(i as f64) / reach).exp())
                } else {
                    resting
                })
            });
            return;
        }
        let age = t - self.nova;
        let front = if hyper { VIOLET } else { ICE }.blend(WHITE, /*alpha*/ 0.5);
        canvas.mark = vec![if age < 0.25 {
            Ink::of('✺')
                .fg(WHITE)
                .bg(WHITE.blend(palette.bg, /*alpha*/ 0.3))
                .bold(true)
        } else if life < REMNANT {
            Ink::of('✺').fg(if hyper { VIOLET } else { SUN }).bold(true)
        } else if hyper {
            Ink::of('●')
                .fg(palette.bg)
                .bg(VIOLET.blend(palette.bg, /*alpha*/ 0.7))
        } else {
            Ink::of(if (t / 0.128).floor() as i64 % 2 == 1 {
                '•'
            } else {
                '·'
            })
            .fg(CYAN)
            .bold(true)
        }];
        let unlit = VIOLET.blend(WHITE, /*alpha*/ 0.75);
        canvas.letters = stage.letters(|i, _| {
            let i = i as f64;
            let behind = age - i / FRONT;
            if behind < 0.0 {
                return Ink::default()
                    .fg(unlit.blend(palette.grey(/*alpha*/ 0.42), (-i / 27.0).exp()));
            }
            let ink = if behind < FADE {
                Ink::default()
                    .fg(front.blend(palette.grey(/*alpha*/ 0.5), 1.0 - behind / FADE))
                    .bold(behind < 0.3)
            } else {
                Ink::default().fg(palette.grey(/*alpha*/ 0.5))
            };
            if hyper && behind < 1.6 {
                ink.bg(VIOLET.blend(palette.bg, 0.25 * (1.0 - behind / 1.6)))
            } else {
                ink
            }
        });
    }

    fn bullet(&self, stage: &Stage, palette: Palette) -> Ink {
        Ink::of('✦').fg(if frac(stage.t / 1.7) < 0.5 {
            ICE.blend(palette.bg, /*alpha*/ 0.5)
        } else {
            ICE
        })
    }
}
