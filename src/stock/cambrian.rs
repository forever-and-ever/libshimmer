//! Cambrian: three elementary cellular automata fight over the caption. Each live cell carries the
//! rule that bore it, and every window fires the rule of one of its live cells, drawn in proportion
//! to their counts, so territories spread, collide and fall. A rule whose output is 0 sometimes
//! misfires a mutant of another rule instead. Life starts in the right corner and restarts there
//! whenever the row dies out.

use super::USER;
use crate::kit::{CYAN, PINK, Rng, SUN};
use crate::paint::{Ink, Palette, Rgb};
use crate::recipe::{Glyphs, Recipe};
use crate::stage::{Canvas, Shader, Stage, TICK_SECS};

pub(super) const RECIPE: Recipe = Recipe {
    name: "Cambrian",
    blurb: "rival automata fight for the caption",
    credit: USER,
    needs: Glyphs::BASE,
    listens: false,
    summon: |seed| Box::new(Cambrian::new(seed)),
};

/// Quanta per generation, and the most generations a second.
const FARE: f64 = 20.0;
const RUSH: f64 = 4.0;
/// Seconds of paced flow the clock reads.
const WINDOW: f64 = 0.5;
/// Seconds a misfire's strike and a genesis glow.
const STRIKE: f64 = 0.3;
const GENESIS: f64 = 0.6;
/// The census in the tail: one bar per species, as tall as its share of the living.
const BARS: [char; 8] = ['▁', '▂', '▃', '▄', '▅', '▆', '▇', '█'];
/// The alpha of the grey the dead are drawn in.
const DEAD: f64 = 0.42;

/// What a species inherits: the elementary rule it fires, its color, and the chance that its 0
/// misfires into a mutant of another species.
struct Genome {
    rule: u8,
    color: Rgb,
    misfire: f64,
}

/// Rule 110's gliders, 126's nested triangles and 150's three-way parity. Rollouts at 7 to
/// 45 cells balanced the misfire rates so that each species holds the most cells a fifth to two
/// fifths of the time at every length, and a monoculture rarely outlasts a few hundred
/// generations. Rule 30 freezes into stripes against the right wall, so it is not among them.
const GENOMES: [Genome; 3] = [
    Genome {
        rule: 110,
        color: CYAN,
        misfire: 0.0112,
    },
    Genome {
        rule: 126,
        color: PINK,
        misfire: 0.0241,
    },
    Genome {
        rule: 150,
        color: SUN,
        misfire: 0.0010,
    },
];

// A rule must leave an empty window empty, and grow leftward from the seed in the right corner.
const _: () = {
    let mut index = 0;
    while index < GENOMES.len() {
        assert!(
            GENOMES[index].rule & 0b01 == 0,
            "a rule lights an empty window"
        );
        assert!(
            GENOMES[index].rule & 0b10 != 0,
            "a rule cannot grow from the corner"
        );
        index += 1;
    }
};

/// A species, by its genome's place in [`GENOMES`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Species(usize);

impl Species {
    fn random(rng: &mut Rng) -> Self {
        Self(rng.below(GENOMES.len()))
    }

    /// A species other than this one, uniformly.
    fn mutant(self, rng: &mut Rng) -> Self {
        let other = rng.below(GENOMES.len() - 1);
        Self(if other >= self.0 { other + 1 } else { other })
    }

    fn genome(self) -> &'static Genome {
        &GENOMES[self.0]
    }

    /// This species' color, dimmed while waiting freezes the row.
    fn ink(self, stage: &Stage, palette: Palette) -> Rgb {
        let lit = self.genome().color.blend(palette.fg, /*alpha*/ 0.65);
        if stage.busy() {
            lit
        } else {
            lit.blend(palette.grey(DEAD), /*alpha*/ 0.5)
        }
    }
}

/// The row: each cell dead or alive with the species that bore it. Both walls are dead.
#[derive(Default)]
struct World {
    cells: Vec<Option<Species>>,
}

