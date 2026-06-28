# optimize
> shrink the database after the pipeline is complete

## delete_intermediary_data

1. drops `osm_relations`, `osm_ways`, `osm_nodes`, `osm_pbf_blob_chunks` in order
2. removes every `*.osm.pbf` in the data dir, logging each file's size

## sqlite_file

1. checkpoints WAL and runs `VACUUM` to compact the sqlite file
