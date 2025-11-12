use geo_types::Geometry;
use mvt_reader::Reader as MvtTile;
use vello::Scene;
use vello::kurbo::{Affine, Line, Stroke};
use vello::peniko::Color;

pub struct MapRenderer {
    tile: MvtTile,
}

impl MapRenderer {
    pub fn new(tile: MvtTile) -> Self {
        MapRenderer { tile }
    }

    pub fn render_to_scene(&mut self, scene: &mut Scene) {
        let my_stroke = Stroke::new(6.0);
        let my_line = Line::new((100.0, 100.0), (500.0, 500.0));
        let my_color = Color::new([0.7, 0.6, 1.0, 1.0]);
        scene.stroke(&my_stroke, Affine::IDENTITY, my_color, None, &my_line);

        let road_layer_id = self
            .tile
            .get_layer_names()
            .unwrap() // FIXME
            .iter()
            .position(|x| x == "roads");

        let Some(road_layer_id) = road_layer_id else {
            return;
        };

        // FIXME: remove unwrap
        let road_features = self.tile.get_features(road_layer_id).unwrap();

        for feature in road_features {
            println!("geo {:#?} {:#?}", feature.id, feature.geometry);

            match feature.geometry {
                Geometry::LineString(line) => {
                    // let points = line.into_points();

                    // for window in points.windows(2) {
                    //     let a = &window[0];
                    //     let b = &window[1];

                    //     println!("x: {}, y: {}", a.x(), b.x())
                    // }
                }
                _ => (),
            }
        }
    }
}
