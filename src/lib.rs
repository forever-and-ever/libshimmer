//! Animated status lines for terminal UIs, paced by the host's work.
//!
//! A [`Shimmer`] is one running animation, made from a [`Recipe`] of [`stock::ROSTER`]. It dresses
//! a caption, such as "Working", and paints the status line's lead (a mark of glyphs, the
//! caption's letters, and a tail of cells after them) over a backdrop of background colors. It
//! also paints the bullet in front of a running sub-task, such as a tool call.
//!
//! The host drives a shimmer with commands, each holding until the next replaces it:
//! [`Shimmer::show`] sets the caption, [`Shimmer::set_spell`] what the host is doing, and
//! [`Shimmer::set_intensity`] how hard it works. Each frame, the host passes the time elapsed since
//! the previous frame to [`Shimmer::advance`], which runs whole ticks of [`TICK`], and draws the
//! [`Row`] that [`Shimmer::status`] paints. Paint depends only on the recipe, the seed, the
//! commands so far, the [`Palette`], and the [`Motion`]: libshimmer reads no clock or environment
//! and keeps no global state.
//!
//! A host that learns of work in lumps can smooth them with a [`Reservoir`], and a picker can
//! preview a recipe by playing the [`script`]. A host that hears sound, such as what its speakers
//! play, passes it to [`Shimmer::hear`], and a recipe that [listens](Recipe::listens) paints it.
//! The companion crate libshimmer-ratatui converts a row into ratatui spans and lays its backdrop
//! into a ratatui buffer.
//!
//! ```
//! use std::time::Duration;
//!
//! use libshimmer::{Glyphs, Intensity, Motion, Palette, Rgb, Shimmer, Spell, stock};
//!
//! let recipe = stock::named("Lava Lamp").expect("a stock recipe");
//! let mut shimmer = Shimmer::new(recipe, 7, Glyphs::BASE)?;
//! shimmer.set_spell(Spell::Busy);
//! shimmer.show("Compiling");
//! shimmer.set_intensity(Intensity::new(0.6));
//! shimmer.advance(Duration::from_millis(100));
//! let palette = Palette { fg: Rgb::new(208, 208, 208), bg: Rgb::new(21, 21, 21) };
//! let row = shimmer.status(palette, Motion { mark: true, letters: true });
//! let lead: String = row.cells().map(|(symbol, _)| symbol).collect();
//! assert_eq!(lead, "● Compiling");
//! # Ok::<(), libshimmer::Unfit>(())
//! ```

use std::time::Duration;

mod kit;
mod paint;
mod recipe;
mod reservoir;
mod rest;
pub mod script;
mod shimmer;
mod stage;
pub mod stock;
#[cfg(test)]
mod tests;

pub use paint::{Ink, Motion, Palette, Rgb, Row};
pub use recipe::{Glyphs, Recipe, Unfit};
pub use reservoir::Reservoir;
pub use shimmer::{Intensity, Shimmer, Spell};

/// One tick of shimmer time; a host that draws every tick schedules frames this far apart.
pub const TICK: Duration = Duration::from_millis(32);

/// A gap between frames longer than this advances a shimmer by one tick, as after a pause in drawing.
pub const STALE: Duration = Duration::from_secs(1);

/// Samples a second of the sound a host passes to [`Shimmer::hear`].
pub const SAMPLE_RATE: u32 = 48_000;

// P13: a host may keep a shimmer on any thread.
const _: () = {
    const fn crosses_threads<T: Send + Sync>() {}
    crosses_threads::<Shimmer>();
};
