//! Quipu: four pendant cords tally the quantum inflow in base four.

use super::DEEPSEEK;
use crate::kit::{CORAL, CYAN, SUN, VIOLET, WHITE, bump, frac};
use crate::paint::{Ink, Palette, Rgb};
use crate::recipe::{Glyphs, Recipe};
use crate::stage::{Canvas, Shader, Stage};

pub(super) const RECIPE: Recipe = Recipe {
    name: "Quipu",
    blurb: "four knotted cords tally the output",
    credit: DEEPSEEK,
    needs: Glyphs::BASE,
    listens: false,
    summon: |_| Box::new(Quipu),
};

const KNOTS: [char; 4] = ['┆', '┼', '╫', '※'];
const CORDS: [Rgb; 4] = [SUN, CORAL, CYAN, VIOLET];

struct Quipu;

impl Shader for Quipu {
    #[expect(
        clippy::cast_precision_loss,
        reason = "The reading knot and caption positions are approximate visual coordinates; exact quantum accounting remains upstream."
    )]
    fn status(&self, stage: &Stage, palette: Palette, canvas: &mut Canvas) {
        let busy = stage.busy();
        let grey = palette.grey(/*alpha*/ 0.6);
        let digits = [
            (stage.quanta / 64) % 4,
            (stage.quanta / 16) % 4,
            (stage.quanta / 4) % 4,
            stage.quanta % 4,
        ];
        canvas.mark = digits
            .iter()
            .enumerate()
            .map(|(i, &digit)| {
                let knot = usize::try_from(digit).unwrap_or_default();
                Ink::of(KNOTS[knot])
                    .fg(if busy { CORDS[i] } else { grey })
                    .bold(busy && digit > 0)
            })
            .collect();
        let width = f64::from(u32::try_from(stage.caption.len()).unwrap_or(u32::MAX));
        let read = frac(stage.quanta as f64 / 128.0) * (width + 3.0) - 1.5;
        canvas.letters = stage.letters(|i, _| {
            let cord = CORDS[i % 4];
            let near = if busy {
                bump(
                    f64::from(u32::try_from(i).unwrap_or(u32::MAX)) + 0.5 - read,
                    /*width*/ 1.5,
                )
            } else {
                0.0
            };
            let mut ink = Ink::default()
                .fg(cord.blend(if near > 0.5 { WHITE } else { grey }, 0.3 + 0.55 * near))
                .bold(busy && near > 0.55);
            if busy {
                ink = ink.bg(cord.blend(palette.bg, /*alpha*/ 0.04 + 0.16 * near));
            }
            ink
        });
    }

    fn bullet(&self, stage: &Stage, palette: Palette) -> Ink {
        Ink::of('┆')
            .fg(if stage.busy() {
                CORDS[usize::try_from(stage.quanta % 4).unwrap_or_default()]
            } else {
                palette.grey(/*alpha*/ 0.6)
            })
            .bold(stage.busy())
    }
}
