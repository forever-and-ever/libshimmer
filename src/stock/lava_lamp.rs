//! Lava Lamp: wax blobs drift behind the status line and merge, heaving with work and settling to
//! wait.

use std::f64::consts::TAU;

use super::OPUS;
use crate::kit::{Lottery, PINK, SUN, TANGERINE, VIOLET, hash, smooth};
use crate::paint::{Ink, Palette, Rgb};
use crate::recipe::{Glyphs, Recipe};
use crate::stage::{Canvas, Shader, Stage, TICK_SECS};

pub(super) const RECIPE: Recipe = Recipe {
    name: "Lava Lamp",
    blurb: "wax drifts behind the status line",
    credit: OPUS,
    needs: Glyphs::BASE,
    listens: false,
    summon: |_| Box::<LavaLamp>::default(),
};

const WAX: [Rgb; 3] = [PINK, TANGERINE, VIOLET];
/// Periods, in lamp seconds, of each blob's drift along the line, heave across it and swell.
const DRIFT: [f64; 3] = [13.1, 17.3, 23.9];
const HEAVE: [f64; 3] = [5.3, 7.1, 6.2];
const SWELL: [f64; 3] = [4.1, 5.7, 3.3];
/// Columns of the status line the lamp lights.
const GLASS: usize = 34;
const BUBBLE: Lottery = Lottery {
    every: 13.0,
    odds: 0.5,
    span: 1.5,
    seed: 2,
};

#[derive(Default)]
struct LavaLamp {
    /// The lamp's own clock, running a third as fast unless the host is busy.
    t: f64,
}

struct Blob {
    wax: Rgb,
    x: f64,
    y: f64,
    radius: f64,
}

impl Shader for LavaLamp {
    fn tick(&mut self, stage: &Stage) {
        self.t += if stage.busy() {
            TICK_SECS
        } else {
            0.35 * TICK_SECS
        };
    }

    #[expect(
        clippy::cast_precision_loss,
        clippy::cast_possible_truncation,
        reason = "Blob indices are below 3 and columns below GLASS = 34; bubble noise deliberately quantizes whole animation-clock buckets."
    )]
    fn status(&self, stage: &Stage, palette: Palette, canvas: &mut Canvas) {
        let t = self.t;
        let heat = (stage.quanta_in(/*window*/ 3.0) / 240.0).min(1.0);
        let wave = |period: f64, phase: f64| (TAU * t / period + phase).sin();
        let mut blobs: Vec<Blob> = WAX
            .iter()
            .enumerate()
            .map(|(j, &wax)| {
                let phase = j as f64;
                Blob {
                    wax,
                    x: 2.0 + 30.0 * (0.5 + 0.5 * wave(DRIFT[j], 2.0 * phase)),
                    y: 0.25 + heat * (0.8 * wave(HEAVE[j], phase) - 0.3),
                    radius: 2.6 + 0.8 * wave(SWELL[j], /*phase*/ 0.0),
                }
            })
            .collect();
        if stage.busy()
            && let Some(rise) = BUBBLE.at(stage.t)
        {
            blobs.push(Blob {
                wax: SUN,
                x: 4.0 + 26.0 * hash((stage.t / BUBBLE.every).floor() as i64),
                y: 1.0 - 2.0 * rise,
                radius: 1.1,
            });
        }
        let liquid = if stage.busy() {
            Rgb(90.0, 30.0, 120.0)
        } else {
            Rgb(25.0, 40.0, 120.0)
        }
        .blend(palette.bg, /*alpha*/ 0.15);
        canvas.backdrop = (0..GLASS)
            .map(|column| {
                let (mut field, mut sum) = (0.0, Rgb(0.0, 0.0, 0.0));
                for blob in &blobs {
                    let reach = blob.radius * blob.radius
                        / ((column as f64 + 0.5 - blob.x).powi(2) + (2.2 * blob.y).powi(2) + 0.3);
                    field += reach;
                    sum = Rgb(
                        sum.0 + reach * blob.wax.0,
                        sum.1 + reach * blob.wax.1,
                        sum.2 + reach * blob.wax.2,
                    );
                }
                let wax = Rgb(sum.0 / field, sum.1 / field, sum.2 / field)
                    .blend(palette.bg, /*alpha*/ 0.275);
                if field > 1.0 {
                    wax
                } else {
                    wax.blend(liquid, 0.5 * smooth(/*from*/ 0.5, /*to*/ 1.0, field))
                }
            })
            .collect();
        canvas.mark = vec![Ink::of('●').fg(palette.grey(/*alpha*/ 0.85))];
        if !stage.busy() {
            let resting = palette.grey(/*alpha*/ 0.6);
            canvas.letters = stage.letters(|_, _| Ink::default().fg(resting));
        }
    }

    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "The nonnegative lamp clock deliberately selects a whole seven-second bucket modulo the three wax colors."
    )]
    fn bullet(&self, _stage: &Stage, palette: Palette) -> Ink {
        let drop = WAX[(self.t / 7.0).floor() as usize % WAX.len()];
        Ink::of('•').fg(drop.blend(palette.grey(/*alpha*/ 0.6), /*alpha*/ 0.5))
    }
}
