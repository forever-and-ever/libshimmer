//! Fortress: a dwarf miner digs the caption, drawn as Dwarf Fortress draws its classic ASCII
//! fortresses. The caption is rock, tinted beneath its letters by the level's stone, and work
//! swings the pick: a dug letter fades to floor. A finished corridor ends in a down stair, and the
//! fortress persists from bout to bout, digging ever deeper through soil, stone, the caverns and
//! the magma sea to raw adamantine and the underworld beneath it. Warm or damp stone gives the
//! miner pause before he breaches magma or water, which floods the corridor; giant cave spiders,
//! fire imps and demons chase him toward the up stair, goblin sieges come down it while he digs
//! on, his job never cancelled, and a dwarf on fire flashes red and yellow. About once every
//! quarter hour of work the fun is fatal, and migrants take up the pick; the underworld ends the
//! fortress, and a new one embarks. While the host waits the miner has no job and shows `?`;
//! between bouts he sleeps, now and then a `Z`.

use std::iter;

use unicode_width::UnicodeWidthStr;

use super::USER;
use crate::Spell;
use crate::kit::{Lottery, Rng, frac, real};
use crate::paint::{BACKDROP, Ink, Palette, Rgb};
use crate::recipe::{Glyphs, Recipe};
use crate::rest::LULL;
use crate::stage::{Canvas, Shader, Stage, TICK_SECS};

pub(super) const RECIPE: Recipe = Recipe {
    name: "Fortress",
    blurb: "a dwarf digs deeper; losing is fun",
    credit: USER,
    needs: Glyphs::BASE,
    listens: false,
    summon: |seed| Box::new(Fortress::new(seed)),
};

/// Rock tiles the caption spans, whatever its length, and the tail's tiles after them.
const ROCK: usize = 32;
const TAIL: usize = 3;
const TILES: usize = ROCK + TAIL;
/// Where the up stair stands: the mark, a tile before the corridor.
const STAIR: f64 = -0.5;
/// Columns before the caption: the mark and the gap after it.
const LEAD: usize = 2;
/// Columns of the source's glow under the host's text past the tail.
const GLOW: usize = 10;

/// Dwarf Fortress's code page 437 tiles: a dwarf, the stairs, a deep liquid, remains, a web, a
/// fireball and a demon.
const DWARF: char = '☺';
const UP: char = '<';
const DOWN: char = '>';
const LIQUID: char = '≈';
const REMAINS: char = '%';
const WEB: char = '☼';
const FIREBALL: char = '*';
const DEMON: char = '&';
/// Status icons, which classic Dwarf Fortress alternates with a creature's tile: a migrant, a
/// dwarf without a job, a sleeper, and a blow landing left or right.
const MIGRANT: char = 'X';
const JOBLESS: char = '?';
const SLEEP: char = 'Z';
const LEFT: char = '←';
const RIGHT: char = '→';

/// Dwarf Fortress's light grey: miners, and giant cave spiders.
const GREY: Rgb = Rgb(192.0, 192.0, 192.0);
/// Dwarf Fortress's dark grey: every military profession's color, a goblin siege's too.
const MILITARY: Rgb = Rgb(128.0, 128.0, 128.0);
/// A burning creature flashes between these.
const FLAME: Rgb = Rgb(255.0, 72.0, 24.0);
const BLAZE: Rgb = Rgb(255.0, 226.0, 64.0);
/// Fire imps are yellow.
const AMBER: Rgb = Rgb(250.0, 216.0, 56.0);
const BLOOD: Rgb = Rgb(168.0, 12.0, 24.0);
const CHAR: Rgb = Rgb(92.0, 74.0, 66.0);
const WATER: Rgb = Rgb(36.0, 96.0, 232.0);
const MAGMA: Rgb = Rgb(255.0, 88.0, 12.0);
/// Warm stone before magma, and damp stone before water.
const WARM: Rgb = Rgb(214.0, 72.0, 32.0);
const DAMP: Rgb = Rgb(52.0, 112.0, 214.0);
/// Raw adamantine, and the underworld's eerie glow beneath it.
const ADAMANTINE: Rgb = Rgb(96.0, 240.0, 228.0);
const EERIE: Rgb = Rgb(196.0, 56.0, 216.0);
/// Loam, sandy clay and clay.
const SOILS: [Rgb; 3] = [
    Rgb(150.0, 102.0, 58.0),
    Rgb(178.0, 130.0, 78.0),
    Rgb(128.0, 84.0, 52.0),
];
/// Limestone, granite, basalt, slate, marble and sandstone.
const STONES: [Rgb; 6] = [
    Rgb(206.0, 200.0, 178.0),
    Rgb(184.0, 142.0, 138.0),
    Rgb(98.0, 98.0, 104.0),
    Rgb(112.0, 122.0, 142.0),
    Rgb(222.0, 222.0, 228.0),
    Rgb(204.0, 172.0, 120.0),
];
/// Obsidian and basalt around the magma sea.
const DEEPS: [Rgb; 2] = [Rgb(80.0, 58.0, 96.0), Rgb(98.0, 98.0, 104.0)];
/// Hematite, malachite, native gold, galena, native silver and sapphire.
const ORES: [Rgb; 6] = [
    Rgb(168.0, 52.0, 40.0),
    Rgb(40.0, 156.0, 104.0),
    Rgb(232.0, 192.0, 40.0),
    Rgb(156.0, 166.0, 184.0),
    Rgb(214.0, 218.0, 228.0),
    Rgb(60.0, 96.0, 236.0),
];
/// Demons' colors: Dwarf Fortress's bright red, green, magenta, cyan, yellow and white.
const FIENDISH: [Rgb; 6] = [
    Rgb(255.0, 85.0, 85.0),
    Rgb(85.0, 255.0, 120.0),
    Rgb(255.0, 85.0, 255.0),
    Rgb(85.0, 255.0, 255.0),
    Rgb(255.0, 255.0, 85.0),
    Rgb(245.0, 245.0, 245.0),
];
/// How strongly rock, a vein, and a found warm or damp stone tint the background.
const ROCK_TINT: f64 = 0.3;
const VEIN_TINT: f64 = 0.45;
const FOUND_TINT: f64 = 0.5;

