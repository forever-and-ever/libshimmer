//! Tetromino: a four-by-four Tetris well in two octant cells, played by a bot as quanta arrive.
//! While the host waits the falling piece blinks; between bouts the well holds still.

use std::sync::LazyLock;

use crate::kit::AZURE;
use crate::kit::CORAL;
use crate::kit::CYAN;
use crate::kit::GOLD;
use crate::kit::LIME;
use crate::kit::OCTANTS;
use crate::kit::STEEL;
use crate::kit::SUN;
use crate::kit::TANGERINE;
use crate::kit::VIOLET;
use crate::kit::WHITE;
use crate::kit::ease;
use crate::kit::frac;
use crate::kit::hash;

use super::OPUS;
use crate::Spell;
use crate::paint::{Ink, Palette, Rgb};
use crate::recipe::{Glyphs, Recipe};
use crate::stage::{Canvas, Shader, Stage};

pub(super) const RECIPE: Recipe = Recipe {
    name: "Tetromino",
    blurb: "a bot plays a tiny Tetris well",
    credit: OPUS,
    needs: Glyphs::OCTANTS,
    listens: false,
    summon: |_| Box::<Tetromino>::default(),
};

/// The well is four cells square; `Board` holds rows top down.
const SIDE: i8 = 4;
type Board = [Option<Piece>; 16];
type Cells = [(i8, i8); 4];
/// Quanta per row of fall; quanta beyond three pieces' worth are forgotten.
const FALL: u32 = 10;
const BACKLOG: u32 = 30;
/// Seconds a cleared line flashes before it goes, a feat glows, and a top-out flashes.
const FLASH: f64 = 0.2;
const FEAT: f64 = 1.6;
const TOP_OUT: f64 = 0.6;
/// Seconds the caption takes to crossfade to the last piece's tint.
const CROSSFADE: f64 = 0.4;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Piece {
    I,
    O,
    T,
    S,
    Z,
    J,
    L,
}

const PIECES: [Piece; 7] = [
    Piece::I,
    Piece::O,
    Piece::T,
    Piece::S,
    Piece::Z,
    Piece::J,
    Piece::L,
];

impl Piece {
    fn cells(self) -> Cells {
        match self {
            Piece::I => [(0, 0), (1, 0), (2, 0), (3, 0)],
            Piece::O => [(0, 0), (1, 0), (0, 1), (1, 1)],
            Piece::T => [(0, 0), (1, 0), (2, 0), (1, 1)],
            Piece::S => [(1, 0), (2, 0), (0, 1), (1, 1)],
            Piece::Z => [(0, 0), (1, 0), (1, 1), (2, 1)],
            Piece::J => [(0, 0), (0, 1), (1, 1), (2, 1)],
            Piece::L => [(2, 0), (0, 1), (1, 1), (2, 1)],
        }
    }

    fn color(self) -> Rgb {
        match self {
            Piece::I => CYAN,
            Piece::O => SUN,
            Piece::T => VIOLET,
            Piece::S => LIME,
            Piece::Z => CORAL,
            Piece::J => AZURE,
            Piece::L => TANGERINE,
        }
    }

    /// The piece's distinct rotations, each normalized to the top left and sorted row by row.
    fn orientations(self) -> &'static [Cells] {
        static ORIENTATIONS: LazyLock<Vec<Vec<Cells>>> = LazyLock::new(|| {
            PIECES
                .iter()
                .map(|piece| {
                    let mut found: Vec<Cells> = Vec::new();
                    let mut cells = piece.cells();
                    for _ in 0..4 {
                        let (x0, y0) = (
                            cells.iter().map(|c| c.0).min().unwrap_or_default(),
                            cells.iter().map(|c| c.1).min().unwrap_or_default(),
                        );
                        let mut normal = cells.map(|(x, y)| (x - x0, y - y0));
                        normal.sort_by_key(|&(x, y)| (y, x));
                        if !found.contains(&normal) {
                            found.push(normal);
                        }
                        cells = cells.map(|(x, y)| (y, -x));
                    }
                    found
                })
                .collect()
        });
        &ORIENTATIONS[self as usize]
    }

    /// The piece standing up in one octant cell, for the next-piece preview.
    fn upright(self) -> char {
        let Some(cells) = self
            .orientations()
            .iter()
            .find(|cells| cells.iter().all(|c| c.0 < 2))
        else {
            return '•';
        };
        let y0 = (3 - cells.iter().map(|c| c.1).max().unwrap_or_default()) >> 1;
        OCTANTS[cells
            .iter()
            .map(|&(x, y)| 1 << (2 * (y + y0) + x))
            .sum::<usize>()]
    }
}

