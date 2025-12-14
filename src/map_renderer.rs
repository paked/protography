use geo_types::{Geometry, LineString, Polygon};
use mvt_reader::feature::Feature;
use vello::Scene;
use vello::kurbo::simplify::SimplifyOptions;
use vello::kurbo::{Affine, BezPath, Rect, Stroke};
use vello::peniko::Color;

use crate::pmtiles::{Position, TileCoord, lat_lon_to_xyz};

pub const TILE_SIZE: f64 = 512.0;

pub struct Camera {
    pub x: f64,
    pub y: f64,
    pub zoom: f64,
    pub world_origin: Position,
    pub width: u32,
    pub height: u32,
}

// TODO: these small values of zoom are probably not usable. find a better way to represent this
const ZOOM_LEVELS_LUT: [f64; 15] = [
    0.00006103515625,
    0.0001220703125,
    0.000244140625,
    0.00048828125,
    0.0009765625,
    0.001953125,
    0.00390625,
    0.0078125,
    0.015625,
    0.03125,
    0.0625,
    0.125,
    0.25,
    0.5,
    1.0,
];

fn get_zoom_level(zoom: f64) -> usize {
    if zoom >= *ZOOM_LEVELS_LUT.last().unwrap() {
        return ZOOM_LEVELS_LUT.len() - 1;
    }

    ZOOM_LEVELS_LUT.iter().position(|&z| zoom <= z).unwrap()
}

impl Camera {
    pub fn get_tile_size_in_world_pixels(&self) -> f64 {
        TILE_SIZE * self.get_tile_size_multipler()
    }

    pub fn get_tile_size_multipler(&self) -> f64 {
        let idx = (ZOOM_LEVELS_LUT.len() - get_zoom_level(self.zoom)) as u32 - 1;

        // TODO: turn this into a lut
        u32::pow(2, idx) as f64
    }

    fn get_tile_range_dimensions(&self) -> (f64, f64) {
        let tile_in_pixels = self.get_tile_size_in_world_pixels() * self.zoom;

        let width_in_tiles = self.width as f64 / tile_in_pixels;
        let height_in_tiles = self.height as f64 / tile_in_pixels;

        (width_in_tiles, height_in_tiles)
    }

    pub fn get_slippy_zoom(&self) -> u8 {
        let z = get_zoom_level(self.zoom) + 1;

        z as u8
    }

    pub fn world_origin(&self) -> TileCoord {
        lat_lon_to_xyz(
            self.world_origin.lat,
            self.world_origin.long,
            self.get_slippy_zoom(),
        )
    }

    pub fn get_tile_range(&self) -> ((u32, u32), (u32, u32)) {
        let TileCoord {
            x: world_origin_x,
            y: world_origin_y,
            z: _,
        } = self.world_origin();

        let (wx, wy) = self.get_tile_range_dimensions();

        let tile_x = self.x / self.get_tile_size_in_world_pixels();
        let tile_y = self.y / self.get_tile_size_in_world_pixels();

        let x = world_origin_x + tile_x;
        let y = world_origin_y + tile_y;

        let min_x = (x - wx / 2.0).floor() as u32;
        let min_y = (y - wy / 2.0).floor() as u32;

        let max_x = (x + wx / 2.0).ceil() as u32;
        let max_y = (y + wy / 2.0).ceil() as u32;

        ((min_x, min_y), (max_x, max_y))
    }
}

pub struct MapRenderer {}

impl MapRenderer {
    pub fn new() -> Self {
        MapRenderer {}
    }

    // TODO: this should be a from?
    fn path_from_line(line: &LineString<f32>) -> BezPath {
        let mut path = BezPath::new();

        if let Some(first) = line.points().next() {
            // TODO: this transformation should be a transformation
            let first = first / 4096.0 * TILE_SIZE as f32;
            path.move_to((first.x(), first.y()));

            for next in line.points().skip(1) {
                let next = next / 4096.0 * TILE_SIZE as f32;
                path.line_to((next.x(), next.y()));
            }
        }

        path
    }

    fn draw_line(&mut self, scene: &mut Scene, transform: Affine, line: &LineString<f32>) {
        // TODO: refactor to use BezPath Kurbo primitive
        let my_stroke = Stroke::new(6.0);
        let my_color = Color::new([0.7, 0.6, 1.0, 1.0]);

        let path = MapRenderer::path_from_line(line);

        scene.stroke(&my_stroke, transform, my_color, None, &path);
    }

    fn draw_polygon(&mut self, scene: &mut Scene, transform: Affine, polygon: &Polygon<f32>) {
        let stroke = Stroke::new(1.0);
        let stroke_color = Color::new([0.0, 0.5, 0.0, 1.0]);
        let fill_color = Color::new([0.2, 7.0, 0.5, 0.5]);

        let path = MapRenderer::path_from_line(polygon.exterior());

        scene.fill(
            vello::peniko::Fill::NonZero,
            transform,
            fill_color,
            None,
            &path,
        );

        scene.stroke(&stroke, transform, stroke_color, None, &path);

        // TODO(render internal areas too, alternate rings with Fill:EvenOdd)
    }

    fn draw_feature(&mut self, scene: &mut Scene, transform: Affine, feature: &Feature) {
        match &feature.geometry {
            Geometry::MultiLineString(multi_line) => multi_line
                .iter()
                .for_each(|l| self.draw_line(scene, transform, l)),
            Geometry::LineString(line) => self.draw_line(scene, transform, line),
            Geometry::Polygon(_) => println!("got polygon"),
            Geometry::MultiPolygon(multi_polygon) => {
                multi_polygon
                    .iter()
                    .for_each(|p| self.draw_polygon(scene, transform, p));
            }
            Geometry::GeometryCollection(_) => println!("got geometry collection"),
            _ => println!("Other geoemetry value"),
        }
    }

    pub fn render_to_scene(
        &mut self,
        tile: &mvt_reader::Reader,
        scene: &mut Scene,
        transform: Affine,
    ) {
        let layer_names = tile.get_layer_names().unwrap(); // FIXME

        let bounds = Rect::new(0.0, 0.0, TILE_SIZE, TILE_SIZE);

        let hairline_compensation = 0.5;
        scene.push_clip_layer(
            transform,
            &bounds.inflate(hairline_compensation, hairline_compensation),
        );

        let landuse_layer_id = layer_names.iter().position(|x| x == "landuse");
        if let Some(landuse_layer_id) = landuse_layer_id {
            // FIXME: remove unwrap
            let landuse_features = tile.get_features(landuse_layer_id).unwrap();
            for feature in landuse_features {
                self.draw_feature(scene, transform, &feature);
            }
        };

        let road_layer_id = layer_names.iter().position(|x| x == "roads");
        if let Some(road_layer_id) = road_layer_id {
            // FIXME: remove unwrap
            let road_features = tile.get_features(road_layer_id).unwrap();
            for feature in road_features {
                self.draw_feature(scene, transform, &feature);
            }
        }

        scene.pop_layer();
    }
}