/// What one generation did.
struct Generation {
    /// Cells born of misfires, with their species.
    mutants: Vec<(usize, Species)>,
    /// A birth drawn uniformly from the generation.
    sample: Option<Species>,
    /// The seed in the right corner, when the row died out.
    genesis: Option<Species>,
}

impl World {
    /// Fits the row to `width` cells flush against the right wall, where life began, and reseeds a
    /// dead row.
    fn fit(&mut self, width: usize, rng: &mut Rng) -> Option<Species> {
        let kept = self.cells.len().min(width);
        let mut cells = vec![None; width - kept];
        cells.extend_from_slice(&self.cells[self.cells.len() - kept..]);
        self.cells = cells;
        self.reseed(rng)
    }

    /// Lights the right corner with a random species when nothing lives.
    fn reseed(&mut self, rng: &mut Rng) -> Option<Species> {
        if self.cells.iter().any(Option::is_some) {
            return None;
        }
        let corner = self.cells.last_mut()?;
        let seed = Species::random(rng);
        *corner = Some(seed);
        Some(seed)
    }

    fn generate(&mut self, rng: &mut Rng) -> Generation {
        let cells = &self.cells;
        let mut mutants = Vec::new();
        let mut sample = None;
        let mut births = 0;
        let next = (0..cells.len())
            .map(|at| {
                let window = [
                    at.checked_sub(1).and_then(|left| cells[left]),
                    cells[at],
                    cells.get(at + 1).copied().flatten(),
                ];
                let live = window.iter().flatten().count();
                if live == 0 {
                    return None;
                }
                // Each live cell in the window is equally likely to fire its rule.
                let species = *window.iter().flatten().nth(rng.below(live))?;
                let pattern = window
                    .iter()
                    .fold(0, |pattern, cell| pattern << 1 | u8::from(cell.is_some()));
                let genome = species.genome();
                let born = if (genome.rule >> pattern) & 1 == 1 {
                    species
                } else if rng.chance(genome.misfire) {
                    let mutant = species.mutant(rng);
                    mutants.push((at, mutant));
                    mutant
                } else {
                    return None;
                };
                births += 1;
                if rng.below(births) == 0 {
                    sample = Some(born);
                }
                Some(born)
            })
            .collect();
        self.cells = next;
        Generation {
            mutants,
            sample,
            genesis: self.reseed(rng),
        }
    }
}

struct Cambrian {
    rng: Rng,
    world: World,
    /// The caption the world is fitted to, by [`Stage::captions`].
    captions: i64,
    /// Progress toward the next generation, in generations.
    phase: f64,
    /// The species on the mark: a birth drawn from the latest generation.
    sample: Option<Species>,
    /// The latest generation's misfires, and when they struck.
    strikes: Vec<(usize, Species)>,
    struck: f64,
    /// When the row last died out and reseeded its corner.
    genesis: f64,
}

impl Cambrian {
    fn new(seed: u64) -> Self {
        Self {
            rng: Rng::new(seed),
            world: World::default(),
            captions: 0,
            phase: 0.0,
            sample: None,
            strikes: Vec::new(),
            struck: f64::NEG_INFINITY,
            genesis: f64::NEG_INFINITY,
        }
    }

    fn sow(&mut self, seed: Species, t: f64) {
        self.sample = Some(seed);
        self.genesis = t;
    }
}

impl Shader for Cambrian {
    fn tick(&mut self, stage: &Stage) {
        if stage.captions != self.captions {
            self.captions = stage.captions;
            if let Some(seed) = self.world.fit(stage.caption.len(), &mut self.rng) {
                self.sow(seed, stage.t);
            }
        }
        if !stage.busy() {
            return;
        }
        self.phase = (stage.flow(WINDOW) / FARE)
            .min(RUSH)
            .mul_add(TICK_SECS, self.phase);
        if self.phase < 1.0 {
            return;
        }
        self.phase -= 1.0;
        let Generation {
            mutants,
            sample,
            genesis,
        } = self.world.generate(&mut self.rng);
        if !mutants.is_empty() {
            self.strikes = mutants;
            self.struck = stage.t;
        }
        match genesis {
            Some(seed) => self.sow(seed, stage.t),
            None => self.sample = sample.or(self.sample),
        }
    }

