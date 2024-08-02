use wasm_bindgen::{prelude::*, JsValue};

use super::{Backend, Coord};
use crate::debug::debug_log;
use crate::geo_types::Point;
use crate::routing::{Connector, Router, Segment};
use mercantile::LngLatBbox;
use mvt_reader::Reader;
use std::convert::TryFrom;
use thiserror::Error;

#[wasm_bindgen(module = "pmtiles")]
extern "C" {
    type PMTiles;

    #[wasm_bindgen(constructor)]
    fn new(url: String) -> PMTiles;

    #[wasm_bindgen(method, js_name = getZxy)]
    fn get_zxy(this: &PMTiles, z: u8, x: u32, y: u32) -> JsValue;
}

pub struct Tile {
    data: Vec<u8>,
    coord: Coord,
}

impl super::Tile for Tile {
    fn parse(&self, router: &mut Router) -> Result<(), Box<dyn std::error::Error>> {
        Ok(parse_mvt_buffer(router, &self.data, &self.coord, false)?)
    }
}

pub struct PMTilesMVTBackend {
    pm_tiles: PMTiles,
}

impl PMTilesMVTBackend {
    pub fn new(url: &str) -> Self {
        PMTilesMVTBackend {
            pm_tiles: PMTiles::new(url.into()),
        }
    }
}

#[derive(Error, Debug)]
enum FetchingError {
    #[error("Could not find tile")]
    TileNotFound,
}

impl Backend<Tile> for PMTilesMVTBackend {
    async fn get_tile(&self, coord: &Coord) -> Result<Tile, Box<dyn std::error::Error>> {
        debug_log!("get tile {:?}", coord);
        let promise = js_sys::Promise::from(self.pm_tiles.get_zxy(coord.z, coord.x, coord.y));
        wasm_bindgen_futures::JsFuture::from(promise)
            .await
            .and_then(|inside| js_sys::Reflect::get(&inside, &JsValue::from(String::from("data"))))
            .and_then(|data| {
                Ok(Tile {
                    data: js_sys::Uint8Array::new(&data).to_vec(),
                    coord: coord.clone(),
                })
            })
            .or(Err(FetchingError::TileNotFound.into()))
    }
}

#[derive(Error, Debug)]
enum ParsingError {
    #[error("Could not parse MVT tile")]
    MVTError,
    #[error("Connector with id `{connector_id:?}` is invalid: {context}")]
    InvalidConnector {
        connector_id: String,
        context: String,
    },
    #[error("Segment with id `{segment_id:?}` is invalid: {context}")]
    InvalidSegment { segment_id: String, context: String },
    #[error("Missing ID")]
    InvalidID,
}

fn parse_connectors(
    segments: &mut Router,
    reader: &Reader,
    extent: f64,
    bbox: &LngLatBbox,
    strict: bool,
) -> Result<(), ParsingError> {
    for feature in reader.get_features(0).unwrap() {
        let id = feature
            .properties
            .as_ref()
            .and_then(|p| p.get("id"))
            .ok_or(ParsingError::InvalidID)?
            .to_string();
        let point = match geo::MultiPoint::<f32>::try_from(feature.geometry) {
            Ok(p) => p.into_iter().next(),
            Err(err) => {
                let err = ParsingError::InvalidConnector {
                    connector_id: id.clone(),
                    context: format!("Could not parse geometry {:?} for connector {}", err, id),
                };
                if strict {
                    return Err(err);
                } else {
                    debug_log!("{}", err);
                    continue;
                }
            }
        };
        match point {
            Some(point) => {
                let x = bbox.west + point.x() as f64 / extent * (bbox.east - bbox.west);
                let y = bbox.north + point.y() as f64 / extent * (bbox.south - bbox.north);
                segments.push_connector(Connector::new(id.as_str(), &Point::new(x, y)));
            }
            None => {
                let err = ParsingError::InvalidConnector {
                    connector_id: id.clone(),
                    context: format!("Empty geometry for connector {}", id),
                };
                if strict {
                    return Err(err);
                } else {
                    debug_log!("{}", err);
                }
            }
        }
    }
    Ok(())
}

