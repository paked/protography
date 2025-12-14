# TODO

* [ ] Switch to vello hybrid, have separate web and native versions (web is awesome from a demo perspective!)
* [ ] Handle bigger pmtiles files correctly (eg. leaf directories)
* [ ] Where are the missing tiles coming from?
* [ ] Zoom on cursor, not center of screen.
* [ ] Gesture based Zoom support
* [ ] Debug text (eg. FPS counter)
* [ ] Street / region labels
* [ ] Make zoom more ergonomic
* Actual functionality
  * Load heatmaps from Strava data?
* [ ] Separate stroke and fill layers
  * Allows strokes to stay same width irrespective of zoom
  * enables potential caching of fill layers (as they don't change with zoom)
* [ ] Nicer styled maps
  * Read JSON layer and style based on those attributes
* Pmtiles
  * [ ] Network backed pmtiles file, instead of local
  * [ ] Decode pmtiles on separate thread, don't block render (ever)
