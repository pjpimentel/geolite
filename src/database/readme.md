# database

store admin data in `~/.geolite/database.sqlite3` (layers 1, 4, 5); raw OSM primitives (layers 2–3) live in a sibling `~/.geolite/database.osm_data.sqlite3`, attached at runtime as schema `osm_data`

## layers

```
layer 5  ·  admin_levels_hierarchy     ancestor chain + user-friendly name per admin level — domain/admin_level_hierarchy
         ·  admin_levels_rtree         virtual rtree: bbox spatial index for fast coord lookup — domain/admin_level
            ─────────────────────────────────────────────────────────────────────────────────
layer 4  ·  admin_levels               admin areas — levels 1–9 (admin_level tag), 10 (place ways), 12 (streets) — domain/admin_level
         ·  house_numbers              address nodes linked to a street (admin_levels row) — domain/house_number
            ─────────────────────────────────────────────────────────────────────────────────
layer 3  ·  osm_nodes / osm_ways / osm_relations    raw OSM primitives
            ─────────────────────────────────────────────────────────────────────────────────
layer 2  ·  osm_pbf_blob_chunks        byte ranges of each blob (header=0 / data=1)
            ─────────────────────────────────────────────────────────────────────────────────
layer 1  ·  osm_pbf_files              one row per .osm.pbf — origin (local_path, geofabrik, url) and its coverage, download state, header and counts
```

`admin_levels` and its rtree are owned by [`domain/admin_level`](../domain/readme.md#admin_level), `admin_levels_hierarchy` by [`domain/admin_level_hierarchy`](../domain/readme.md#admin_level_hierarchy) and `house_numbers` by [`domain/house_number`](../domain/readme.md#house_number); this folder keeps the connection lifecycle that creates them.

layers are built roughly bottom-up (back-references: `house_numbers → admin_levels`, and the `*_count` columns on `osm_pbf_files`). `destroy_data` selectively drops upper layers — deleting the `osm_data` sibling file when layer 2 goes, and dropping only the three element tables when layer 3 goes alone, so the chunk index survives a `--recreate` of the osm-data stage — then vacuums; `osm_pbf_files` is kept.

every writable connection runs in WAL mode; the schema is stamped via `PRAGMA user_version` (`SCHEMA_VERSION`): a writable open refuses a database stamped with another version, and `geolite merge` refuses to combine builds with an incompatible one. a column that only extends the schema is added to an older database on open (`ALTER TABLE`), without a new version.

`jsonb.rs` is the encoder behind every `payload` column: sqlite's jsonb binary format, written once per element in the extraction's decoder threads and read back with `JSON(payload)`.