fn parse_segments(
    segments: &mut Router,
    reader: &Reader,
    extent: f64,
    bbox: &LngLatBbox,
    _strict: bool,
) -> Result<(), ParsingError> {
    for feature in reader.get_features(1).unwrap() {
        let id = feature.properties.as_ref().unwrap().get("id").unwrap();
        if geo::MultiLineString::<f32>::try_from(feature.geometry.clone()).is_ok() {
            continue;
        }
        let coords = geo::LineString::<f32>::try_from(feature.geometry)
            .unwrap()
            .into_inner();
        let geometry: geo::LineString<f64> = coords
            .iter()
            .map(|coord| geo::Coord {
                x: bbox.west + coord.x as f64 / extent * (bbox.east - bbox.west),
                y: bbox.north + coord.y as f64 / extent * (bbox.south - bbox.north),
            })
            .collect();
        let connector_ids: Vec<String> = feature
            .properties
            .as_ref()
            .and_then(|p| p.get("connector_ids"))
            .and_then(|ids| serde_json::from_str(ids).ok())
            .ok_or(ParsingError::InvalidSegment {
                segment_id: id.clone(),
                context: "Connector ids missing or invalid".into(),
            })?;
        let segment = Segment::new(id.clone(), geometry.into(), connector_ids);
        segments.push_segment(segment);
    }
    Ok(())
}

// Parses the given MVT tile and adds the included segments and connectors to
// the router.
fn parse_mvt_buffer(
    router: &mut Router,
    buffer: &Vec<u8>,
    coord: &Coord,
    strict: bool,
) -> Result<(), ParsingError> {
    let tile = mercantile::Tile::new(
        i32::try_from(coord.x).unwrap(),
        i32::try_from(coord.y).unwrap(),
        i32::try_from(coord.z).unwrap(),
    );
    let bbox = mercantile::bounds(tile);
    let extent: f64 = 4096.0;
    let reader = Reader::new(buffer.to_vec()).map_err(|_| ParsingError::MVTError)?;
    parse_connectors(router, &reader, extent, &bbox, strict)?;
    parse_segments(router, &reader, extent, &bbox, strict)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::Coord;

    #[test]
    /// Test find_route method.
    fn parse_mvt_buffer() {
        let mut tile = mvt::Tile::new(4096);
        {
            let layer = tile.create_layer("connectors");
            let b = mvt::GeomEncoder::new(mvt::GeomType::Point)
                .point(0.0, 0.0)
                .unwrap()
                .encode()
                .unwrap();
            let mut feature = layer.into_feature(b);
            feature.set_id(1);
            feature.add_tag_string("id", "foo");
            let layer = feature.into_layer();
            tile.add_layer(layer).unwrap();
        }
        {
            let layer = tile.create_layer("segments");
            let b = mvt::GeomEncoder::new(mvt::GeomType::Linestring)
                .point(0.0, 0.0)
                .unwrap()
                .point(1024.0, 0.0)
                .unwrap()
                .point(1024.0, 2048.0)
                .unwrap()
                .point(4096.0, 4096.0)
                .unwrap()
                .encode()
                .unwrap();
            let mut feature = layer.into_feature(b);
            feature.set_id(1);
            feature.add_tag_string("id", "foo");
            feature.add_tag_string("connector_ids", "[\"foo\"]");
            let layer = feature.into_layer();
            tile.add_layer(layer).unwrap();
        }
        let data = tile.to_bytes().unwrap();
        let mut router = crate::routing::Router::new();
        super::parse_mvt_buffer(&mut router, &data, &Coord { x: 0, y: 0, z: 0 }, true).unwrap();
        assert_eq!(1, router.segments_len());
        assert_eq!(1, router.connectors_len());
    }
}