/// The piece `i`th in play: shuffled seven-bags.
#[expect(
    clippy::cast_possible_wrap,
    reason = "The bag number is i / 7, below i64::MAX, and piece indices are below seven."
)]
fn bag(i: u64) -> Piece {
    let mut bag: Vec<(f64, Piece)> = PIECES
        .iter()
        .enumerate()
        .map(|(j, &piece)| (hash(77 + 7 * (i / 7) as i64 + j as i64), piece))
        .collect();
    bag.sort_by(|a, b| a.0.total_cmp(&b.0));
    bag[(i % 7) as usize].1
}

#[expect(
    clippy::cast_sign_loss,
    reason = "Short-circuit bounds checks establish 0 <= px, py < SIDE before indexing the 16-cell board."
)]
fn fits(board: &Board, cells: Cells, x: i8, y: i8) -> bool {
    cells.iter().all(|&(cx, cy)| {
        let (px, py) = (cx + x, cy + y);
        (0..SIDE).contains(&px)
            && py < SIDE
            && (py < 0 || board[(SIDE * py + px) as usize].is_none())
    })
}

/// The board after locking a piece, full rows removed, and how many there were.
#[expect(
    clippy::cast_sign_loss,
    reason = "Only fully in-board placements established by fits and the caller's top-out check are locked."
)]
fn settle(board: &Board, cells: Cells, x: i8, y: i8, piece: Piece) -> (Board, usize) {
    let mut locked = *board;
    for (cx, cy) in cells {
        locked[(SIDE * (cy + y) + cx + x) as usize] = Some(piece);
    }
    let kept: Vec<&[Option<Piece>]> = locked
        .chunks(4)
        .filter(|row| row.iter().any(Option::is_none))
        .collect();
    let lines = 4 - kept.len();
    let mut settled = [None; 16];
    for (row, cells) in kept.iter().enumerate() {
        settled[4 * (lines + row)..4 * (lines + row + 1)].copy_from_slice(cells);
    }
    (settled, lines)
}

struct Placement {
    orientation: usize,
    cells: Cells,
    x: i8,
    height: i8,
    board: Board,
    score: f64,
}

/// Every rotation and column for `piece`, with the board each leaves and its worth.
#[expect(
    clippy::cast_possible_wrap,
    clippy::cast_precision_loss,
    reason = "Counts and the heuristic score come from a 4×4 board: at most 16 cells, four rows, and twelve adjacent height differences."
)]
fn placements(board: &Board, piece: Piece) -> Vec<Placement> {
    let mut all = Vec::new();
    for (orientation, &cells) in piece.orientations().iter().enumerate() {
        let width = 1 + cells.iter().map(|c| c.0).max().unwrap_or_default();
        let height = 1 + cells.iter().map(|c| c.1).max().unwrap_or_default();
        for x in 0..=SIDE - width {
            let mut y = -height;
            while fits(board, cells, x, y + 1) {
                y += 1;
            }
            if cells.iter().any(|c| c.1 + y < 0) {
                continue;
            }
            let (settled, lines) = settle(board, cells, x, y, piece);
            let tops: [usize; 4] = std::array::from_fn(|column| {
                (0..4)
                    .find(|row| settled[4 * row + column].is_some())
                    .map_or(0, |row| 4 - row)
            });
            let holes = (0..4)
                .map(|column| {
                    (0..4)
                        .filter(|row| settled[4 * row + column].is_none() && 4 - row < tops[column])
                        .count()
                })
                .sum::<usize>();
            let bumps = tops
                .windows(2)
                .map(|pair| pair[0].abs_diff(pair[1]))
                .sum::<usize>();
            let score = 3 * lines as i64
                - 4 * holes as i64
                - tops.iter().sum::<usize>() as i64
                - bumps as i64;
            all.push(Placement {
                orientation,
                cells,
                x,
                height,
                board: settled,
                score: score as f64,
            });
        }
    }
    all
}

