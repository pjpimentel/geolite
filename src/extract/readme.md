# extract

parse a `.osm.pbf` file and populate the database layer by layer

the three stages that come before these — scanning the file into byte ranges, reading its header
and decoding its nodes, ways and relations — belong to the file itself and live in
[`domain/osm_pbf_file`](../domain/readme.md#osm_pbf_file).

## admin_levels

1. selects candidate ids per level: relations from `osm_relations` by `admin_level` tag (levels 1–9, and 10), or ways from `osm_ways` by `place`/name (levels 10 and 12) — all requiring a non-null `name`
2. assembles the member way segments into rings, building a polygon when the ring closes, otherwise a line (streets at level 12 are always lines)
3. inserts each feature into `admin_levels` (name, country/post codes, and a spatialite WKB geometry blob)

## house_numbers

1. queries house number nodes from `osm_nodes` (those with `addr:housenumber`)
2. assigns each node to a street (`admin_levels` row) using proximity or name matching
3. inserts into `house_numbers` with geometry and the strategy used
