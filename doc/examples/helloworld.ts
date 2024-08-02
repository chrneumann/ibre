import maplibregl, { GeoJSONSource, Marker, Map } from "maplibre-gl";
import initIbre, { PMTilesMVTRouter, RoutingError, Point, init_hooks } from "ibre";

const map = new Map({
    container: 'map',
    zoom: 12,
    center: [8.684966, 50.110573],
    style: {
        version: 8,
        sources: {
            osm: {
                type: 'raster',
                tiles: ['https://a.tile.openstreetmap.org/{z}/{x}/{y}.png'],
                tileSize: 256,
                attribution: '&copy; OpenStreetMap Contributors',
                maxzoom: 19
            },
        },
        layers: [
            {
                id: 'osm',
                type: 'raster',
                source: 'osm'
            },
        ],
    },
    maxZoom: 18,
    maxPitch: 85
});

map.once('load', () => {
    map.addSource("transport", {
        type: "geojson",
        data: {
            type: "FeatureCollection",
            features: [],
        },
    });

    map.addLayer({
        id: "transport",
        type: "line",
        source: "transport",
        layout: {
            "line-join": "round",
            "line-cap": "round",
        },
        paint: {
            "line-color": "#FF0000",
            "line-opacity": 0.7,
            "line-width": 5,
        },
    });
});

const startMarker = new Marker({
    color: "#FFCCE6",
    draggable: true,
}).setLngLat([8.68, 50.11]);

const stopMarker = new Marker({
    color: "#AB212A",
    draggable: true,
    scale: 1.3,
}).setLngLat([8.692, 50.117]);


(async () => {
    await initIbre();
    init_hooks();

    const router = new PMTilesMVTRouter('https://raw.githubusercontent.com/chrneumann/ibre-example-data/main/network.pmtiles');
    const toPoint = (marker: Marker) => {
        const lngLat = marker.getLngLat();
        return new Point(lngLat.lng, lngLat.lat);
    };
    const updateRoute = async () => {
        try {
            const route = await router.findRoute(
                toPoint(startMarker),
                toPoint(stopMarker),
            );
            const source = map.getSource("transport");
            if (source instanceof GeoJSONSource) {
                const segments = route.get_segments_as_geojson();
                source.setData(
                    JSON.parse(segments)
                );
            }
        } catch (error) {
            console.log("error finding route", error);
        }
    };

    for (const marker of [startMarker, stopMarker]) {
        marker
            .on("dragend", updateRoute)
            .addTo(map);
    }
})();
