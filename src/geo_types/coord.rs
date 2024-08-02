use geo::geometry as geo;
use wasm_bindgen::prelude::*;

#[derive(Debug, Clone)]
#[wasm_bindgen]
/// Two dimensional coordinate.
pub struct Coord(geo::Coord<f64>);

#[wasm_bindgen]
impl Coord {
    #[wasm_bindgen(constructor)]
    pub fn new(x: f64, y: f64) -> Coord {
        Coord(geo::Coord { x, y })
    }
}

impl From<geo::Coord<f64>> for Coord {
    fn from(value: geo::Coord<f64>) -> Coord {
        Coord(value)
    }
}

impl From<Coord> for geo::Coord<f64> {
    fn from(value: Coord) -> geo::Coord<f64> {
        value.0
    }
}

#[cfg(test)]
/// Create a two dimensional coordinate.
macro_rules! coord {
    (x: $x:expr, y: $y:expr $(,)* ) => {
        Coord::from(geo::Coord { x: $x, y: $y })
    };
}

#[cfg(test)]
pub(crate) use coord;