    fn status(&self, stage: &Stage, palette: Palette, canvas: &mut Canvas) {
        let t = stage.t;
        let dead = palette.grey(DEAD);
        let label = |species: Species| format!("{:03}", species.genome().rule);
        let strike = self
            .strikes
            .last()
            .filter(|_| t - self.struck < STRIKE)
            .map(|(_, mutant)| *mutant);
        let flare = strike.or(self.sample.filter(|_| t - self.genesis < GENESIS));
        canvas.mark = match (flare, self.sample) {
            // A misfire or a genesis inverts the mark in the newcomer's color.
            (Some(species), _) => label(species)
                .chars()
                .map(|glyph| {
                    Ink::of(glyph)
                        .fg(palette.bg)
                        .bg(species.ink(stage, palette))
                        .bold(true)
                })
                .collect(),
            (None, Some(species)) => label(species)
                .chars()
                .map(|glyph| Ink::of(glyph).fg(species.ink(stage, palette)).bold(true))
                .collect(),
            (None, None) => vec![Ink::of('·').fg(dead); 3],
        };
        let corner = self.world.cells.len().checked_sub(1);
        canvas.letters = stage.letters(|cell, _| {
            let Some(Some(species)) = self.world.cells.get(cell).copied() else {
                return Ink::default().fg(dead);
            };
            let mut paint = Ink::default().fg(species.ink(stage, palette));
            let struck = self
                .strikes
                .iter()
                .any(|(at, _)| *at == cell && t - self.struck < STRIKE);
            if struck {
                let fade = 1.0 - (t - self.struck) / STRIKE;
                paint = paint
                    .bg(species.ink(stage, palette).blend(palette.bg, 0.55 * fade))
                    .bold(true);
            } else if corner == Some(cell) && t - self.genesis < GENESIS {
                let fade = 1.0 - (t - self.genesis) / GENESIS;
                paint = paint
                    .bg(species.ink(stage, palette).blend(palette.bg, 0.6 * fade))
                    .bold(true);
            }
            paint
        });
        let mut census = [0; GENOMES.len()];
        for Species(index) in self.world.cells.iter().flatten() {
            census[*index] += 1;
        }
        let living: usize = census.iter().sum();
        canvas.tail = std::iter::once(Ink::of(' '))
            .chain(census.iter().enumerate().map(|(index, count)| {
                if *count == 0 {
                    return Ink::of('·').fg(dead);
                }
                let height = (count * BARS.len()).div_ceil(living);
                Ink::of(BARS[height - 1]).fg(Species(index).ink(stage, palette))
            }))
            .collect();
    }

    fn bullet(&self, stage: &Stage, palette: Palette) -> Ink {
        Ink::of('•').fg(self.sample.map_or_else(
            || palette.grey(/*alpha*/ 0.6),
            |species| species.ink(stage, palette),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Rollouts balanced the misfire rates against the rules' strengths; a changed rule or rate must
    /// keep every species in the running, holding the most cells at least an eighth of the time at
    /// every common caption length.
    #[test]
    fn every_species_takes_the_crown_at_every_length() {
        let mut rng = Rng::new(/*state*/ 0xca3b);
        for width in [7, 16, 27, 45] {
            let mut crowned = [0_u32; GENOMES.len()];
            for _ in 0..4 {
                let mut world = World::default();
                world.fit(width, &mut rng);
                let mut crown = 0;
                for _ in 0..12_000 {
                    world.generate(&mut rng);
                    let mut census = [0; GENOMES.len()];
                    for Species(index) in world.cells.iter().flatten() {
                        census[*index] += 1;
                    }
                    for (index, count) in census.iter().enumerate() {
                        if *count > census[crown] {
                            crown = index;
                        }
                    }
                    crowned[crown] += 1;
                }
            }
            let total: u32 = crowned.iter().sum();
            for (genome, reigns) in GENOMES.iter().zip(crowned) {
                assert!(
                    reigns * 8 >= total,
                    "rule {} starves at {width} cells: {crowned:?}",
                    genome.rule
                );
            }
        }
    }
}
