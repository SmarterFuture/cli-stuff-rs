use std::iter;

use renderer::traits::Renderable;

use crate::snake::{Status, Tile};

impl Renderable for Tile {
    type Primitive = char;
    fn render(&self) -> impl Iterator<Item = Self::Primitive> {
        iter::once(match self {
            Tile::Corpse => 'x',
            Tile::Empty => ' ',
            Tile::Food => '+',
            Tile::Snake => 'o',
        })
    }
}

impl Renderable for Status {
    type Primitive = char;
    fn render(&self) -> impl Iterator<Item = Self::Primitive> {
        let owned = format!("Status: {:>4} Difficulty: {:>4}", self.score, self.diff)
            .chars()
            .collect::<Vec<_>>();
        owned.into_iter()
    }
}
