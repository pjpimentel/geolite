# the south-america fixture

`south-america.osm.pbf` is a synthetic map, written by hand and not cut from openstreetmap: for
each of the thirteen countries of south america it holds one closed level-2 boundary, a square of
four corners 0.01° around a point well inside the compiled outline of the country, and one
two-node street through its centre. nothing else: the scenario over it asserts that `build`
writes one base per country, and needs no more. the brazil fixture of `02_preset_brazil` remains
the map every other scenario of this target runs on.

## layout

the position `k` of a country in the table is the prefix of its ids: nodes `k*100+1..6` (the four
corners sw, se, ne, nw, then the two ends of the street at `lon ∓ 0.005`), way `k*100+1` (the
ring, unnamed and tagged `boundary=administrative`, so it is never a street), way `k*100+2` (the
street, `highway=residential`, named `<country> Street`) and relation `k*100+1` (`type=boundary`,
`boundary=administrative`, `admin_level=2`, named after the country). every node comes first,
then every way, then every relation, ids ascending.

| k | iso | name | lon | lat |
|---|---|---|---|---|
| 1 | AR | Argentina | -64.0 | -32.0 |
| 2 | BO | Bolivia | -65.0 | -17.5 |
| 3 | BR | Brazil | -47.9 | -15.8 |
| 4 | CL | Chile | -71.3 | -35.4 |
| 5 | CO | Colombia | -74.0 | 4.7 |
| 6 | EC | Ecuador | -78.5 | -1.0 |
| 7 | GF | French Guiana | -53.0 | 4.0 |
| 8 | GY | Guyana | -58.8 | 5.5 |
| 9 | PE | Peru | -75.0 | -10.0 |
| 10 | PY | Paraguay | -56.5 | -24.0 |
| 11 | SR | Suriname | -55.5 | 4.5 |
| 12 | UY | Uruguay | -56.0 | -33.0 |
| 13 | VE | Venezuela | -66.0 | 8.0 |

every centre falls inside exactly one of the compiled outlines, at least 89 km from the edge of
its own (chile, the tightest), so the country of every row is decided without a doubt.

## identity

md5 `de1c9891726dcf3708cf4573cde1521e`, 1,085 bytes; 78 nodes, 26 ways and 13 relations; header
generator `geolite-e2e-fixture/1`, no edit metadata. a build of it writes 13 bases of two rows
each, the country and the street, and no house number. the e2e harness keys its world on the
fixture name and this md5, so a regenerated file rebuilds the world on its own. `GEOLITE_E2E_PBF`
overrides the pbf of every scenario of the target, this one included.

## regenerating

`./south-america.build-fixture.sh` (osmium-tool 1.14 or newer, seconds): the script writes the
map above as osm xml and converts it with `osmium cat -f pbf,add_metadata=false
--output-header="generator=geolite-e2e-fixture/1"`. like every fixture recipe it stays out of the
repository; this readme holds everything needed to write it again.
