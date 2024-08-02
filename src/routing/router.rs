use crate::debug::debug_log;
use crate::geo_types::{LineString, Point};
use crate::routing::{Route, RouteSegment};
use ::geo::Closest;
use ::geo::ClosestPoint;
use ::geo::EuclideanDistance;
use ::geo::EuclideanLength;
use ::geo::LineInterpolatePoint;
use ::geo::LineLocatePoint;
use geo::geometry as geo;
use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashMap};
use thiserror::Error;
use wasm_bindgen::prelude::*;

#[derive(Debug, Clone)]
#[wasm_bindgen]
/// A connector in the transport network.
pub struct Connector {
    id: String,
    point: Point,
}

#[wasm_bindgen]
impl Connector {
    #[wasm_bindgen(constructor)]
    pub fn new(id: &str, point: &Point) -> Connector {
        Connector {
            id: id.into(),
            point: point.clone(),
        }
    }

    pub fn get_id(&self) -> String {
        self.id.clone()
    }

    pub fn get_point(&self) -> Point {
        self.point.clone()
    }
}

#[derive(Debug, Clone)]
#[wasm_bindgen]
/// A segment in the transport network.
pub struct Segment {
    id: String,
    geometry: LineString,
    /// List of connectors which are part of the segment.
    connectors: Vec<String>,
}

#[wasm_bindgen]
impl Segment {
    #[wasm_bindgen(constructor)]
    pub fn new(id: String, geometry: LineString, connectors: Vec<String>) -> Segment {
        console_error_panic_hook::set_once();
        Segment {
            id,
            geometry,
            connectors,
        }
    }

    pub fn get_id(&self) -> String {
        return self.id.clone();
    }

    pub fn get_geometry(&self) -> LineString {
        return self.geometry.clone();
    }

    fn get_connectors(&self) -> &Vec<String> {
        return &self.connectors;
    }

    /// Returns the linear position of the given point on this segment.
    fn get_point_position(&self, point: &Point) -> Option<f64> {
        let geo_line_string = Into::<geo::LineString<f64>>::into(self.geometry.clone());
        let geo_point = &Into::<geo::Point<f64>>::into(point.clone());
        let position = geo_line_string.line_locate_point(&geo_point);
        debug_log!(
            "point position {:?} for linestring: {:?}, point: {:?}",
            position,
            self.get_geometry(),
            point
        );
        position
    }
}

pub type Position = f64;

#[derive(Debug)]
/// A segment with a linear position on it.
pub struct SegmentWithPosition<'a> {
    segment: &'a Segment,
    position: Position,
}

impl<'a> SegmentWithPosition<'a> {
    pub fn get_segment(&self) -> &Segment {
        self.segment
    }

    pub fn get_position(&self) -> Position {
        self.position
    }

    /// Returns the position on the segment as point.
    pub fn get_position_as_point(&self) -> Point {
        Into::<geo::LineString<f64>>::into(self.segment.get_geometry())
            .line_interpolate_point(self.position)
            .unwrap()
            .into()
    }
}

#[derive(Debug)]
enum Error {}

#[derive(Debug)]
#[wasm_bindgen]
pub struct Router {
    segments: Vec<Segment>,
    connectors: Vec<Connector>,
}

#[wasm_bindgen]
impl Router {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Router {
        Router {
            segments: Vec::new(),
            connectors: Vec::new(),
        }
    }
}

#[derive(Copy, Clone, Eq, PartialEq)]
struct ToVisitState<'a> {
    cost: u32,
    connector_id: &'a String,
}
impl<'a> Ord for ToVisitState<'a> {
    fn cmp(&self, other: &Self) -> Ordering {
        // Notice that we flip the ordering on costs.
        // In case of a tie we compare positions - this step is necessary
        // to make implementations of `PartialEq` and `Ord` consistent.
        other
            .cost
            .cmp(&self.cost)
            .then_with(|| self.connector_id.cmp(&other.connector_id))
    }
} // `PartialOrd` needs to be implemented as well.
impl<'a> PartialOrd for ToVisitState<'a> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

