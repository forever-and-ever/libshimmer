//! The stock recipes.

mod cambrian;
mod caterpillar;
mod critter;
mod dandelion;
mod fireflies;
mod forge;
mod fortress;
mod frost;
mod hi_fi;
mod komorebi;
mod lava_lamp;
mod lfsr;
mod moth;
mod neon;
mod phyllotaxis;
mod prism;
mod quipu;
mod relay;
mod rubik;
mod sandpile;
mod supernova;
mod tectonic;
mod tetromino;
mod turing;
mod yo_yo;

use crate::recipe::Recipe;

const OPUS: &str = "Claude Opus 5.5";
const DEEPSEEK: &str = "DeepSeek V4.1 Flash 1017";
const USER: &str = "The User";

/// The stock recipes, in the order a picker shows them.
pub static ROSTER: &[Recipe] = &[
    lava_lamp::RECIPE,
    critter::RECIPE,
    rubik::RECIPE,
    tetromino::RECIPE,
    forge::RECIPE,
    supernova::RECIPE,
    fireflies::RECIPE,
    neon::RECIPE,
    prism::RECIPE,
    turing::RECIPE,
    sandpile::RECIPE,
    quipu::RECIPE,
    relay::RECIPE,
    phyllotaxis::RECIPE,
    tectonic::RECIPE,
    lfsr::RECIPE,
    cambrian::RECIPE,
    moth::RECIPE,
    caterpillar::RECIPE,
    dandelion::RECIPE,
    komorebi::RECIPE,
    frost::RECIPE,
    yo_yo::RECIPE,
    fortress::RECIPE,
    hi_fi::RECIPE,
];

/// The stock recipe named `name`.
pub fn named(name: &str) -> Option<&'static Recipe> {
    ROSTER.iter().find(|recipe| recipe.name == name)
}
