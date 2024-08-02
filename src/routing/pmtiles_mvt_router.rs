use wasm_bindgen::prelude::*;

use crate::debug::debug_log;
use crate::geo_types::Point;
use crate::routing::{Route, RoutingError};
use crate::tile::backend::pmtiles_mvt_backend::{PMTilesMVTBackend, Tile};
use crate::tile::backend::CachedTileNetwork;

#[wasm_bindgen]
/// A router using Mapbox Vector Tiles insiden an PMTiles container.
pub struct PMTilesMVTRouter {
    network: CachedTileNetwork<PMTilesMVTBackend, Tile>,
}

#[wasm_bindgen]
impl PMTilesMVTRouter {
    #[wasm_bindgen(constructor)]
    /// Create the router using the given PMTiles URL.
    pub fn new(url: &str) -> PMTilesMVTRouter {
        let backend = PMTilesMVTBackend::new(url);
        PMTilesMVTRouter {
            network: CachedTileNetwork::new(backend),
        }
    }

    #[wasm_bindgen(js_name = findRoute)]
    /// Find a route for the given start and stop points.
    pub async fn find_route(&mut self, start: &Point, stop: &Point) -> Result<Route, RoutingError> {
        debug_log!("PMTilesMVTRouter::find_route {:?}, {:?}", start, stop);
        self.network.find_route(start, stop).await
    }
}