#[wasm_bindgen]
impl Router {
    #[wasm_bindgen(js_name = segmentsLength)]
    /// Returns number of stored segments.
    pub fn segments_len(&self) -> usize {
        self.segments.len()
    }

    #[wasm_bindgen(js_name = connectorsLength)]
    /// Returns number of stored connectors.
    pub fn connectors_len(&self) -> usize {
        self.connectors.len()
    }

    #[wasm_bindgen(js_name = toGeoJSON)]
    /// Returns the transport network (segments and connectors) as GeoJSON
    /// feature collection.
    pub fn to_geojson(&self) -> String {
        let mut feature_strs = Vec::new();
        for segment in &self.segments {
            let linestring = Into::<geo::LineString<f64>>::into(segment.get_geometry().clone());
            let mut coordinates_str = String::new();
            for coordinate in linestring {
                if !coordinates_str.is_empty() {
                    coordinates_str.push_str(", ");
                }
                coordinates_str.push_str(&format!("[{}, {}]", coordinate.x, coordinate.y));
            }
            feature_strs.push(format!(
                r#"{{
            "type": "Feature",
            "id": "{}",
            "geometry": {{
                "type": "LineString",
                "coordinates": [{}]
            }},
            "properties": {{}}
        }}"#,
                segment.get_id(),
                coordinates_str
            ));
        }
        format!(
            r#"{{ "type": "FeatureCollection", "features": [{}] }}"#,
            feature_strs.join(",")
        )
    }

    #[wasm_bindgen(js_name = findRoute)]
    /// Find a route from start to stop.
    pub fn find_route(&self, start: &Point, stop: &Point) -> Result<Route, RoutingError> {
        debug_log!("find route for start {:?}, stop {:?}", start, stop);
        if self.segments_len() == 0 {
            return Err(RoutingError::MissingSegments);
        }
        let start_segment = self.find_nearest(start).unwrap();
        let stop_segment = self.find_nearest(stop).unwrap();

        let start_connector = Connector {
            id: "#start".into(),
            point: start_segment.get_position_as_point(),
        };
        let stop_connector = Connector {
            id: "#stop".into(),
            point: stop_segment.get_position_as_point(),
        };
        let (mut connector_map, _) = self.build_maps(
            &start_segment,
            &stop_segment,
            &start_connector,
            &stop_connector,
        );

        let mut to_visit = BinaryHeap::new();

        to_visit.push(ToVisitState {
            cost: 0,
            connector_id: &start_connector.id,
        });
        connector_map
            .get_mut(&start_connector.get_id())
            .expect(&format!(
                "Starting connector {} is missing in map",
                start_connector.get_id()
            ))
            .distance = Some(0.0);
        while to_visit.len() > 0 {
            let visiting = connector_map
                .get(to_visit.pop().unwrap().connector_id)
                .unwrap()
                .connector;
            // debug_log!("Visiting {}", visiting.get_id());
            if visiting.id == stop_connector.get_id() {
                debug_log!("Found way to stop connector!");
                break;
            }
            let visiting_data = (*connector_map.get(&visiting.id).unwrap()).clone();
            // debug_log!("Data {:?}", visiting_data);
            for neighbour in &visiting_data.neighbours {
                // debug_log!("Checking neigbour {}", neighbour.connector.get_id());
                let old_neighbour_data = connector_map.get(&neighbour.connector.id).unwrap();
                let new_distance = visiting_data.distance.unwrap()
                    + Into::<geo::LineString<f64>>::into(neighbour.segment.get_geometry())
                        .euclidean_length();
                let priority = new_distance
                    + Into::<geo::Point<f64>>::into(neighbour.connector.get_point())
                        .euclidean_distance(&Into::<geo::Point<f64>>::into(
                            stop_connector.get_point(),
                        ));
                if old_neighbour_data
                    .distance
                    .is_some_and(|x| x <= new_distance)
                {
                    continue;
                }
                // debug_log!(
                // "Found shorter way for {} coming from {}",
                // neighbour.connector.get_id(), visiting.get_id()
                // );
                let data = connector_map.get_mut(&neighbour.connector.id).unwrap();
                data.distance = Some(new_distance);
                data.previous_segment = Some(neighbour.segment);
                data.previous_connector = Some(visiting);
                to_visit.push(ToVisitState {
                    cost: (priority * 1000.0).round() as u32,
                    connector_id: &neighbour.connector.id,
                });
            }
        }
        let mut route_segments = Vec::new();
        let mut current_connector = connector_map.get(&stop_connector.get_id()).unwrap();
        if current_connector.previous_connector.is_none() {
            return Err(RoutingError::CouldNotFindRoute);
        };
        loop {
            debug_log!(
                "Way back: {:?} through connector {:?}",
                current_connector.previous_segment,
                current_connector.previous_connector,
            );
            let start_position = match &current_connector.previous_connector {
                Some(&ref connector) => current_connector
                    .previous_segment
                    .unwrap()
                    .get_point_position(&connector.point)
                    .unwrap(),
                None => start_segment.position,
            };

            let stop_position = current_connector
                .previous_segment
                .unwrap()
                .get_point_position(&current_connector.connector.point);

            route_segments.push(RouteSegment::new(
                current_connector.previous_segment.unwrap(),
                start_position,
                stop_position.unwrap(),
            ));

            current_connector = connector_map
                .get(&current_connector.previous_connector.unwrap().id)
                .unwrap();

            if current_connector.previous_connector.is_none() {
                debug_log!("found way back to start");
                break;
            }
        }
        let last_segment = route_segments.pop().unwrap();
        route_segments.push(RouteSegment::new(
            &last_segment.get_segment(),
            start_segment.get_position(),
            last_segment.get_stop(),
        ));
        route_segments.reverse();
        debug_log!("segments {:?}", route_segments);
        Ok(Route::new(
            vec![start.clone(), stop.clone()],
            route_segments,
        ))
    }
}

