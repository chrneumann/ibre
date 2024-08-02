use crate::debug::debug_log;
use crate::geo_types::Point;
use crate::routing::router::Segment;
use ::geo::{LineInterpolatePoint, LineLocatePoint};
use wasm_bindgen::prelude::*;

#[derive(Debug, Clone)]
#[wasm_bindgen]
/// A route segment.
///
/// A found route consists of these.
pub struct RouteSegment {
    /// The segment.
    segment: Segment,
    /// The start position on this segment (0..1).
    start: f64,
    /// The end position on this segment (0..1).
    stop: f64,
}

#[wasm_bindgen]
impl RouteSegment {
    #[wasm_bindgen(constructor)]
    pub fn new(segment: &Segment, start: f64, stop: f64) -> RouteSegment {
        RouteSegment {
            segment: (*segment).clone(),
            start,
            stop,
        }
    }

    pub fn get_segment(&self) -> Segment {
        self.segment.clone()
    }

    pub fn get_start(&self) -> f64 {
        self.start
    }

    pub fn get_stop(&self) -> f64 {
        self.stop
    }

    /// Cuts the geometry of the segment at the start and stop positions.
    fn get_cutted_geometry(&self) -> geo::LineString<f64> {
        let linestring = Into::<geo::LineString<f64>>::into(self.segment.get_geometry().clone());
        let (start, stop) = if self.start > self.stop {
            (self.stop, self.start)
        } else {
            (self.start, self.stop)
        };

        let starting_point = linestring.line_interpolate_point(start).unwrap();
        let stopping_point = linestring.line_interpolate_point(stop).unwrap();

        debug_log!("cut geometry {:?} at {:?}, {:?}", linestring, start, stop);
        let coords: Vec<_> = linestring.clone().into_inner();
        let mut filtered: Vec<_> = coords
            .into_iter()
            .filter(|coord| {
                let point = geo::Point::new(coord.x, coord.y);
                let position = linestring.line_locate_point(&point).unwrap();
                let filter = position >= start && position <= stop;
                debug_log!(
                    "point {:?}, position {:?}. filtered? {:?}",
                    point,
                    position,
                    filter
                );
                filter
            })
            .collect();
        if self.start != 0.0 {
            filtered.insert(0, starting_point.0);
        }
        if self.stop != 1.0 {
            filtered.push(stopping_point.0);
        }
        let coords_cloned: Vec<geo::Coord<f64>> =
            filtered.into_iter().map(|coord| coord.clone()).collect();

        let new = geo::LineString::new(coords_cloned);
        debug_log!("new geometry {:?}", new);
        new
    }

    /// Returns a GeoJSON feature representation of the route segment.
    pub fn to_geojson(&self) -> String {
        let mut coordinates_str = String::new();
        for coordinate in self.get_cutted_geometry() {
            if !coordinates_str.is_empty() {
                coordinates_str.push_str(", ");
            }
            coordinates_str.push_str(&format!("[{}, {}]", coordinate.x, coordinate.y));
        }
        format!(
            r#"{{
            "type": "Feature",
            "id": "{}",
            "geometry": {{
                "type": "LineString",
                "coordinates": [{}]
            }},
            "properties": {{}}
        }}"#,
            self.segment.get_id(),
            coordinates_str
        )
    }
}

#[derive(Debug, Clone)]
#[wasm_bindgen]
/// A calculated route.
pub struct Route {
    /// Stops of the route; First is the start, last is the finish.
    stops: Vec<Point>,
    /// Calculated segments.
    segments: Vec<RouteSegment>,
}

#[wasm_bindgen]
impl Route {
    #[wasm_bindgen(constructor)]
    pub fn new(stops: Vec<Point>, segments: Vec<RouteSegment>) -> Route {
        Route {
            stops: stops.clone(),
            segments: segments.clone(),
        }
    }

    pub fn get_stops(&self) -> Vec<Point> {
        self.stops.clone()
    }

    pub fn get_segments(&self) -> Vec<RouteSegment> {
        self.segments.clone()
    }

    /// Returns the route as a GeoJSON collection of its segments.
    pub fn get_segments_as_geojson(&self) -> String {
        let mut features = Vec::new();
        for segment in &self.segments {
            features.push(segment.to_geojson());
        }
        format!(
            r#"
                {{
                    "type": "FeatureCollection",
                    "features": [{}]
                }}"#,
            features.join(",")
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geo_types::coord::{coord, Coord};
    use crate::geo_types::LineString;

    #[test]
    pub fn get_cutted_geometry() {
        let mut segment = RouteSegment::new(
            &Segment::new(
                "foo".into(),
                LineString::new(vec![
                    coord!(x: 0.0, y: 0.0),
                    coord!(x: 6.0, y: 0.0),
                    coord!(x: 7.0, y: 0.0),
                    coord!(x: 10.0, y: 0.0),
                ]),
                Vec::new(),
            ),
            0.35,
            0.75,
        );
        {
            let cutted = segment.get_cutted_geometry();
            assert_eq!(cutted.0[0], coord!(x: 3.5, y: 0.0).into());
            assert_eq!(cutted.0[1], coord!(x: 6.0, y: 0.0).into());
            assert_eq!(cutted.0[2], coord!(x: 7.0, y: 0.0).into());
            assert_eq!(cutted.0[3], coord!(x: 7.5, y: 0.0).into());
            assert_eq!(cutted.0.len(), 4);
        }
        segment.start = 0.75;
        segment.stop = 0.35;
        {
            let cutted = segment.get_cutted_geometry();
            assert_eq!(cutted.0[0], coord!(x: 3.5, y: 0.0).into());
            assert_eq!(cutted.0[1], coord!(x: 6.0, y: 0.0).into());
            assert_eq!(cutted.0[2], coord!(x: 7.0, y: 0.0).into());
            assert_eq!(cutted.0[3], coord!(x: 7.5, y: 0.0).into());
            assert_eq!(cutted.0.len(), 4);
        }
    }

    #[test]
    // Tests problems from rounding errors.
    pub fn get_cutted_geometry_rounding_errors() {
        let segment = RouteSegment::new(
            &Segment::new(
                "foo".into(),
                LineString::new(vec![
                    coord!(x: 8.682461, y: 50.123024),
                    coord!(x: 8.682504, y: 50.123795),
                ]),
                Vec::new(),
            ),
            0.09508603,
            0.49503046,
        );
        let cutted = segment.get_cutted_geometry();
        assert_eq!(cutted.0.len(), 2);
    }
}