/// Depths at which stone, the caverns, the deep, and the magma sea begin, and the bottom, where raw
/// adamantine rises from the underworld.
const STONE_AT: u32 = 3;
const CAVERNS_AT: u32 = 12;
const DEEP_AT: u32 = 30;
const MAGMA_AT: u32 = 45;
const BOTTOM: u32 = 60;

/// Quanta a tile takes to dig, and between strikes of the pick.
const DIG: u64 = 90;
const STRIKE: u64 = 30;
/// Seconds a strike's glow on the rock face lasts.
const STRIKE_FADE: f64 = 0.25;
/// Mean seconds of work between fatal fun.
const DOOM: f64 = 1000.0;
/// Tiles dug when a siege comes down the stair: half the rock.
const SIEGE_AT: usize = ROCK / 2;

/// Tiles a second: the miner walking to work and fleeing, a giant cave spider, a demon, a goblin
/// marching, a fire imp swimming, and a bolt in flight.
const WALK: f64 = 4.0;
const RUN: f64 = 7.0;
const SPIDER_PACE: f64 = 9.5;
const DEMON_PACE: f64 = 9.0;
const GOBLIN_PACE: f64 = 4.5;
const IMP_PACE: f64 = 2.0;
const BOLT_PACE: f64 = 12.0;
/// Tiles between blows.
const REACH: f64 = 0.9;
/// Seconds per step of a flood: water runs, magma creeps.
const WATER_STEP: f64 = 0.12;
const MAGMA_STEP: f64 = 0.36;
/// A liquid's greatest depth, in sevenths.
const FULL: u8 = 7;
/// The depths at which a dwarf swims, and at which he goes under.
const SWIMMING: u8 = 4;
const UNDER: u8 = 3;

/// Seconds: under water before a dwarf drowns, alight before he burns to death, held in a web, a
/// blow taking to land, and a dragonfire burst lasting.
const DROWN: f64 = 3.0;
const BURN: f64 = 2.5;
const WEBBED: f64 = 2.0;
const SWING: f64 = 1.1;
const BREATH: f64 = 0.6;
/// Seconds between a fire imp's fireballs and a demon's breaths or webs.
const IMP_RELOAD: f64 = 1.8;
const DEMON_RELOAD: f64 = 2.8;
/// Seconds between a horde's creatures: demons pouring out of the underworld, or goblins coming
/// down the stair.
const HORDE_GAP: f64 = 0.55;
/// Seconds: paused at warm or damp stone, showing a migrant's `X`, before migrants come, before a lost
/// corridor's replacement opens, stepping down a stair, and before a fallen fortress's successor
/// embarks.
const WARY: f64 = 2.0;
const ARRIVE: f64 = 3.0;
const MOURN: f64 = 5.0;
const SEALED: f64 = 2.5;
const STEP_DOWN: f64 = 1.0;
const FALLEN: f64 = 6.0;
/// Half periods: a status icon's alternation, a burning creature's flash, and a blow's arrow.
const BLINK: f64 = 0.7;
const FLASH: f64 = 0.16;
const LUNGE: f64 = 0.12;
/// Between bouts the sleeping miner shows a `Z` now and then, at most once a lull.
const SNORE: Lottery = Lottery {
    every: 2.0 * LULL,
    odds: 0.6,
    span: 0.3,
    seed: 41,
};

/// A level's fun: what waits past the corridor's end, or a siege that comes down its stair.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Fun {
    /// An aquifer or a cavern lake, which floods the corridor.
    Water,
    /// The magma sea, with fire imps swimming in it.
    Magma,
    /// A cavern where a giant cave spider lurks.
    Spider,
    /// A goblin siege, which comes down the up stair halfway along the corridor.
    Siege,
    /// The underworld beneath raw adamantine, whose demons end the fortress.
    Circus,
}

/// What a rock tile holds besides its stone.
#[derive(Clone, Copy)]
enum Vein {
    /// An ore or gem, by index into [`ORES`].
    Ore(usize),
    Adamantine,
}

/// One corridor at one depth.
struct Level {
    depth: u32,
    /// The level's stone, by index into its layer's stones.
    stone: usize,
    veins: [Option<Vein>; TILES],
    fun: Option<Fun>,
    /// Whether the fun kills the miner.
    fatal: bool,
    /// Tiles dug: those below are floor.
    dug: usize,
    /// Quanta into the next tile.
    chips: u64,
    /// Whether the miner found warm or damp stone before the end.
    found: bool,
}

impl Level {
    fn new(rng: &mut Rng, depth: u32) -> Self {
        let bottom = depth >= BOTTOM;
        let mut veins = [None; TILES];
        if bottom {
            // raw adamantine thickens toward the spire's hollow heart
            for (tile, vein) in veins.iter_mut().enumerate() {
                if rng.chance(real(tile) / real(TILES)) {
                    *vein = Some(Vein::Adamantine);
                }
            }
        } else if depth >= STONE_AT {
            for _ in 0..rng.below(3) {
                let ore = Vein::Ore(rng.below(ORES.len()));
                let start = rng.below(ROCK - 4);
                let length = 2 + rng.below(4);
                for vein in veins.iter_mut().skip(start).take(length) {
                    *vein = Some(ore);
                }
            }
        }
        let fun = if bottom {
            Some(Fun::Circus)
        } else {
            rng.chance(odds(depth)).then(|| fun_at(rng, depth))
        };
        Self {
            depth,
            stone: rng.below(STONES.len()),
            veins,
            fun,
            fatal: bottom,
            dug: 0,
            chips: 0,
            found: false,
        }
    }

    /// Where the miner stands to dig: amid the last tile dug, or on the up stair before the first.
    fn face(&self) -> f64 {
        STAIR + real(self.dug)
    }