#[derive(Clone, Debug)]
struct ConnectorNeighbour<'a> {
    connector: &'a Connector,
    segment: &'a Segment,
}

#[derive(Clone, Debug)]
struct ConnectorData<'a> {
    connector: &'a Connector,
    distance: Option<f64>,
    neighbours: Vec<ConnectorNeighbour<'a>>,
    previous_segment: Option<&'a Segment>,
    previous_connector: Option<&'a Connector>,
}

impl Router {
    pub fn push_segment(&mut self, segment: Segment) {
        self.segments.push(segment);
    }

    pub fn push_connector(&mut self, connector: Connector) {
        self.connectors.push(connector);
    }

    /// Returns the position of the segment that is nearest to the given point.
    ///
    /// Returns None if there are no segments at all.
    pub fn find_nearest<'a>(&'a self, point: &Point) -> Option<SegmentWithPosition<'a>> {
        debug_log!("find nearest for point {:?}", point);
        let mut shortest_distance: f64 = std::f64::MAX;
        let mut nearest_segment = None;
        let mut position: f64 = 0.0;
        for segment in &self.segments {
            let geo_line_string = Into::<geo::LineString<f64>>::into(segment.geometry.clone());
            let geo_point = &Into::<geo::Point<f64>>::into(point.clone());
            let distance = geo_line_string.euclidean_distance(geo_point);
            if distance < shortest_distance {
                shortest_distance = distance;
                nearest_segment = Some(segment);
                let closest_point = geo_line_string.closest_point(geo_point);
                match closest_point {
                    Closest::Intersection(closest) | Closest::SinglePoint(closest) => {
                        position = geo_line_string.line_locate_point(&closest).unwrap();
                    }
                    Closest::Indeterminate => {
                        panic!("unimplemented")
                    }
                }
            }
        }
        match nearest_segment {
            Some(segment) => {
                let it = Some(SegmentWithPosition { segment, position });
                debug_log!("found nearest {:?}", it);
                return it;
            }
            None => None,
        }
    }

    fn build_maps<'a>(
        &'a self,
        start_segment: &'a SegmentWithPosition,
        stop_segment: &'a SegmentWithPosition,
        start_connector: &'a Connector,
        stop_connector: &'a Connector,
    ) -> (HashMap<String, ConnectorData>, HashMap<&String, &Segment>) {
        let mut connector_map = HashMap::with_capacity(self.connectors.len());
        for connector in &self.connectors {
            connector_map.insert(
                connector.id.clone(),
                ConnectorData {
                    connector,
                    distance: None,
                    neighbours: Vec::new(),
                    previous_segment: Some(start_segment.get_segment()),
                    previous_connector: None,
                },
            );
        }
        connector_map.insert(
            start_connector.get_id(),
            ConnectorData {
                connector: &start_connector,
                distance: None,
                neighbours: Vec::new(),
                previous_segment: Some(start_segment.get_segment()),
                previous_connector: None,
            },
        );
        connector_map.insert(
            stop_connector.get_id(),
            ConnectorData {
                connector: &stop_connector,
                distance: None,
                neighbours: Vec::new(),
                previous_segment: Some(start_segment.get_segment()),
                previous_connector: None,
            },
        );

        let mut segment_map = HashMap::with_capacity(self.segments.len());
        for segment in &self.segments {
            segment_map.insert(&segment.id, segment);
            let mut connectors = segment.get_connectors().clone();
            if segment.get_id() == start_segment.get_segment().get_id() {
                connectors.push(start_connector.get_id());
            }
            if segment.get_id() == stop_segment.get_segment().get_id() {
                connectors.push(stop_connector.get_id());
            }
            for connector_id in &connectors {
                if !connector_map.contains_key(connector_id) {
                    // Ignore unknown connectors.
                    continue;
                }
                let new_neighbours: Vec<ConnectorNeighbour> = connectors
                    .clone()
                    .iter()
                    .filter_map(|x| {
                        if x == connector_id {
                            return None;
                        }
                        match connector_map.get(x) {
                            Some(neighbour) => Some(ConnectorNeighbour {
                                connector: neighbour.connector,
                                segment,
                            }),
                            None => None, // Ignore unknown connectors.
                        }
                    })
                    .collect();

                connector_map
                    .get_mut(connector_id)
                    .unwrap()
                    .neighbours
                    .extend(new_neighbours.into_iter());
            }
        }
        (connector_map, segment_map)
    }
}

