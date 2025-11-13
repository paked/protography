use std::iter::Map;
use std::sync::Mutex;

use geo_types::{Coord, Geometry, LineString, Point, Polygon};
use mvt_reader::feature::Feature;
use mvt_reader::{Reader as MvtTile, feature, layer};
use vello::kurbo::{Affine, BezPath, Line, Stroke};
use vello::low_level::Render;
use vello::peniko::Color;
use vello::{Scene, kurbo};

pub struct RenderTargetInfo {
    pub width: u32,
    pub height: u32,
}

pub struct MapRenderer {
    tile: MvtTile,
}

impl MapRenderer {
    pub fn new(tile: MvtTile) -> Self {
        MapRenderer { tile }
    }

    fn path_from_line(line: &LineString<f32>, target_info: &RenderTargetInfo) -> BezPath {
        let mut path = BezPath::new();

        if let Some(first) = line.points().next() {
            let first = first / 4096.0 * target_info.width as f32;
            path.move_to((first.x(), first.y()));

            for next in line.points().skip(1) {
                let next = next / 4096.0 * target_info.width as f32;
                path.line_to((next.x(), next.y()));
            }
        }

        path
    }

    fn draw_line(
        &mut self,
        scene: &mut Scene,
        target_info: &RenderTargetInfo,
        line: &LineString<f32>,
    ) {
        // TODO: refactor to use BezPath Kurbo primitive
        let my_stroke = Stroke::new(6.0);
        let my_color = Color::new([0.7, 0.6, 1.0, 1.0]);

        let path = MapRenderer::path_from_line(line, target_info);

        scene.stroke(&my_stroke, Affine::IDENTITY, my_color, None, &path);

        // let mut points = line.points();
        // let mut start = points.nth(0).unwrap() / 4096.0 * target_info.width as f32;

        // for end in points {
        //     let end = end / 4096.0 * target_info.width as f32;
        //     let line = Line::new((start.x(), start.y()), (end.x(), end.y()));
        //     scene.stroke(&my_stroke, Affine::IDENTITY, my_color, None, &line);

        //     start = end;
        // }
    }

    fn draw_polygon(
        &mut self,
        scene: &mut Scene,
        target_info: &RenderTargetInfo,
        polygon: &Polygon<f32>,
    ) {
        let stroke = Stroke::new(1.0);
        let stroke_color = Color::new([0.2, 1.0, 0.5, 1.0]);
        let fill_color = Color::new([0.2, 7.0, 0.5, 0.5]);

        let path = MapRenderer::path_from_line(polygon.exterior(), target_info);
        scene.stroke(&stroke, Affine::IDENTITY, stroke_color, None, &path);

        scene.fill(
            vello::peniko::Fill::NonZero,
            Affine::IDENTITY,
            fill_color,
            None,
            &path,
        );

        // TODO(render internal areas to, alternate rings with Fill:EvenOdd)

        // self.draw_line(scene, target_info);
    }

    fn draw_feature(
        &mut self,
        scene: &mut Scene,
        target_info: &RenderTargetInfo,
        feature: &Feature,
    ) {
        match &feature.geometry {
            Geometry::MultiLineString(multi_line) => multi_line
                .iter()
                .for_each(|l| self.draw_line(scene, target_info, l)),
            Geometry::LineString(line) => self.draw_line(scene, target_info, line),
            Geometry::Polygon(_) => println!("got polygon"),
            Geometry::MultiPolygon(multi_polygon) => {
                multi_polygon
                    .iter()
                    .for_each(|p| self.draw_polygon(scene, target_info, p));
            }
            Geometry::GeometryCollection(_) => println!("got geometry collection"),
            _ => println!("Other geoemetry value"),
        }
    }

    pub fn render_to_scene(&mut self, scene: &mut Scene, target_info: &RenderTargetInfo) {
        let layer_names = self.tile.get_layer_names().unwrap(); // FIXME

        let road_layer_id = layer_names.iter().position(|x| x == "roads");

        let Some(road_layer_id) = road_layer_id else {
            return;
        };

        let landuse_layer_id = layer_names.iter().position(|x| x == "landuse");
        let Some(landuse_layer_id) = landuse_layer_id else {
            return;
        };

        // FIXME: remove unwrap
        let landuse_features = self.tile.get_features(landuse_layer_id).unwrap();
        for feature in landuse_features {
            self.draw_feature(scene, target_info, &feature);
        }

        // FIXME: remove unwrap
        let road_features = self.tile.get_features(road_layer_id).unwrap();
        for feature in road_features {
            self.draw_feature(scene, target_info, &feature);
        }
    }
}
