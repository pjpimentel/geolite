# osm_pbf_file

discover and download `.osm.pbf` files

## ls

1. fetches the geofabrik GeoJSON index from the configured endpoint
1. caches all regions in `osm_pbf_files` (upsert by `geofabrik_id`)
1. subsequent calls read from sqlite — skips http unless `recreate_cache` is set

## download

1. resolves the source: geofabrik id → looks up `geofabrik_url` in sqlite; direct url → used as-is
1. skips entirely if the destination file already exists
1. splits the total size into N byte ranges and fetches them in parallel threads
1. merges parts in order into the final `.osm.pbf` file
1. verifies md5 checksum against `<url>.md5` (ok / mismatch / unavailable)
