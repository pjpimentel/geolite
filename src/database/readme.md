# database

store admin data in `~/.geolite/database.sqlite3` (layers 1, 4, 5); raw OSM primitives (layers 2–3) live in a sibling `~/.geolite/database.osm_data.sqlite3`, attached at runtime as schema `osm_data`

## layers

```
layer 5  ·  admin_levels_hierarchy     one edge child > parent per row; parent_id for the downward read — admin_level_hierarchy
         ·  admin_levels_rtree         virtual rtree: bbox spatial index for fast coord lookup — admin_level
            ─────────────────────────────────────────────────────────────────────────────────
layer 4  ·  admin_levels               admin areas — levels 1–9 (admin_level tag), 10 (place ways), 12 (streets) — admin_level
         ·  house_numbers              address nodes linked to a street (admin_levels row) — house_number
            ─────────────────────────────────────────────────────────────────────────────────
layer 3  ·  osm_nodes / osm_ways / osm_relations    raw OSM primitives — osm_node, osm_way, osm_relation
            ─────────────────────────────────────────────────────────────────────────────────
layer 2  ·  osm_pbf_blob_chunks        byte ranges of each blob (header=0 / data=1)
            ─────────────────────────────────────────────────────────────────────────────────
layer 1  ·  osm_pbf_files              one row per .osm.pbf — origin (local_path, geofabrik, url) and its coverage, download state, header and counts
```

`admin_levels` and its rtree are owned by [`admin_level`](../admin_level/readme.md), `admin_levels_hierarchy` by [`admin_level_hierarchy`](../admin_level_hierarchy/readme.md) `house_numbers` by [`house_number`](../house_number/readme.md) and the three element tables by [`osm_node`, `osm_way` and `osm_relation`](../osm_node/readme.md); this folder keeps the connection lifecycle that creates them, the `table` trait every repository implements, the shared helpers behind their sql (`placeholders_for`, `query_by_ids`, `insert_in_chunks`) and the generic jsonb codec.

layers are built roughly bottom-up (back-references: `house_numbers → admin_levels`, and the `*_count` columns on `osm_pbf_files`). `destroy_data` selectively drops upper layers — deleting the `osm_data` sibling file when layer 2 goes, and dropping only the three element tables when layer 3 goes alone, so the chunk index survives a `--recreate` of the osm-data stage — then vacuums; `osm_pbf_files` is kept.

`compact` is the end of a build: `ANALYZE`, `PRAGMA optimize`, `wal_checkpoint(TRUNCATE)` and `VACUUM` on the main file, answering with the bytes before and after; `remove_osm_data_files` deletes the `osm_data` sibling with its `-wal` and `-shm` and answers with the bytes freed. `destroy_data` uses the second, `geolite optimize` and `geolite merge` the first.

every writable connection runs in WAL mode; the schema is stamped via `PRAGMA user_version` (`SCHEMA_VERSION`): a writable open refuses a database stamped with another version, and `geolite merge` refuses to combine builds with an incompatible one. a column that only extends the schema is added to an older database on open (`ALTER TABLE`), without a new version: `osm_pbf_files.origin_wkt` and `admin_levels.merged_way_ids` came that way, each guarded by `has_column`, which takes the schema so it can also ask an attached database.

`merge_source` copies `merged_way_ids` with the `wkb` it describes, in the one upsert. a source is attached read-only and cannot gain the column, so one built before it answers `NULL` there, and the street merge that follows traces what it folds.

`jsonb.rs` is the generic codec behind every `payload` column: sqlite's jsonb binary format — headers, size classes, objects and arrays — with no knowledge of what is written; each element folder's `payload` module writes its own shape with it, once per element in the extraction's decoder threads, and every reader gets it back with `JSON(payload)`. `admin_levels.merged_way_ids` is the one json column that does not go through it: a short array written a few thousand times by the street merge, bound as json text under `JSONB(?n)`.