#[derive(Error, Debug, PartialEq, Eq)]
#[wasm_bindgen]
pub enum RoutingError {
    #[error("No segments added to router.")]
    MissingSegments,
    #[error("Could not fetch tile")]
    TileFetchingError,
    #[error("Could not parse tile")]
    TileParsingError,
    #[error("Could not find route")]
    CouldNotFindRoute,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geo_types::coord::{coord, Coord};

    #[test]
    /// General tests.
    fn genereal() {
        let router = Router::new();
        assert_eq!(router.segments.len(), 0);
        assert_eq!(router.connectors.len(), 0);
    }

    #[test]
    /// Test find_nearest method.
    fn find_nearest() {
        let mut router = Router::new();
        assert_eq!(router.find_nearest(&Point::new(0.0, 0.0)).is_none(), true);
        router.push_segment(Segment::new(
            "a".into(),
            LineString::new(vec![
                coord!( x: 0.0, y: 0.0 ),
                coord!( x: 1.0, y: 1.0 ),
                coord!( x: 1.0, y: 2.0 ),
            ]),
            vec![],
        ));
        router.push_segment(Segment::new(
            "b".into(),
            LineString::new(vec![
                coord!( x: 2.0, y: 3.0 ),
                coord!( x: 2.0, y: 2.0 ),
                coord!( x: 3.0, y: 1.0 ),
                coord!( x: 3.0, y: 0.0 ),
            ]),
            vec![],
        ));
        router.push_segment(Segment::new(
            "c".into(),
            LineString::new(vec![
                coord!( x: 4.0, y: 1.0 ),
                coord!( x: 4.0, y: 0.0 ),
                coord!( x: 5.0, y: 0.0 ),
            ]),
            vec![],
        ));
        {
            let nearest = router.find_nearest(&Point::new(0.0, 2.0)).unwrap();
            assert_eq!(nearest.position, 1.0);
            assert_eq!(nearest.segment.id, "a");
        }
        {
            let nearest = router.find_nearest(&Point::new(2.0, 1.0)).unwrap();
            assert_eq!(nearest.position, 0.5);
            assert_eq!(nearest.segment.id, "b");
        }
        {
            let nearest = router.find_nearest(&Point::new(5.0, 1.0)).unwrap();
            assert_eq!(nearest.position, 1.0);
            assert_eq!(nearest.segment.id, "c");
        }
    }

