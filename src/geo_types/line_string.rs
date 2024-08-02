use super::Coord;
use wasm_bindgen::prelude::*;

#[derive(Debug, Clone)]
#[wasm_bindgen]
/// A series of contiguous line segments represented by two or more Coords.
pub struct LineString(geo::LineString<f64>);

#[wasm_bindgen]
impl LineString {
    #[wasm_bindgen(constructor)]
    pub fn new(coords: Vec<Coord>) -> LineString {
        let converted = geo::LineString::new(coords.into_iter().map(|x| x.into()).collect());
        LineString(converted)
    }
}

impl From<LineString> for geo::LineString<f64> {
    fn from(value: LineString) -> Self {
        value.0
    }
}

impl From<geo::LineString<f64>> for LineString {
    fn from(value: geo::LineString<f64>) -> LineString {
        LineString(value.into())
    }
}
