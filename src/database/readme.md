# database

store all data in `~/.geolite/database.sqlite3`

## layers

```
layer 5  ·  admin_levels_hierarchy     ancestor chain + user-friendly name per admin level
         ·  admin_levels_rtree         virtual rtree: bbox spatial index for fast coord lookup
            ─────────────────────────────────────────────────────────────────────────────────
layer 4  ·  admin_levels               OSM boundary polygons (admin_level tag 2–12)
         ·  house_numbers              address nodes linked to a street (admin_levels row)
            ─────────────────────────────────────────────────────────────────────────────────
layer 3  ·  osm_nodes / osm_ways / osm_relations    raw OSM primitives
            ─────────────────────────────────────────────────────────────────────────────────
layer 2  ·  osm_pbf_blob_chunks        byte ranges of each blob (header=0 / data=1)
            ─────────────────────────────────────────────────────────────────────────────────
layer 1  ·  osm_pbf_files              one row per .osm.pbf — geofabrik metadata + download state
```

each layer depends only on the one below it; `destroy_data` drops top-down and vacuums.
