use crate::geo_types::Point;
use std::convert::TryFrom;
use wasm_bindgen::prelude::*;

pub mod backend;

/// Coordinate of a tile.
#[derive(Debug, Clone, Hash, Eq, PartialEq)]
pub struct Coord {
    pub x: u32,
    pub y: u32,
    pub z: u8,
}

#[wasm_bindgen(module = "@mapbox/tilebelt")]
extern "C" {
    fn pointToTile(x: f64, y: f64, z: u8) -> Vec<u32>;
}

/// Returns the coordinates of the tile that cover this point.
pub fn point_to_tile_coord(point: &Point, z: u8) -> Coord {
    let ret = pointToTile(point.x(), point.y(), z);
    Coord {
        x: ret[0],
        y: ret[1],
        z: u8::try_from(ret[2]).unwrap(),
    }
}