    #[test]
    /// Test find_route method.
    fn find_route_away_from_points() {
        let mut router = Router::new();
        router.push_segment(Segment::new(
            "1".into(),
            LineString::new(vec![coord!( x: 1.0, y: 0.0 ), coord!( x: 9.0, y: 0.0 )]),
            vec![],
        ));
        let route = router
            .find_route(&Point::new(0.0, 0.0), &Point::new(10.0, 0.0))
            .unwrap();
        assert_eq!(route.get_segments().len(), 1);
        let segment = &route.get_segments()[0];
        assert_eq!(segment.get_segment().get_id(), "1");
        assert_eq!(segment.get_start(), 0.0);
        assert_eq!(segment.get_stop(), 1.0);
    }

    #[test]
    fn find_route_no_route() {
        let mut router = Router::new();
        router.push_segment(Segment::new(
            "1".into(),
            LineString::new(vec![coord!( x: 1.0, y: 0.0 ), coord!( x: 4.0, y: 0.0 )]),
            vec![],
        ));
        router.push_segment(Segment::new(
            "2".into(),
            LineString::new(vec![coord!( x: 5.0, y: 0.0 ), coord!( x: 8.0, y: 0.0 )]),
            vec![],
        ));
        let route = router.find_route(&Point::new(0.0, 0.0), &Point::new(10.0, 0.0));
        assert_eq!(route.err().unwrap(), RoutingError::CouldNotFindRoute);
    }

    #[test]
    fn find_route_away_from_start() {
        let mut router = Router::new();
        router.push_connector(Connector {
            id: "a".to_string(),
            point: Point::new(3.0, 0.0),
        });
        router.push_connector(Connector {
            id: "b".to_string(),
            point: Point::new(6.0, 0.0),
        });
        router.push_segment(Segment::new(
            "1".into(),
            LineString::new(vec![coord!( x: 1.0, y: 0.0 ), coord!( x: 4.0, y: 0.0 )]),
            vec!["a".into(), "b".into()],
        ));
        router.push_segment(Segment::new(
            "2".into(),
            LineString::new(vec![coord!( x: 5.0, y: 0.0 ), coord!( x: 8.0, y: 0.0 )]),
            vec!["a".into(), "b".into()],
        ));
        let route = router
            .find_route(&Point::new(0.0, 0.0), &Point::new(10.0, 0.0))
            .unwrap();
        assert_eq!(route.get_segments().len(), 2);
        {
            let segment = &route.get_segments()[0];
            assert_eq!(segment.get_segment().get_id(), "1");
            assert_eq!(segment.get_start(), 0.0);
            assert_eq!(segment.get_stop(), 1.0);
        }
        {
            let segment = &route.get_segments()[1];
            assert_eq!(segment.get_segment().get_id(), "2");
            assert_eq!(segment.get_start(), 1.0 / 3.0);
            assert_eq!(segment.get_stop(), 1.0);
        }
    }

