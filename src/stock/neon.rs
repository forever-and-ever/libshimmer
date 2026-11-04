//! Neon: the caption is a sign of pink and cyan tubes; one letter is always on its way out.

use super::OPUS;
use crate::kit::{CYAN, Lottery, PINK, VIOLET, WHITE, chance, frac, hash};
use crate::paint::{Ink, Palette};
use crate::recipe::{Glyphs, Recipe};
use crate::stage::{Canvas, Shader, Stage, TICK_SECS};

pub(super) const RECIPE: Recipe = Recipe {
    name: "Neon",
    blurb: "a neon sign with a failing tube",
    credit: OPUS,
    needs: Glyphs::BASE,
    listens: false,
    summon: |_| Box::new(Neon),
};

/// Seconds each faulty letter lasts, and the share of them it spends dark at the end.
const FAULT: f64 = 6.0;
const DARK: f64 = 0.93;
/// A power surge flares the sign, which then ignites again.
const SURGE: Lottery = Lottery {
    every: 15.0,
    odds: 0.33,
    span: 3.0,
    seed: 41,
};

struct Neon;

impl Shader for Neon {
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        clippy::cast_precision_loss,
        clippy::cast_possible_wrap,
        reason = "Neon noise deliberately samples whole clock buckets and nonnegative hash-selected letters; caption coordinates are visual scalars and allocated-slice indices fit i64."
    )]
    fn status(&self, stage: &Stage, palette: Palette, canvas: &mut Canvas) {
        let t = stage.t;
        let flicker = (t / TICK_SECS).round() as i64;
        let fault = frac(t / FAULT);
        let faulty =
            (hash(13 * (t / FAULT).floor() as i64 + 1) * stage.caption.len() as f64) as usize;
        let warmth = SURGE.at(t).map_or(stage.caption_t, |surge| {
            stage.caption_t.min(3.0 * surge - 0.5)
        });
        canvas.mark = vec![
            Ink::of('◉')
                .fg(if stage.busy() { CYAN } else { VIOLET })
                .bold(true),
        ];
        let mut word = 0;
        canvas.letters = stage.letters(|i, letter| {
            if letter == " " {
                word += 1;
                return Ink::default();
            }
            let tube = match (stage.busy(), word % 2) {
                (false, _) => VIOLET,
                (true, 0) => PINK,
                (true, _) => CYAN,
            };
            let index = i as i64;
            if warmth < 0.0 {
                return Ink::default()
                    .fg(WHITE)
                    .bg(WHITE.blend(palette.bg, /*alpha*/ 0.2))
                    .bold(true);
            }
            let ignites = 0.06 * i as f64 + 0.1 * hash(7 * stage.captions + index);
            let lit = warmth > ignites + 0.25
                || warmth > ignites && chance(13 * index + flicker, /*odds*/ 0.5);
            let failing = i == faulty
                && (fault >= DARK || chance(7 * index + 3 * flicker, 0.8 * fault * fault));
            if !lit || failing {
                return Ink::default().fg(tube.blend(palette.bg, /*alpha*/ 0.22));
            }
            if stage.busy() {
                Ink::default()
                    .fg(WHITE.blend(tube, /*alpha*/ 0.2))
                    .bg(tube.blend(palette.bg, /*alpha*/ 0.2))
                    .bold(true)
            } else {
                Ink::default()
                    .fg(tube.blend(palette.bg, /*alpha*/ 0.7))
                    .bg(tube.blend(palette.bg, /*alpha*/ 0.08))
            }
        });
    }

    fn bullet(&self, _stage: &Stage, palette: Palette) -> Ink {
        Ink::of('◦').fg(PINK.blend(palette.bg, /*alpha*/ 0.6))
    }
}
