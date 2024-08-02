use wasm_bindgen::prelude::*;

#[derive(Debug, Clone)]
#[wasm_bindgen]
/// A single point in 2D space.
pub struct Point(geo::Point<f64>);

#[wasm_bindgen]
impl Point {
    #[wasm_bindgen(constructor)]
    pub fn new(x: f64, y: f64) -> Point {
        let point: geo::Point<f64> = (x, y).into();
        Point::from(point)
    }

    pub fn x(&self) -> f64 {
        self.0.x()
    }

    pub fn y(&self) -> f64 {
        self.0.y()
    }
}

impl From<geo::Point<f64>> for Point {
    fn from(value: geo::Point<f64>) -> Point {
        Point(value)
    }
}

impl From<Point> for geo::Point<f64> {
    fn from(value: Point) -> Self {
        value.0
    }
}