    /// The color of rock tile `tile`'s stone and of the vein in it, if any.
    fn rock(&self, tile: usize) -> (Rgb, Option<Rgb>) {
        let stones: &[Rgb] = match self.depth {
            0..STONE_AT => &SOILS,
            STONE_AT..MAGMA_AT => &STONES,
            _ => &DEEPS,
        };
        let vein = self
            .veins
            .get(tile)
            .copied()
            .flatten()
            .map(|vein| match vein {
                Vein::Ore(ore) => ORES[ore % ORES.len()],
                Vein::Adamantine => ADAMANTINE,
            });
        (stones[self.stone % stones.len()], vein)
    }
}

/// The odds that a level at `depth` hides fun the miner survives.
fn odds(depth: u32) -> f64 {
    match depth {
        0..STONE_AT => 0.04,
        STONE_AT..CAVERNS_AT => 0.08,
        CAVERNS_AT..MAGMA_AT => 0.16,
        _ => 0.25,
    }
}

/// The fun a level at `depth` may hide: water above the caverns, spiders in them, and magma below;
/// and once the fortress has struck stone, a siege at any depth.
fn fun_at(rng: &mut Rng, depth: u32) -> Fun {
    let menu: &[Fun] = match depth {
        0..STONE_AT => &[Fun::Water],
        STONE_AT..CAVERNS_AT => &[Fun::Water, Fun::Siege],
        CAVERNS_AT..DEEP_AT => &[Fun::Water, Fun::Spider, Fun::Siege],
        DEEP_AT..MAGMA_AT => &[Fun::Water, Fun::Spider, Fun::Magma, Fun::Siege],
        _ => &[Fun::Magma, Fun::Siege],
    };
    menu[rng.below(menu.len())]
}

/// Seconds of work until the next fatal fun: exponential, about [`DOOM`] on average.
fn doom(rng: &mut Rng) -> f64 {
    -(1.0 - rng.unit()).ln() * DOOM
}

