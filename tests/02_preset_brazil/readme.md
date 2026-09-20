# the santos fixture

`santos.osm.pbf` is the map every e2e scenario runs on: santos, são vicente, guarujá and cubatão,
cut from a geofabrik `brazil-latest.osm.pbf` by `santos.build-fixture.sh`.

## provenance

- source: the [geofabrik brazil extract](https://download.geofabrik.de/south-america/brazil.html),
  itself a cut of the openstreetmap planet. the extract's replication timestamp is not kept: the
  last step of the script rewrites the header with the fixed generator `geolite-e2e-fixture/1`, so
  the header assertion of the suite is machine-independent.
- cut: the bounding box `-46.45,-24.06,-46.16,-23.72` with complete ways, merged with the
  administrative boundaries of `santos.boundary-ids.txt` and everything they reference (a clipped
  boundary never closes a ring and could not become an ancestor), sorted, and stripped of the edit
  metadata (`version`, `timestamp`, `changeset`, `uid`, `user`) the build ignores anyway.
- identity: md5 `44ba55e5d242559e6709418427ae9168`, 5,100,204 bytes; 683,313 nodes, 41,335 ways
  and 1,274 relations; 12,981 admin levels before `optimize merge-admin-levels` folds the ways of
  each street, 7,298 after, and 473 house numbers, after a build with `--preset brazil`. the e2e
  harness keys its shared world on the preset, the city and this md5, so a regenerated file rebuilds
  the world on its own.

## regenerating

`./santos.build-fixture.sh /path/to/brazil-latest.osm.pbf` (osmium-tool 1.14 or newer, about 4 GB
of ram, about 10 minutes). a newer source changes the map, and with it the counts, ids and names
the scenarios pin: every assertion that can move carries a `REGENERATE` note saying what to
re-read.

## licence

the file is a derivative database of openstreetmap data,
© [openstreetmap contributors](https://www.openstreetmap.org/copyright), distributed under the
[open database license](https://opendatacommons.org/licenses/odbl/) (odbl 1.0) like every database
geolite builds. it is here only to keep the assertions running over the same bytes.
