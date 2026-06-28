# optimize
> shrink the database after the pipeline is complete

## delete_intermediary_data

1. deletes the `osm_data.sqlite3` sibling file (plus its `-wal`/`-shm`), which holds `osm_pbf_blob_chunks`, `osm_nodes`, `osm_ways`, `osm_relations`
2. removes every `*.osm.pbf` in the data dir, logging each file's size

## sqlite_file

1. runs `ANALYZE`, `PRAGMA optimize`, `PRAGMA wal_checkpoint(TRUNCATE)`, then `VACUUM` to refresh stats and compact the sqlite file (reporting size before → after)