/// The best total worth reachable by placing the next `depth` pieces from the `i`th.
fn outlook(board: &Board, i: u64, depth: u32) -> f64 {
    if depth == 0 {
        return 0.0;
    }
    placements(board, bag(i))
        .iter()
        .map(|p| p.score + outlook(&p.board, i + 1, depth - 1))
        .fold(-1e3, f64::max)
}

/// The bot: where the `i`th piece goes, looking three pieces ahead. With nowhere to go, it drops the
/// piece at the left and tops out.
#[expect(
    clippy::cast_possible_wrap,
    reason = "Drawn pieces consume at least FALL = 10 quanta each, so the count fits i64; orientation indices are below four."
)]
fn aim(board: &Board, i: u64) -> (Cells, i8, i8) {
    placements(board, bag(i))
        .into_iter()
        .map(|p| {
            let worth = p.score
                + outlook(&p.board, i + 1, /*depth*/ 3)
                + 0.01 * hash(64 * i as i64 + 4 * i64::from(p.x) + p.orientation as i64);
            (worth, p)
        })
        .reduce(|best, next| if next.0 > best.0 { next } else { best })
        .map_or_else(
            || {
                let cells = bag(i).orientations()[0];
                (
                    cells,
                    0,
                    1 + cells.iter().map(|c| c.1).max().unwrap_or_default(),
                )
            },
            |(_, p)| (p.cells, p.x, p.height),
        )
}

#[derive(Clone, Copy)]
struct Falling {
    piece: Piece,
    cells: Cells,
    x: i8,
    y: i8,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Feat {
    Double,
    Tetris,
    TopOut,
}

#[derive(Default)]
struct Tetromino {
    board: Board,
    falling: Option<Falling>,
    /// Pieces drawn from the bags so far.
    drawn: u64,
    /// Quanta owed to gravity.
    owed: u32,
    /// Full rows flashing before they go: when they filled, and the board without them.
    clearing: Option<(f64, Board)>,
    /// The last two pieces locked, newest first, and when the newest locked.
    locked: (Option<Piece>, Option<Piece>),
    locked_at: f64,
    feat: Option<(Feat, f64)>,
}

impl Tetromino {
    /// One row of fall: spawn, drop or lock the piece.
    #[expect(
        clippy::cast_sign_loss,
        reason = "The failed downward fits check locks the prior in-board placement only after every cell passes the top-out check."
    )]
    fn gravity(&mut self, t: f64) {
        let Some(falling) = self.falling else {
            let (cells, x, height) = aim(&self.board, self.drawn);
            self.falling = Some(Falling {
                piece: bag(self.drawn),
                cells,
                x,
                y: -height,
            });
            self.drawn += 1;
            return;
        };
        let Falling { piece, cells, x, y } = falling;
        if fits(&self.board, cells, x, y + 1) {
            self.falling = Some(Falling {
                y: y + 1,
                ..falling
            });
            return;
        }
        self.falling = None;
        if cells.iter().any(|c| c.1 + y < 0) {
            self.board = [None; 16];
            self.feat = Some((Feat::TopOut, t));
            return;
        }
        let (settled, lines) = settle(&self.board, cells, x, y, piece);
        self.locked = (Some(piece), self.locked.0);
        self.locked_at = t;
        if lines == 0 {
            self.board = settled;
            return;
        }
        // Full rows flash before they go.
        for &(cx, cy) in &cells {
            self.board[(SIDE * (cy + y) + cx + x) as usize] = Some(piece);
        }
        self.clearing = Some((t, settled));
        if lines > 1 {
            self.feat = Some((
                if lines == 4 {
                    Feat::Tetris
                } else {
                    Feat::Double
                },
                t,
            ));
        }
    }

    fn feat(&self, t: f64) -> Option<(Feat, f64)> {
        self.feat
            .filter(|&(feat, at)| t - at < if feat == Feat::TopOut { TOP_OUT } else { FEAT })
    }
}

