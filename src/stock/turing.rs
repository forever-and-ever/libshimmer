//! Turing: four blank-start machines rotate at work boundaries, never on repaint.

mod machine;

use crate::kit::Rng;

use crate::kit::AZURE;
use crate::kit::CYAN;
use crate::kit::GOLD;
use crate::kit::ICE;
use crate::kit::LIME;
use crate::kit::PINK;
use crate::kit::STEEL;
use crate::kit::VIOLET;
use crate::kit::WHITE;
use machine::Direction;
use machine::Machine;
use machine::State;

use super::DEEPSEEK;
use crate::paint::{Ink, Palette, Rgb};
use crate::recipe::{Glyphs, Recipe};
use crate::stage::{Canvas, Shader, Stage};

pub(super) const RECIPE: Recipe = Recipe {
    name: "Turing",
    blurb: "four machines take turns on the tape",
    credit: DEEPSEEK,
    needs: Glyphs::BASE,
    listens: false,
    summon: |seed| {
        let mut rng = Rng::new(seed);
        let machine = Machine::ALL[rng.below(Machine::ALL.len())];
        Box::new(Turing::new(machine, 0, rng))
    },
};

const TAPE: usize = 37;
/// Quanta between transitions; waiting quanta are discarded, not caught up later.
const STEP: u64 = 12;
const TONES: [Rgb; 5] = [CYAN, GOLD, PINK, LIME, VIOLET];

struct Turing {
    machine: Machine,
    rng: Rng,
    tape: [bool; TAPE],
    head: usize,
    /// No state means the selected machine reached its undefined (halt) instruction.
    state: Option<State>,
    direction: Direction,
    seen: u64,
}

impl Turing {
    fn new(machine: Machine, seen: u64, rng: Rng) -> Self {
        Self {
            machine,
            rng,
            tape: [false; TAPE],
            head: TAPE / 2,
            state: Some(State::A),
            direction: Direction::Right,
            seen,
        }
    }

    fn step(&mut self) {
        let Some(state) = self.state else {
            return;
        };
        let Some(instruction) = self.machine.instruction(state, self.tape[self.head]) else {
            self.state = None;
            return;
        };
        self.tape[self.head] = instruction.write;
        self.head = match instruction.direction {
            Direction::Right => (self.head + 1) % TAPE,
            Direction::Left => (self.head + TAPE - 1) % TAPE,
        };
        self.direction = instruction.direction;
        self.state = Some(instruction.next);
    }

    fn tone(&self) -> Rgb {
        self.state.map_or(STEEL, |state| TONES[state as usize])
    }

    fn motion(&self) -> char {
        self.state.map_or('■', |_| self.direction.glyph())
    }
}

impl Shader for Turing {
    fn idle(&mut self, stage: &Stage) {
        *self = Self::new(self.machine.next(), stage.quanta / STEP, self.rng.clone());
    }

    fn tick(&mut self, stage: &Stage) {
        let target = stage.quanta / STEP;
        if !stage.busy() {
            self.seen = target;
            return;
        }
        while self.seen < target {
            self.seen += 1;
            self.step();
        }
    }

    fn status(&self, stage: &Stage, palette: Palette, canvas: &mut Canvas) {
        let busy = stage.busy();
        let grey = palette.grey(/*alpha*/ 0.6);
        let tone = self.tone();
        canvas.mark = vec![
            Ink::of(self.state.map_or('H', State::glyph))
                .fg(tone)
                .bold(busy),
            Ink::of(if self.tape[self.head] { '1' } else { '0' }).fg(ICE),
            Ink::of(self.motion()).fg(tone),
        ];
        let width = stage.caption.len();
        if width == 0 {
            return;
        }
        let focus = self.head % width;
        canvas.letters = stage.letters(|i, _| {
            let bit = self.tape[(self.head + TAPE + i - focus) % TAPE];
            let here = i == focus && busy;
            let mut ink = Ink::default().fg(if !busy {
                grey
            } else if here {
                WHITE
            } else if bit {
                ICE
            } else {
                grey
            });
            if busy {
                if here {
                    ink = ink.bg(tone.blend(palette.bg, /*alpha*/ 0.5));
                } else if bit {
                    ink = ink.bg(AZURE.blend(palette.bg, /*alpha*/ 0.12));
                }
            }
            ink.bold(here)
        });
    }

    fn bullet(&self, stage: &Stage, palette: Palette) -> Ink {
        Ink::of(self.motion())
            .fg(if stage.busy() {
                self.tone()
            } else {
                palette.grey(/*alpha*/ 0.6)
            })
            .bold(stage.busy())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Spell;

    #[test]
    fn work_boundaries_rotate_without_replaying_waiting_quanta() {
        let mut stage = Stage::default();
        let palette = Palette { fg: ICE, bg: STEEL };
        let mut machine = Turing::new(Machine::Wolfram, 0, Rng::new(0));
        for expected in Machine::ALL {
            assert_eq!(machine.machine, expected);
            assert_eq!(machine.tape, [false; TAPE]);
            assert_eq!(machine.state, Some(State::A));
            assert_eq!(machine.head, TAPE / 2);
            stage.cast(Spell::Busy);
            stage.advance(120);
            machine.tick(&stage);
            let running = (machine.tape, machine.head, machine.state, machine.direction);
            for _ in 0..3 {
                stage.cast(Spell::Wait);
                stage.advance(120);
                machine.tick(&stage);
                machine.status(&stage, palette, &mut Canvas::default());
                assert_eq!(machine.machine, expected);
                assert_eq!(
                    (machine.tape, machine.head, machine.state, machine.direction),
                    running
                );
            }
            stage.cast(Spell::Busy);
            stage.advance(0);
            machine.tick(&stage);
            assert_eq!(
                (machine.tape, machine.head, machine.state, machine.direction),
                running
            );
            machine.idle(&stage);
            machine.tick(&stage);
        }
        assert_eq!(machine.machine, Machine::Wolfram);
        assert_eq!(machine.tape, [false; TAPE]);

        // The counter's undefined instruction remains a real halt, not a fifth invented rule.
        machine = Turing::new(Machine::Counter, 0, Rng::new(0));
        machine.state = Some(State::D);
        machine.tape[machine.head] = true;
        machine.step();
        assert_eq!(machine.state, None);
        machine.step();
        assert_eq!(machine.motion(), '■');
        machine.idle(&stage);
        assert_eq!(machine.state, Some(State::A));
    }
}
