use crate::debug::debug_log;
use crate::geo_types::Point;
use crate::routing::{Route, Router, RoutingError};
use crate::tile;
use crate::tile::backend::{Backend, Tile};
use crate::tile::point_to_tile_coord;
use futures::future::join_all;
use lru::LruCache;
use std::num::NonZeroUsize;

/// A transport network which caches tiles.
pub struct CachedTileNetwork<B: Backend<T>, T: Tile> {
    backend: B,
    router: Router,
    tiles: LruCache<tile::Coord, T>,
}

impl<B: Backend<T>, T: Tile> CachedTileNetwork<B, T> {
    pub fn new(backend: B) -> Self {
        CachedTileNetwork {
            router: Router::new(),
            tiles: LruCache::new(NonZeroUsize::new(27).unwrap()),
            backend,
        }
    }

    pub async fn find_route(&mut self, start: &Point, stop: &Point) -> Result<Route, RoutingError> {
        let backend = &self.backend;
        debug_log!("find route");
        let tile_coord = point_to_tile_coord(&start, 14);
        self.router = Router::new();
        let mut futures = Vec::new();
        for x in (tile_coord.x - 1)..=(tile_coord.x + 1) {
            for y in (tile_coord.y - 1)..=(tile_coord.y + 1) {
                let rel_coord = tile::Coord {
                    x,
                    y,
                    z: tile_coord.z,
                };
                if self.tiles.get(&rel_coord).is_none() {
                    let coord_clone = rel_coord.clone();
                    futures
                        .push(async move { (backend.get_tile(&coord_clone).await, coord_clone) });
                }
            }
        }
        let tiles = join_all(futures).await;
        for (tile, coord) in tiles {
            if tile.is_ok() {
                self.tiles.push(coord.clone(), tile.unwrap());
            }
        }
        for x in (tile_coord.x - 1)..=(tile_coord.x + 1) {
            for y in (tile_coord.y - 1)..=(tile_coord.y + 1) {
                let rel_coord = tile::Coord {
                    x,
                    y,
                    z: tile_coord.z,
                };
                let tile = self.tiles.get(&rel_coord);
                if let Some(tile) = tile {
                    let result = tile.parse(&mut self.router);
                    if result.is_err() {
                        debug_log!("Tile parsing error: {:?}", result);
                        return Err(RoutingError::TileParsingError);
                    }
                }
            }
        }
        self.router.find_route(start, stop)
    }
}
