#![allow(unused_imports)]

pub mod cached;
pub use cached::CachedTileNetwork;

pub mod pmtiles_mvt_backend;
pub use pmtiles_mvt_backend::PMTilesMVTBackend;

use crate::{routing::Router, tile::Coord};

/// Trait for tile implementations.
pub trait Tile {
    fn parse(&self, router: &mut Router) -> Result<(), Box<dyn std::error::Error>>;
}

/// Trait for tile backend implementations.
pub trait Backend<T: Tile> {
    async fn get_tile(&self, coord: &Coord) -> Result<T, Box<dyn std::error::Error>>;
}