    #[test]
    /// Test find_route method.
    fn find_route_single_segment() {
        let mut router = Router::new();
        router.push_connector(Connector {
            id: "a".to_string(),
            point: Point::new(0.0, 0.0),
        });
        router.push_connector(Connector {
            id: "b".to_string(),
            point: Point::new(10.0, 0.0),
        });
        router.push_segment(Segment::new(
            "1".into(),
            LineString::new(vec![coord!( x: 0.0, y: 0.0 ), coord!( x: 10.0, y: 0.0 )]),
            vec!["a".to_string()],
        ));
        let route = router
            .find_route(&Point::new(3.0, 0.0), &Point::new(6.0, 0.0))
            .unwrap();
        assert_eq!(route.get_segments().len(), 1);
        let segment = &route.get_segments()[0];
        assert_eq!(segment.get_segment().get_id(), "1");
        assert_eq!(segment.get_start(), 0.3);
        assert_eq!(segment.get_stop(), 0.6);
    }

    #[test]
    /// Test find_route method.
    fn find_route() {
        let mut router = Router::new();
        router.push_connector(Connector {
            id: "a".to_string(),
            point: Point::new(2.0, 0.0),
        });
        router.push_connector(Connector {
            id: "b".to_string(),
            point: Point::new(3.0, 3.0),
        });
        router.push_connector(Connector {
            id: "c".to_string(),
            point: Point::new(2.0, 4.0),
        });
        router.push_connector(Connector {
            id: "d".to_string(),
            point: Point::new(3.0, 5.0),
        });
        router.push_segment(Segment::new(
            "1".into(),
            LineString::new(vec![coord!( x: 0.0, y: 0.0 ), coord!( x: 4.0, y: 0.0 )]),
            vec!["a".to_string()],
        ));
        router.push_segment(Segment::new(
            "2".into(),
            LineString::new(vec![
                coord!( x: 3.0, y: 3.0 ),
                coord!( x: 3.0, y: 4.0 ),
                coord!( x: 2.0, y: 4.0 ),
            ]),
            vec!["b".to_string(), "c".to_string()],
        ));
        router.push_segment(Segment::new(
            "3".into(),
            LineString::new(vec![
                coord!( x: 2.0, y: 0.0 ),
                coord!( x: 2.0, y: 2.0 ),
                coord!( x: 3.0, y: 2.0 ),
                coord!( x: 3.0, y: 1.0 ),
                coord!( x: 4.0, y: 1.0 ),
                coord!( x: 4.0, y: 3.0 ),
                coord!( x: 3.0, y: 3.0 ),
            ]),
            vec!["a".to_string(), "b".to_string()],
        ));
        router.push_segment(Segment::new(
            "4".into(),
            LineString::new(vec![
                coord!( x: 2.0, y: 4.0 ),
                coord!( x: 2.0, y: 4.5 ),
                coord!( x: 3.5, y: 4.5 ),
            ]),
            vec!["c".to_string(), "d".to_string()],
        ));
        {
            let route = router
                .find_route(&Point::new(0.5, 1.0), &Point::new(2.5, 5.0))
                .unwrap();
            let segments = route.get_segments();
            assert_eq!(route.get_segments().len(), 4);
            {
                let route_segment = &segments[0];
                let segment = route_segment.get_segment();
                assert_eq!(segment.id, "1");
                assert_eq!(route_segment.get_start(), 0.125);
                assert_eq!(route_segment.get_stop(), 0.5);
            }
            {
                let route_segment = &segments[1];
                let segment = route_segment.get_segment();
                assert_eq!(segment.id, "3");
                assert_eq!(route_segment.get_start(), 0.0);
                assert_eq!(route_segment.get_stop(), 1.0);
            }
            {
                let route_segment = &segments[2];
                let segment = route_segment.get_segment();
                assert_eq!(segment.id, "2");
                assert_eq!(route_segment.get_start(), 0.0);
                assert_eq!(route_segment.get_stop(), 1.0);
            }
            {
                let route_segment = &segments[3];
                let segment = route_segment.get_segment();
                assert_eq!(segment.id, "4");
                assert_eq!(route_segment.get_start(), 0.0);
                assert_eq!(route_segment.get_stop(), 0.5);
            }
        }
    }
}
