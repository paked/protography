# protography - vector map explorations with Vello and Protomaps

To run: `cargo run toolangi.pmtiles`. Other pmtiles archives could work too one day.

Do not use this, or judge me too hard for the code within it. I am writing it to learn Rust.

## Adding a new region

* Install the `pmtiles` cli
* Copy the URL of the latest global pmtiles archive from `https://maps.protomaps.com/builds/` (do not download this file)
* Create a `your-region.geojson` file containing the bounds of the region you want to explore. An easy way to do this is on https://geojson.io.
* Run `pmtiles extract <latest-pmtiles-archive-url> --region your-region.geojson your-region.pmtiles`

You can now run `cargo run your-region.pmtiles` and see the viewer!

## License

Code in this repo is licensed as per the [LICENSE](./LICENSE) file.

The example dataset (eg. toolangi.pmtiles) includes map data from **OpenStreetMap**, which is © OpenStreetMap contributors and is licensed under the **Open Database License (ODbL)**. The included `*.pmtiles` file is a derivative database created from OSM data. You must attribute OSM if you use or redistribute this file. [See this link for more information](https://www.openstreetmap.org/copyright).
