# database

store admin data in `~/.geolite/database.sqlite3` (layers 1, 4, 5); raw OSM primitives (layers 2–3) live in a sibling `~/.geolite/database.osm_data.sqlite3`, attached at runtime as schema `osm_data`

## layers

```
layer 5  ·  admin_levels_hierarchy     ancestor chain + user-friendly name per admin level
         ·  admin_levels_rtree         virtual rtree: bbox spatial index for fast coord lookup
            ─────────────────────────────────────────────────────────────────────────────────
layer 4  ·  admin_levels               admin areas — levels 1–9 (admin_level tag), 10 (place ways), 12 (streets)
         ·  house_numbers              address nodes linked to a street (admin_levels row)
            ─────────────────────────────────────────────────────────────────────────────────
layer 3  ·  osm_nodes / osm_ways / osm_relations    raw OSM primitives
            ─────────────────────────────────────────────────────────────────────────────────
layer 2  ·  osm_pbf_blob_chunks        byte ranges of each blob (header=0 / data=1)
            ─────────────────────────────────────────────────────────────────────────────────
layer 1  ·  osm_pbf_files              one row per .osm.pbf — geofabrik metadata + download state
```

layers are built roughly bottom-up (back-references: `house_numbers → admin_levels`, and the `*_count` columns on `osm_pbf_files`). `destroy_data` selectively drops upper layers — deleting the `osm_data` sibling file for layers 2–3 — then vacuums; `osm_pbf_files` is kept.

every writable connection runs in WAL mode; the schema is stamped via `PRAGMA user_version` (`SCHEMA_VERSION`) and checked by `geolite merge`, which refuses to combine builds with an incompatible version.
