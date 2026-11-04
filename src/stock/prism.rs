//! Prism: a narrow band of the spectrum across the caption, fanned a little wider by fast output.

use super::OPUS;
use crate::kit::{bump, frac, hsl, lag, smooth};
use crate::paint::{Ink, Palette};
use crate::recipe::{Glyphs, Recipe};
use crate::stage::{Canvas, Shader, Stage};

pub(super) const RECIPE: Recipe = Recipe {
    name: "Prism",
    blurb: "a narrow band of the spectrum",
    credit: OPUS,
    needs: Glyphs::BASE,
    listens: false,
    summon: |_| Box::<Prism>::default(),
};

/// Quanta a second that light the whole caption and fan the band widest.
const BRIGHTEST: f64 = 90.0;
/// Seconds a waiting ray takes to cross the caption.
const RAY: f64 = 9.0;

#[derive(Default)]
struct Prism {
    /// The quantum rate, smoothed over about 0.6 seconds.
    flow: f64,
}

impl Prism {
    #[expect(
        clippy::cast_precision_loss,
        reason = "Quantum totals only shift an approximate visual hue; exact accounting remains upstream."
    )]
    fn hue(stage: &Stage) -> f64 {
        0.03 * stage.t + stage.quanta as f64 / 3000.0
    }
}

impl Shader for Prism {
    fn tick(&mut self, stage: &Stage) {
        let flow = if stage.busy() {
            stage.flow(/*window*/ 0.5)
        } else {
            0.0
        };
        self.flow = lag(self.flow, flow, /*tau*/ 0.6);
    }

    #[expect(
        clippy::cast_precision_loss,
        reason = "Caption indices and lengths are approximate visual coordinates; subpixel rounding cannot change transcript content."
    )]
    fn status(&self, stage: &Stage, palette: Palette, canvas: &mut Canvas) {
        let hue = Prism::hue(stage);
        let width = stage.caption.len() as f64;
        let resting = palette.grey(/*alpha*/ 0.6);
        if !stage.busy() {
            let ray = frac(stage.t / RAY) * (width + 4.0) - 2.0;
            let faint = hsl(hue, /*saturation*/ 0.45, /*lightness*/ 0.62);
            canvas.mark = vec![Ink::of('◭').fg(palette.grey(/*alpha*/ 0.5)).bold(true)];
            canvas.letters = stage.letters(|i, _| {
                Ink::default().fg(faint.blend(resting, bump(i as f64 + 0.5 - ray, /*width*/ 2.0)))
            });
            return;
        }
        let dispersion = (self.flow / BRIGHTEST).clamp(0.0, 1.0);
        let reach = 3.0 + dispersion * (width - 3.0);
        let band = 0.04 + 0.12 * dispersion;
        canvas.mark = vec![
            Ink::of('◭')
                .fg(hsl(hue, /*saturation*/ 0.35, /*lightness*/ 0.75))
                .bold(true),
        ];
        canvas.letters = stage.letters(|i, _| {
            let x = (i as f64 + 0.5) / reach;
            let lit = 1.0 - smooth(/*from*/ 0.85, /*to*/ 1.15, x);
            let ray = hsl(
                hue + band * (x - 0.5),
                /*saturation*/ 0.5,
                /*lightness*/ 0.64,
            );
            Ink::default().fg(ray.blend(resting, lit)).bold(lit > 0.5)
        });
    }

    fn bullet(&self, stage: &Stage, palette: Palette) -> Ink {
        Ink::of('·')
            .fg(palette.grey(if stage.busy() { 0.85 } else { 0.4 }))
            .bold(true)
    }
}
