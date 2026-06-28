# extract

parse a `.osm.pbf` file and populate the database layer by layer

## blob_chunks

1. scans the `.osm.pbf` file sequentially, reading 4-byte length-prefixed blob headers
2. records each blob's byte range (`first_byte`, `chunk_size`, `data_first_byte`, `data_size`) and type (header / data)
3. bulk-inserts all chunks into `osm_pbf_blob_chunks`

## header

1. reads the header blob byte range from `osm_pbf_blob_chunks`
2. decodes the OSM header (bbox, required/optional features, writingprogram, osmosis replication metadata)
3. updates the corresponding row in `osm_pbf_files`

## osm_data

1. loads all data blob byte ranges from `osm_pbf_blob_chunks`
2. decompresses each blob (raw or zlib) in parallel threads via a bounded queue
3. decodes protobuf primitive blocks → nodes (dense), ways, relations
4. bulk-inserts into `osm_nodes`, `osm_ways`, `osm_relations`

## admin_levels

1. queries relations from `osm_relations` filtered by `boundary=administrative` and a given `admin_level`
2. assembles way segments into closed rings and builds the polygon geometry
3. inserts each boundary into `admin_levels` with its spatialite geometry blob and bbox

## house_numbers

1. queries house number nodes from `osm_nodes` (those with `addr:housenumber`)
2. assigns each node to a street (`admin_levels` row) using proximity or name matching
3. inserts into `house_numbers` with geometry and the strategy used