impl Shader for Tetromino {
    fn tick(&mut self, stage: &Stage) {
        self.owed = (self.owed + stage.fresh()).min(BACKLOG);
        if let Some((at, settled)) = self.clearing
            && stage.t - at >= FLASH
        {
            self.board = settled;
            self.clearing = None;
        }
        while stage.busy() && self.clearing.is_none() && self.owed >= FALL {
            self.owed -= FALL;
            self.gravity(stage.t);
        }
    }

    #[expect(
        clippy::cast_sign_loss,
        reason = "The falling placement is horizontally in bounds and visible cells pass the nonnegative row check before board indexing."
    )]
    fn status(&self, stage: &Stage, palette: Palette, canvas: &mut Canvas) {
        let t = stage.t;
        let flashing = |row: usize| {
            self.clearing.is_some() && self.board[4 * row..4 * row + 4].iter().all(Option::is_some)
        };
        let mut pixels: [Option<Rgb>; 16] = std::array::from_fn(|j| {
            self.board[j].map(|piece| {
                if flashing(j / 4) {
                    WHITE
                } else {
                    piece.color()
                }
            })
        });
        let mut own = [false; 16];
        if let Some(falling) = &self.falling
            && (stage.spell != Spell::Wait || frac(t) < 0.5)
        {
            for &(cx, cy) in &falling.cells {
                if cy + falling.y >= 0 {
                    let j = (SIDE * (cy + falling.y) + cx + falling.x) as usize;
                    pixels[j] = Some(falling.piece.color());
                    own[j] = true;
                }
            }
        }
        let feat = self.feat(t);
        let well = STEEL.blend(palette.bg, /*alpha*/ 0.12);
        let well = match feat {
            Some((Feat::Tetris, at)) => GOLD.blend(well, 0.35 * (1.0 - (t - at) / FEAT)),
            _ => well,
        };
        canvas.mark = (0..2)
            .map(|half| {
                if matches!(feat, Some((Feat::TopOut, _))) {
                    return Ink::of('█').fg(CORAL.blend(palette.bg, /*alpha*/ 0.6));
                }
                let (mut bits, mut fg) = (0, None);
                for y in 0..4 {
                    for x in 0..2 {
                        let j = 4 * y + 2 * half + x;
                        if let Some(color) = pixels[j] {
                            bits |= 1 << (2 * y + x);
                            if fg.is_none() || own[j] {
                                fg = Some(color);
                            }
                        }
                    }
                }
                Ink::of(OCTANTS[bits]).fg(fg.unwrap_or(palette.fg)).bg(well)
            })
            .collect();
        let tint = |piece: Option<Piece>| {
            piece.map_or(palette.fg, |piece| {
                piece.color().blend(palette.fg, /*alpha*/ 0.3)
            })
        };
        let fade = ease(((t - self.locked_at) / CROSSFADE).clamp(0.0, 1.0));
        let tone = if stage.busy() {
            tint(self.locked.0).blend(tint(self.locked.1), fade)
        } else {
            palette.grey(/*alpha*/ 0.6)
        };
        canvas.letters = stage.letters(|_, _| Ink::default().fg(tone));
    }

    fn bullet(&self, stage: &Stage, palette: Palette) -> Ink {
        let next = bag(self.drawn);
        let color = if stage.busy() {
            next.color()
        } else {
            next.color().blend(palette.bg, /*alpha*/ 0.6)
        };
        Ink::of(next.upright()).fg(color)
    }
}
