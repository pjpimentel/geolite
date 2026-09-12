# extract

parse a `.osm.pbf` file and populate the database layer by layer

the three stages that come before these — scanning the file into byte ranges, reading its header
and decoding its nodes, ways and relations — belong to the file itself and live in
[`domain/osm_pbf_file`](../domain/readme.md#osm_pbf_file).

## admin_levels

the stage moved into the area it produces: `admin_level::extract(level)` in
[`domain/admin_level`](../domain/readme.md#admin_level).

## house_numbers

1. queries house number nodes from `osm_nodes` (those with `addr:housenumber`)
2. assigns each node to a street (`admin_levels` row) using proximity or name matching
3. inserts into `house_numbers` with geometry and the strategy used