/// The tile at coordinate `x`, clamped to the corridor.
#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "A coordinate clamped to the corridor floors to a tile in 0..TILES."
)]
fn tile(x: f64) -> usize {
    x.clamp(0.0, real(TILES - 1)) as usize
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Liquid {
    Water,
    Magma,
}

/// A liquid pouring in from past the corridor's end, its depth in sevenths on every tile.
struct Flood {
    liquid: Liquid,
    depth: [u8; TILES],
    /// Seconds until the next step.
    due: f64,
}

impl Flood {
    fn new(liquid: Liquid) -> Self {
        Self {
            liquid,
            depth: [0; TILES],
            due: 0.0,
        }
    }

    /// Seconds per step: water runs, magma creeps.
    fn step(&self) -> f64 {
        match self.liquid {
            Liquid::Water => WATER_STEP,
            Liquid::Magma => MAGMA_STEP,
        }
    }

    /// One step of flow, as Dwarf Fortress levels its liquids: each tile takes a seventh from the
    /// tile past it, which the source beyond the corridor's end keeps full, when that tile is
    /// deeper by two, or by one at even odds, and holds two sevenths or more, so that a wet tile
    /// never runs dry; a seventh moves one tile a step.
    fn flow(&mut self, rng: &mut Rng) {
        for tile in 0..TILES {
            let upstream = self.depth.get(tile + 1).copied().unwrap_or(FULL);
            let here = self.depth[tile];
            if upstream >= 2 && (upstream > here + 1 || (upstream > here && rng.chance(0.5))) {
                self.depth[tile] += 1;
                if let Some(source) = self.depth.get_mut(tile + 1) {
                    *source -= 1;
                }
            }
        }
    }

    fn color(&self) -> Rgb {
        match self.liquid {
            Liquid::Water => WATER,
            Liquid::Magma => MAGMA,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    Spider,
    Imp,
    Goblin,
    Demon,
}

/// A creature come out of the fun.
struct Foe {
    kind: Kind,
    x: f64,
    /// Seconds until it can shoot or breathe again.
    reload: f64,
    /// Seconds into the blow it is landing.
    swing: Option<f64>,
    /// Which way it roams once the miner is gone.
    heading: f64,
}

impl Foe {
    const fn new(kind: Kind, x: f64) -> Self {
        Self {
            kind,
            x,
            reload: 0.0,
            swing: None,
            heading: -1.0,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Bolt {
    Fireball,
    Web,
}

/// A fireball or a web in flight toward the miner, which either hits him or falls short.
struct Shot {
    bolt: Bolt,
    x: f64,
    hits: bool,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Body {
    Dwarf,
    Charred,
    /// A creature the miner slew.
    Creature,
    Gore,
}

/// What a creature leaves where it died.
struct Remains {
    x: f64,
    body: Body,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Power {
    Fire,
    Web,
    Claws,
}

/// A fortress's demons, generated at its embark: their color and power.
#[derive(Clone, Copy)]
struct Fiend {
    color: usize,
    power: Power,
}

impl Fiend {
    fn summon(rng: &mut Rng) -> Self {
        Self {
            color: rng.below(FIENDISH.len()),
            power: [Power::Fire, Power::Web, Power::Claws][rng.below(3)],
        }
    }
}

struct Miner {
    /// Where he stands, in tiles from the corridor's start; the up stair is at [`STAIR`].
    x: f64,
    /// Seconds left showing the migrant's `X`.
    arriving: f64,
    /// Seconds left before the fire kills him, while he burns.
    burning: Option<f64>,
    /// Seconds left caught in a web.
    webbed: f64,
    /// Seconds he has spent under the water.
    soaked: f64,
    /// Seconds left standing stunned at a breach.
    stunned: f64,
    /// Whether he flees, fights, or digs on.
    stance: Stance,
    /// Seconds into the blow he is landing.
    swing: Option<f64>,
}

impl Miner {
    const fn new(arriving: f64) -> Self {
        Self {
            x: STAIR,
            arriving,
            burning: None,
            webbed: 0.0,
            soaked: 0.0,
            stunned: 0.0,
            stance: Stance::Flee,
            swing: None,
        }
    }
}

/// What the miner does about the fun.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Stance {
    /// He runs for the up stair.
    Flee,
    /// He stands and strikes whatever reaches him.
    Fight,
    /// He digs on, his job never cancelled.
    Toil,
}

/// Where the fortress stands.
#[derive(Clone, Copy)]
enum Phase {
    /// The miner digs the corridor.
    Dig,
    /// He found warm or damp stone and stopped, `left` seconds before he goes on.
    Wary { left: f64 },
    /// The fun breaks in: the corridor's end opens, or a siege comes down the stair.
    Breach,
    /// The miner is dead; migrants arrive in `left` seconds.
    Mourn { left: f64 },
    /// The corridor is lost; another opens at the same depth in `left` seconds.
    Sealed { left: f64 },
    /// The miner steps down the stair; the next level opens in `left` seconds.
    Descend { left: f64 },
    /// The underworld overran the fortress; another embarks in `left` seconds.
    Fallen { left: f64 },
}

struct Fortress {
    rng: Rng,
    level: Level,
    phase: Phase,
    /// The miner, until he dies and before migrants replace him.
    miner: Option<Miner>,
    foes: Vec<Foe>,
    shots: Vec<Shot>,
    flood: Option<Flood>,
    remains: Vec<Remains>,
    /// Blood spattered on each tile, out of 1.
    blood: [f64; TILES],
    /// Seconds left burning on each tile.
    fire: [f64; TILES],
    /// Seconds of work until fatal fun arms, and whether it waits for a level to take it.
    doom: f64,
    armed: bool,
    /// The last strike's glow on the rock face, out of 1, and quanta toward the next strike.
    strike: f64,
    swings: u64,
    /// Seconds of fortress time, which stands still between bouts.
    clock: f64,
    fiend: Fiend,
    /// Demons yet to pour out of the underworld, and seconds until the next.
    horde: usize,
    spawn: f64,
}

impl Fortress {
    fn new(seed: u64) -> Self {
        let mut rng = Rng::new(seed);
        let fiend = Fiend::summon(&mut rng);
        let level = Level::new(&mut rng, 0);
        let doom = doom(&mut rng);
        Self {
            rng,
            level,
            phase: Phase::Dig,
            miner: Some(Miner::new(0.0)),
            foes: Vec::new(),
            shots: Vec::new(),
            flood: None,
            remains: Vec::new(),
            blood: [0.0; TILES],
            fire: [0.0; TILES],
            doom,
            armed: false,
            strike: 0.0,
            swings: 0,
            clock: 0.0,
            fiend,
            horde: 0,
            spawn: 0.0,
        }
    }

    /// Opens a fresh corridor at `depth`, the miner coming down the stair, or a migrant if he
    /// is dead.
    fn open(&mut self, depth: u32) {
        self.level = Level::new(&mut self.rng, depth);
        self.flood = None;
        self.foes.clear();
        self.shots.clear();
        self.remains.clear();
        self.blood = [0.0; TILES];
        self.fire = [0.0; TILES];
        self.horde = 0;
        let arriving = if self.miner.is_some() { 0.0 } else { ARRIVE };
        self.miner = Some(Miner::new(arriving));
        self.phase = Phase::Dig;
    }

    /// Gives armed doom to the level being dug, unless it is too far along to hide fun.
    fn arm(&mut self) {
        let level = &mut self.level;
        if !matches!(self.phase, Phase::Dig) || level.dug + 2 >= TILES {
            return;
        }
        if level.fun.is_none() {
            level.fun = Some(fun_at(&mut self.rng, level.depth));
        }
        level.fatal = true;
        self.armed = false;
        self.doom = doom(&mut self.rng);
    }

    fn dig(&mut self, stage: &Stage) {
        if !self.toil(stage) {
            return;
        }
        let (fun, dug) = (self.level.fun, self.level.dug);
        match fun {
            Some(Fun::Water | Fun::Magma) if !self.level.found && dug + 2 >= TILES => {
                self.level.found = true;
                self.phase = Phase::Wary { left: WARY };
            }
            Some(Fun::Siege) if dug >= SIEGE_AT => self.breach(Fun::Siege),
            Some(fun) if dug == TILES => self.breach(fun),
            None if dug == TILES => self.phase = Phase::Descend { left: STEP_DOWN },
            _ => {}
        }
    }

    /// While the host is busy, walks the miner up to the rock face, then swings his pick at it as
    /// work comes in; whether he works at the face.
    fn toil(&mut self, stage: &Stage) -> bool {
        if !stage.busy() {
            return false;
        }
        let face = self.level.face();
        let Some(miner) = self.miner.as_mut() else {
            return false;
        };
        if miner.x < face {
            miner.x = (miner.x + WALK * TICK_SECS).min(face);
            return false;
        }
        if self.level.dug < TILES {
            let fresh = u64::from(stage.fresh());
            self.swings += fresh;
            if self.swings >= STRIKE {
                self.swings %= STRIKE;
                self.strike = 1.0;
            }
            self.level.chips += fresh;
            while self.level.chips >= DIG && self.level.dug < TILES {
                self.level.chips -= DIG;
                self.level.dug += 1;
            }
        }
        true
    }

    /// Lets `fun` in: the corridor's end opens onto it, or it comes down the stair.
    fn breach(&mut self, fun: Fun) {
        let fatal = self.level.fatal;
        self.phase = Phase::Breach;
        let beyond = real(TILES);
        match fun {
            Fun::Water => self.flood = Some(Flood::new(Liquid::Water)),
            Fun::Magma => {
                self.flood = Some(Flood::new(Liquid::Magma));
                for imp in 0..=self.rng.below(2) {
                    self.foes
                        .push(Foe::new(Kind::Imp, beyond + 0.5 + real(imp)));
                }
            }
            Fun::Spider => self.foes.push(Foe::new(Kind::Spider, beyond + 5.0)),
            Fun::Siege | Fun::Circus => {
                self.horde = 3 + self.rng.below(3);
                self.spawn = 0.0;
            }
        }
        let Some(miner) = self.miner.as_mut() else {
            return;
        };
        miner.stance = match (fun, fatal) {
            (Fun::Spider | Fun::Siege, false) => Stance::Fight,
            // his dig job is never cancelled
            (Fun::Siege, true) => Stance::Toil,
            _ => Stance::Flee,
        };
        miner.stunned = match (fun, fatal) {
            // swept off his feet where he stands
            (Fun::Water | Fun::Magma, true) => f64::INFINITY,
            _ => 0.0,
        };
    }

    /// One tick of the fun: the flood, the horde, the miner, the creatures and their bolts.
    fn rampage(&mut self, stage: &Stage) {
        if let Some(flood) = self.flood.as_mut() {
            flood.due -= TICK_SECS;
            while flood.due <= 0.0 {
                flood.due += flood.step();
                flood.flow(&mut self.rng);
            }
        }
        if self.horde > 0 {
            self.spawn -= TICK_SECS;
            if self.spawn <= 0.0 {
                self.horde -= 1;
                self.spawn = HORDE_GAP;
                let foe = if self.level.fun == Some(Fun::Siege) {
                    Foe::new(Kind::Goblin, STAIR)
                } else {
                    Foe::new(Kind::Demon, real(TILES) + 3.0)
                };
                self.foes.push(foe);
            }
        }
        if self.level.fun == Some(Fun::Siege) && self.toiling() {
            self.toil(stage);
        }
        self.flee();
        self.hunt();
        self.fly();
    }

    /// Whether the miner digs through the fun: always while he toils, and between foes while he
    /// fights.
    fn toiling(&self) -> bool {
        self.miner.as_ref().is_some_and(|miner| match miner.stance {
            Stance::Toil => true,
            Stance::Fight => {
                miner.swing.is_none()
                    && !self
                        .foes
                        .iter()
                        .any(|foe| (foe.x - miner.x).abs() <= REACH + 0.05)
            }
            Stance::Flee => false,
        })
    }

    /// The liquid on tile `tile` and its depth, if any.
    fn liquid(&self, tile: usize) -> Option<(Liquid, u8)> {
        self.flood
            .as_ref()
            .map(|flood| (flood.liquid, flood.depth[tile]))
            .filter(|&(_, depth)| depth > 0)
    }

    /// The miner's tick in the fun: he burns, drowns, fights, digs on, or flees for the up stair.
    fn flee(&mut self) {
        let fatal = self.level.fatal;
        let grappled = self.foes.iter().any(|foe| foe.swing.is_some());
        let Some(x) = self.miner.as_ref().map(|miner| miner.x) else {
            return;
        };
        let here = tile(x);
        let liquid = self.liquid(here);
        let scorched = self.fire[here] > 0.0 || matches!(liquid, Some((Liquid::Magma, _)));
        let Some(miner) = self.miner.as_mut() else {
            return;
        };
        if fatal && scorched && miner.burning.is_none() {
            // he runs for the stair ablaze
            miner.burning = Some(BURN);
            miner.stunned = 0.0;
        }
        let depth = match liquid {
            Some((Liquid::Water, depth)) => depth,
            _ => 0,
        };
        if fatal && depth >= UNDER {
            miner.soaked += TICK_SECS;
        }
        if let Some(left) = miner.burning.as_mut() {
            *left -= TICK_SECS;
        }
        if let Some(blow) = miner.swing.as_mut() {
            *blow += TICK_SECS;
        }
        if miner.stunned > 0.0 {
            miner.stunned -= TICK_SECS;
        } else if miner.webbed > 0.0 {
            miner.webbed -= TICK_SECS;
        } else if miner.stance == Stance::Flee && miner.swing.is_none() && !grappled {
            let pace = if depth >= SWIMMING { RUN / 2.0 } else { RUN };
            miner.x -= pace * TICK_SECS;
        }
        let burnt = miner.burning.is_some_and(|left| left <= 0.0);
        let drowned = miner.soaked >= DROWN;
        let landed = miner.swing.is_some_and(|blow| blow >= SWING);
        let escaped = miner.x <= STAIR;
        if escaped {
            miner.x = STAIR;
        }
        if burnt {
            self.die(Body::Charred);
        } else if drowned {
            self.die(Body::Dwarf);
        } else if landed {
            self.slay();
        } else if escaped && matches!(self.phase, Phase::Breach) {
            self.phase = Phase::Sealed { left: SEALED };
        }
    }

    /// The creatures' tick: they chase the miner, strike, shoot and breathe fire; once he is
    /// gone, spiders slink home, goblins march back up the stair, and demons roam.
    fn hunt(&mut self) {
        let fatal = self.level.fatal;
        let target = self.miner.as_ref().map(|miner| miner.x);
        let flood = &self.flood;
        // where an imp can swim: magma, or the sea past the corridor's end
        let molten = |at: f64| {
            at >= real(TILES)
                || flood
                    .as_ref()
                    .is_some_and(|flood| flood.liquid == Liquid::Magma && flood.depth[tile(at)] > 0)
        };
        let power = self.fiend.power;
        let mut killed = false;
        let mut bolts = Vec::new();
        let mut breaths = Vec::new();
        for foe in &mut self.foes {
            foe.reload -= TICK_SECS;
            if let Some(blow) = foe.swing.as_mut() {
                *blow += TICK_SECS;
                if *blow >= SWING {
                    foe.swing = None;
                    killed = true;
                }
            }
            let Some(x) = target else {
                foe.x += match foe.kind {
                    Kind::Spider => SPIDER_PACE * TICK_SECS,
                    Kind::Imp => 0.0,
                    Kind::Goblin => -GOBLIN_PACE * TICK_SECS,
                    Kind::Demon => foe.heading * DEMON_PACE * 0.3 * TICK_SECS,
                };
                continue;
            };
            let gap = foe.x - x;
            match foe.kind {
                Kind::Imp => {
                    if gap > 6.0 && molten(foe.x - 1.0) {
                        foe.x -= IMP_PACE * TICK_SECS;
                    }
                    if gap < 12.0 && foe.reload <= 0.0 {
                        foe.reload = IMP_RELOAD;
                        bolts.push(Shot {
                            bolt: Bolt::Fireball,
                            x: foe.x - 0.5,
                            hits: fatal,
                        });
                    }
                }
                Kind::Goblin => {
                    // marching down from the stair behind him
                    if x - foe.x > REACH {
                        foe.x = (foe.x + GOBLIN_PACE * TICK_SECS).min(x - REACH);
                    }
                    if x - foe.x <= REACH && fatal && foe.swing.is_none() && !killed {
                        foe.swing = Some(0.0);
                    }
                }
                Kind::Spider | Kind::Demon => {
                    let pace = if foe.kind == Kind::Spider {
                        SPIDER_PACE
                    } else {
                        DEMON_PACE
                    };
                    if gap > REACH {
                        foe.x = (foe.x - pace * TICK_SECS).max(x + REACH);
                    }
                    // caught: a fated miner is grappled where he runs
                    if foe.x - x <= REACH && fatal && foe.swing.is_none() && !killed {
                        foe.swing = Some(0.0);
                    }
                    if foe.kind == Kind::Demon && foe.reload <= 0.0 {
                        match power {
                            Power::Fire if gap <= 6.0 => {
                                foe.reload = DEMON_RELOAD;
                                breaths.push(foe.x);
                            }
                            Power::Web if gap > REACH && gap <= 9.0 => {
                                foe.reload = DEMON_RELOAD;
                                bolts.push(Shot {
                                    bolt: Bolt::Web,
                                    x: foe.x - 0.5,
                                    hits: true,
                                });
                            }
                            _ => {}
                        }
                    }
                }
            }
        }
        if let Some(miner) = self.miner.as_mut()
            && miner.stance == Stance::Fight
            && miner.swing.is_none()
            && self
                .foes
                .iter()
                .any(|foe| (foe.x - miner.x).abs() <= REACH + 0.05)
        {
            miner.swing = Some(0.0);
        }
        for x in breaths {
            // dragonfire spreads out from the demon's maw
            for reach in 1..=6 {
                let at = x - real(reach);
                if at >= 0.0 {
                    self.fire[tile(at)] = BREATH;
                }
            }
        }
        self.shots.extend(bolts);
        self.foes
            .retain(|foe| foe.x >= STAIR && foe.x < real(TILES) + 8.0);
        if killed {
            self.die(Body::Gore);
        }
    }

    /// Bolts in flight: each flies at the miner and hits him, or falls short and fizzles.
    fn fly(&mut self) {
        let Some(x) = self.miner.as_ref().map(|miner| miner.x) else {
            self.shots.clear();
            return;
        };
        let mut ignite = false;
        let mut snare = false;
        let mut sparks = Vec::new();
        self.shots.retain_mut(|shot| {
            shot.x -= BOLT_PACE * TICK_SECS;
            let short = if shot.hits { 0.3 } else { 2.5 };
            if shot.x > x + short {
                return true;
            }
            match (shot.bolt, shot.hits) {
                (Bolt::Fireball, true) => ignite = true,
                (Bolt::Web, true) => snare = true,
                (Bolt::Fireball, false) => sparks.push(tile(shot.x)),
                (Bolt::Web, false) => {}
            }
            false
        });
        for spark in sparks {
            self.fire[spark] = BREATH / 2.0;
        }
        if let Some(miner) = self.miner.as_mut() {
            if ignite && miner.burning.is_none() {
                miner.burning = Some(BURN);
            }
            if snare {
                miner.webbed = WEBBED;
            }
        }
    }

    /// The miner's blow lands: the creature beside him dies, and the fun ends with the last one.
    fn slay(&mut self) {
        let Some(miner) = self.miner.as_mut() else {
            return;
        };
        miner.swing = None;
        let x = miner.x;
        if let Some(index) = self
            .foes
            .iter()
            .position(|foe| (foe.x - x).abs() <= REACH + 0.05)
        {
            let foe = self.foes.remove(index);
            self.spatter(foe.x, 0.7);
            self.remains.push(Remains {
                x: foe.x,
                body: Body::Creature,
            });
        }
        if self.foes.is_empty() && self.flood.is_none() && self.horde == 0 {
            self.level.fun = None;
            self.phase = Phase::Dig;
            if let Some(miner) = self.miner.as_mut() {
                miner.stance = Stance::Flee;
            }
        }
    }

    /// The miner dies, leaving `body`.
    fn die(&mut self, body: Body) {
        let Some(miner) = self.miner.take() else {
            return;
        };
        if body == Body::Gore {
            self.spatter(miner.x, 1.0);
            let flung = if self.rng.chance(0.5) { 1.0 } else { -1.0 };
            self.remains.push(Remains {
                x: (miner.x + flung).max(0.0),
                body: Body::Gore,
            });
        }
        self.remains.push(Remains { x: miner.x, body });
        for foe in &mut self.foes {
            foe.swing = None;
        }
        self.horde = 0;
        self.level.fatal = false;
        self.phase = if self.level.fun == Some(Fun::Circus) {
            Phase::Fallen { left: FALLEN }
        } else {
            Phase::Mourn { left: MOURN }
        };
    }

    /// Spatters `amount` of blood around `x`.
    fn spatter(&mut self, x: f64, amount: f64) {
        let here = tile(x);
        for (offset, share) in [(0, 1.0), (-1, 0.6), (1, 0.6), (-2, 0.25), (2, 0.25)] {
            if let Some(cell) = here
                .checked_add_signed(offset)
                .and_then(|at| self.blood.get_mut(at))
            {
                *cell = (*cell + amount * share).min(1.0);
            }
        }
    }

    /// Migrants arrive: they take up the corridor if it is safe, and dig a fresh one if not.
    fn migrate(&mut self) {
        if self.flood.is_none() && self.foes.is_empty() {
            self.level.fun = None;
            self.miner = Some(Miner::new(ARRIVE));
            self.phase = Phase::Dig;
        } else {
            self.open(self.level.depth);
        }
    }

    /// Demons roam the fallen fortress, turning now and then.
    fn roam(&mut self) {
        for foe in &mut self.foes {
            if self.rng.chance(TICK_SECS / 0.8) {
                foe.heading = -foe.heading;
            }
            foe.x =
                (foe.x + foe.heading * DEMON_PACE * 0.3 * TICK_SECS).clamp(0.0, real(TILES) - 0.5);
        }
    }

    /// A new fortress embarks at the surface, with demons of its own below.
    fn embark(&mut self) {
        self.fiend = Fiend::summon(&mut self.rng);
        self.miner = Some(Miner::new(0.0));
        self.open(0);
    }
}

impl Shader for Fortress {
    fn tick(&mut self, stage: &Stage) {
        if stage.spell == Spell::Idle {
            return;
        }
        self.clock += TICK_SECS;
        self.strike = (self.strike - TICK_SECS / STRIKE_FADE).max(0.0);
        for fire in &mut self.fire {
            *fire = (*fire - TICK_SECS).max(0.0);
        }
        if let Some(miner) = self.miner.as_mut() {
            miner.arriving = (miner.arriving - TICK_SECS).max(0.0);
        }
        if stage.busy() {
            self.doom -= TICK_SECS;
        }
        if self.doom <= 0.0 {
            self.armed = true;
        }
        if self.armed {
            self.arm();
        }
        match self.phase {
            Phase::Dig => self.dig(stage),
            Phase::Breach | Phase::Mourn { .. } | Phase::Sealed { .. } => self.rampage(stage),
            Phase::Fallen { .. } => self.roam(),
            Phase::Wary { .. } | Phase::Descend { .. } => {}
        }
        let (Phase::Wary { left }
        | Phase::Mourn { left }
        | Phase::Sealed { left }
        | Phase::Descend { left }
        | Phase::Fallen { left }) = &mut self.phase
        else {
            return;
        };
        *left -= TICK_SECS;
        if *left > 0.0 {
            return;
        }
        match self.phase {
            Phase::Wary { .. } => self.phase = Phase::Dig,
            Phase::Mourn { .. } => self.migrate(),
            Phase::Sealed { .. } => self.open(self.level.depth),
            Phase::Descend { .. } => self.open(self.level.depth + 1),
            Phase::Fallen { .. } => self.embark(),
            Phase::Dig | Phase::Breach => {}
        }
    }

    fn status(&self, stage: &Stage, palette: Palette, canvas: &mut Canvas) {
        let paint = Paint {
            fortress: self,
            stage,
            palette,
        };
        let count = stage.caption.len();
        let per = real(ROCK) / real(count.max(1));
        // the pick strikes the first rock cell past where the miner stands to dig
        let face = self.level.face();
        let struck = (0..count)
            .map(|index| real(index) * per)
            .chain((ROCK..TILES).map(real))
            .position(|from| from > face);
        let mut shades = Vec::new();
        canvas.mark = vec![paint.mark()];
        canvas.letters = stage.letters(|index, letter| {
            let from = real(index) * per;
            let ink = paint.cell(from, from + per, false, struck == Some(index));
            shades.push((ink.bg, letter.width()));
            ink
        });
        canvas.tail = (0..TAIL)
            .map(|cell| {
                let from = real(ROCK + cell);
                let ink = paint.cell(from, from + 1.0, true, struck == Some(count + cell));
                shades.push((ink.bg, 1));
                ink
            })
            .collect();
        if let Some(glow) = paint.glow() {
            let mut columns = vec![palette.bg; LEAD];
            for (bg, width) in shades {
                columns.extend(iter::repeat_n(bg.unwrap_or(palette.bg), width));
            }
            columns.extend((0..GLOW).map(|column| {
                let fade = 1.0 - real(column) / real(GLOW);
                glow.blend(palette.bg, 0.45 * fade * fade)
            }));
            columns.truncate(BACKDROP);
            canvas.backdrop = columns;
        }
    }

    fn bullet(&self, _stage: &Stage, palette: Palette) -> Ink {
        match &self.miner {
            Some(miner) => Ink::of(DWARF).fg(palette.legible(self.complexion(miner))),
            None => Ink::of(REMAINS).fg(palette.legible(BLOOD)),
        }
    }
}

impl Fortress {
    /// Whether an alternation with half period `half` shows its second face now.
    fn blink(&self, half: f64) -> bool {
        frac(self.clock / (2.0 * half)) >= 0.5
    }

    /// The miner's color: light grey, flashing red and yellow while he burns.
    fn complexion(&self, miner: &Miner) -> Rgb {
        match miner.burning {
            Some(_) if self.blink(FLASH) => FLAME,
            Some(_) => BLAZE,
            None => GREY,
        }
    }
}

/// One frame's paint of a fortress.
struct Paint<'a> {
    fortress: &'a Fortress,
    stage: &'a Stage,
    palette: Palette,
}

impl Paint<'_> {
    /// The mark: the up stair, or the miner standing on it.
    fn mark(&self) -> Ink {
        match self.fortress.miner.as_ref().filter(|miner| miner.x < 0.0) {
            Some(miner) => self.miner(miner),
            None => Ink::of(UP).fg(self.palette.grey(0.7)),
        }
    }

    fn miner(&self, miner: &Miner) -> Ink {
        let fortress = self.fortress;
        let resting = self.stage.spell == Spell::Idle;
        let calm = matches!(fortress.phase, Phase::Dig | Phase::Wary { .. });
        let jobless = calm
            && (self.stage.spell == Spell::Wait || matches!(fortress.phase, Phase::Wary { .. }));
        let glyph = if resting && calm && SNORE.at(self.stage.t).is_some() {
            SLEEP
        } else if miner.webbed > 0.0 && fortress.blink(BLINK) {
            WEB
        } else if miner.arriving > 0.0 && fortress.blink(BLINK) {
            MIGRANT
        } else if !resting && jobless && fortress.blink(BLINK) {
            JOBLESS
        } else if miner.swing.is_some() && fortress.blink(LUNGE) {
            // his blow lands on the foes' side
            if fortress.foes.iter().any(|foe| foe.x < miner.x) {
                LEFT
            } else {
                RIGHT
            }
        } else {
            DWARF
        };
        Ink::of(glyph)
            .fg(self.palette.legible(fortress.complexion(miner)))
            .bold(true)
    }

    fn foe(&self, foe: &Foe) -> Ink {
        let (glyph, color) = match foe.kind {
            Kind::Spider => ('S', GREY),
            Kind::Imp => ('i', AMBER),
            Kind::Goblin => ('g', MILITARY),
            Kind::Demon => (DEMON, FIENDISH[self.fortress.fiend.color]),
        };
        let glyph = if foe.swing.is_some() && self.fortress.blink(LUNGE) {
            // its blow lands toward the miner
            let miner = self.fortress.miner.as_ref().map_or(foe.x, |miner| miner.x);
            if miner < foe.x { LEFT } else { RIGHT }
        } else {
            glyph
        };
        Ink::of(glyph).fg(self.palette.legible(color)).bold(true)
    }

    /// The miner, creature, bolt, stair or remains in the tiles from `from` to `to`, the liveliest
    /// first.
    fn sprite(&self, from: f64, to: f64, tail: bool) -> Option<Ink> {
        let fortress = self.fortress;
        let within = |x: f64| x >= from && x < to;
        let descending =
            matches!(fortress.phase, Phase::Descend { left } if left < STEP_DOWN / 2.0);
        if let Some(miner) = fortress
            .miner
            .as_ref()
            .filter(|miner| within(miner.x) && !descending)
        {
            return Some(self.miner(miner));
        }
        if let Some(shot) = fortress.shots.iter().find(|shot| within(shot.x)) {
            let ink = match shot.bolt {
                Bolt::Fireball => {
                    let flame = if fortress.blink(FLASH) { FLAME } else { BLAZE };
                    Ink::of(FIREBALL).fg(self.palette.legible(flame)).bold(true)
                }
                Bolt::Web => Ink::of(WEB).fg(self.palette.legible(GREY)),
            };
            return Some(ink);
        }
        if let Some(foe) = fortress.foes.iter().find(|foe| within(foe.x)) {
            return Some(self.foe(foe));
        }
        if tail && to >= real(TILES) && matches!(fortress.phase, Phase::Descend { .. }) {
            return Some(Ink::of(DOWN).fg(self.palette.grey(0.8)).bold(true));
        }
        fortress
            .remains
            .iter()
            .rev()
            .find(|remains| within(remains.x))
            .map(|remains| {
                let color = match remains.body {
                    Body::Dwarf | Body::Creature => GREY,
                    Body::Charred => CHAR,
                    Body::Gore => BLOOD,
                };
                Ink::of(REMAINS).fg(self.palette.legible(color))
            })
    }

    /// The cell over tiles `from` to `to`: rock or floor, with whatever stands, lies or flows
    /// there; `struck` when the pick strikes it. A cell is floor once the miner can stand in it
    /// to dig, so one spanning several tiles opens as he steps in and he never stands on rock.
    fn cell(&self, from: f64, to: f64, tail: bool, struck: bool) -> Ink {
        let fortress = self.fortress;
        let level = &fortress.level;
        let palette = self.palette;
        let tiles = tile(from)..=tile((to - 1e-6).max(from));
        let middle = tile(f64::midpoint(from, to));
        let dug = from <= level.face();
        let fire = tiles.clone().any(|at| fortress.fire[at] > 0.0);
        let liquid = tiles
            .clone()
            .filter_map(|at| fortress.liquid(at))
            .max_by_key(|&(_, depth)| depth);
        let blood = tiles
            .clone()
            .map(|at| fortress.blood[at])
            .fold(0.0, f64::max);
        let faint = palette.grey(0.38);
        let (bg, fg) = if fire {
            let flame = if fortress.blink(FLASH) { FLAME } else { BLAZE };
            (Some(flame.blend(palette.bg, 0.7)), Some(faint))
        } else if let (true, Some((_, depth))) = (dug, liquid) {
            let color = fortress.flood.as_ref().map_or(WATER, Flood::color);
            let alpha = 0.3 + 0.5 * f64::from(depth) / f64::from(FULL);
            (
                Some(color.blend(palette.bg, alpha)),
                Some(palette.legible(color).blend(faint, 0.4)),
            )
        } else if dug {
            let floor = (blood > 0.0).then(|| BLOOD.blend(palette.bg, 0.6 * blood));
            (floor, Some(faint))
        } else {
            let (stone, vein) = tiles
                .clone()
                .map(|at| level.rock(at))
                .find(|(_, vein)| vein.is_some())
                .unwrap_or_else(|| level.rock(middle));
            let mut rock = match vein {
                Some(vein) => vein.blend(palette.bg, VEIN_TINT),
                None => stone.blend(palette.bg, ROCK_TINT),
            };
            if level.found {
                let sign = match level.fun {
                    Some(Fun::Magma) => WARM,
                    _ => DAMP,
                };
                rock = sign.blend(rock, FOUND_TINT);
            }
            if struck {
                rock = palette.fg.blend(rock, 0.2 * fortress.strike);
            }
            (Some(rock), None)
        };
        let ink = self.sprite(from, to, tail).unwrap_or_else(|| {
            let plain = match (tail, liquid) {
                (true, Some(_)) if dug => Ink::of(LIQUID)
                    .fg(palette.legible(fortress.flood.as_ref().map_or(WATER, Flood::color))),
                (true, _) => Ink::of(' '),
                (false, _) => Ink::default(),
            };
            match fg {
                Some(fg) if plain.fg.is_none() => plain.fg(fg),
                _ => plain,
            }
        });
        match bg {
            Some(bg) => ink.bg(bg),
            None => ink,
        }
    }

    /// The glow under the host's text past the tail while the fun pours in: water, magma, or the
    /// underworld's eerie light.
    fn glow(&self) -> Option<Rgb> {
        let fortress = self.fortress;
        match fortress.phase {
            Phase::Breach | Phase::Mourn { .. } | Phase::Sealed { .. } | Phase::Fallen { .. } => {
                fortress
                    .flood
                    .as_ref()
                    .map(Flood::color)
                    .or_else(|| (fortress.level.fun == Some(Fun::Circus)).then_some(EERIE))
            }
            _ => None,
        }
    }
}
