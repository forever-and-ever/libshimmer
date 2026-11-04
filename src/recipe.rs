//! Recipes, and what each needs of a host.

use std::error::Error;
use std::fmt;

use crate::kit::OCTANTS;
use crate::stage::Shader;

/// Glyph repertoires beyond the base set every target terminal draws: those a terminal draws, or
/// those a recipe needs.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Glyphs(u8);

impl Glyphs {
    /// The base set alone.
    pub const BASE: Self = Self(0);
    /// Unicode 16's octants, with the other symbols for legacy computing: U+1FB00 to U+1FBFF and
    /// U+1CC00 to U+1CEBF.
    pub const OCTANTS: Self = Self(1);

    /// The repertoires of both sets.
    pub const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    /// Whether every repertoire of `other` is in this set.
    pub const fn contains(self, other: Self) -> bool {
        other.0 & !self.0 == 0
    }

    const fn without(self, other: Self) -> Self {
        Self(self.0 & !other.0)
    }

    /// The glyph of the octant repertoire whose 2×4 cell lights `pixels`: bit `2 * y + x` lights
    /// the pixel in column `x` of row `y`, counting rows from the top. Unicode encodes some
    /// patterns outside the octant block, among them the base set's block elements.
    pub fn octant(pixels: u8) -> char {
        OCTANTS[usize::from(pixels)]
    }
}

/// Each repertoire's name.
const REPERTOIRES: [(Glyphs, &str); 1] = [(Glyphs::OCTANTS, "octants")];

/// The repertoire beyond the base set that `glyph` belongs to.
#[cfg(test)]
pub(crate) fn repertoire(glyph: char) -> Glyphs {
    if ('\u{1cc00}'..='\u{1cebf}').contains(&glyph) || ('\u{1fb00}'..='\u{1fbff}').contains(&glyph)
    {
        Glyphs::OCTANTS
    } else {
        Glyphs::BASE
    }
}

/// Why a recipe cannot load for a host: glyphs the terminal lacks.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Unfit {
    missing: Glyphs,
}

impl Unfit {
    /// The repertoires the recipe needs and the terminal lacks.
    pub fn missing(self) -> Glyphs {
        self.missing
    }
}

impl fmt::Display for Unfit {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("the terminal lacks ")?;
        let lacking = REPERTOIRES
            .iter()
            .filter(|(repertoire, _)| self.missing.contains(*repertoire));
        for (i, (_, name)) in lacking.enumerate() {
            if i > 0 {
                f.write_str(", ")?;
            }
            f.write_str(name)?;
        }
        Ok(())
    }
}

impl Error for Unfit {}

/// A stock animation: its picker entry, what it needs of a host, and how to start its shader.
pub struct Recipe {
    pub(crate) name: &'static str,
    pub(crate) blurb: &'static str,
    pub(crate) credit: &'static str,
    pub(crate) needs: Glyphs,
    pub(crate) listens: bool,
    pub(crate) summon: fn(u64) -> Box<dyn Shader>,
}

impl Recipe {
    /// The name a picker shows and a host persists, unique among recipes:
    /// [`stock::named`](crate::stock::named) finds the recipe by it, and no release of the same
    /// major version renames or removes it.
    pub fn name(&self) -> &'static str {
        self.name
    }

    /// One line on what the recipe paints.
    pub fn blurb(&self) -> &'static str {
        self.blurb
    }

    /// The recipe's author.
    pub fn credit(&self) -> &'static str {
        self.credit
    }

    /// The repertoires the recipe paints beyond the base set.
    pub fn needs(&self) -> Glyphs {
        self.needs
    }

    /// Whether the recipe paints the sound a host passes to [`Shimmer::hear`]: a host that hears
    /// nothing withholds it.
    ///
    /// [`Shimmer::hear`]: crate::Shimmer::hear
    pub fn listens(&self) -> bool {
        self.listens
    }

    /// Whether a shimmer can run the recipe on a terminal that draws `glyphs`.
    ///
    /// # Errors
    ///
    /// [`Unfit`] when the terminal lacks glyphs the recipe needs.
    pub fn fits(&self, glyphs: Glyphs) -> Result<(), Unfit> {
        match self.needs.without(glyphs) {
            Glyphs::BASE => Ok(()),
            missing => Err(Unfit { missing }),
        }
    }
}

impl fmt::Debug for Recipe {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Recipe")
            .field("name", &self.name)
            .field("needs", &self.needs)
            .field("listens", &self.listens)
            .finish_non_exhaustive()
    }
}
