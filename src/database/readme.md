# database

the layout of a data path and the connection lifecycle: `store` names every file and opens each

```
<data-path>/
  catalog.sqlite3            layer 1: osm_pbf_files, the ledger of the sources and the cache of the geofabrik index
  <name>.osm.pbf             a source, downloaded or copied in; deleted at the end of the build
  osm_data.sqlite3           layers 2–3 of every source, one file per data path, attached as `osm_data` while the stages run; deleted at the end of the build
  br/                        one folder per country, named by its code
    data.sqlite3             layers 4–5 of the country
    index.tantivy            its search index
```

a base is a folder named by the iso 3166-1 alpha-2 code of its country in lower case, the name of
its boundary file, holding `data.sqlite3` and `index.tantivy`; a message names a file of a base by
its folder, `br/data.sqlite3` (`label_of`). `store` knows the paths (`catalog_path`,
`osm_data_path`, `base_path`, `index_path_of`, `bases`, which lists the `??/data.sqlite3` of a data
path) and the openings:
`open_catalog`; `open_osm_data`, the catalogue with the shared osm_data attached, and
`open_osm_data_write`, the same after a probe connection took and released the write lock of the
osm_data, so a second process writing it is refused aloud instead of meeting `database is locked`
deep in a stage (one process at a time writes the osm data of a data path; a reader is not
detected); and `open_base_write` / `open_base_readonly` / `open_base_immutable`. `open_base_readonly` is the pipeline's read of a base another connection
of the run is writing, a resume subtracting what the base holds for instance: it follows the wal.
`open_base_immutable`, behind `query`, `http-server` and the tui, opens the file as
`file:<path>?mode=ro&immutable=1`: no lock and no wal-index, so no `-wal` or `-shm` ever appears
beside a base and a base on read-only media opens; the price is that a base written while it is
read is seen as it was, until the reader opens it again.
`base_pool` is what the extraction writes through: one connection per country, opened (its folder
created) on the first row of that country, never attached, so the ten attachments sqlite allows are never in the way;
`attach_base` lets the ledger count the rows of a file in one base at a time.

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

layers are built roughly bottom-up (back-references: `house_numbers → admin_levels`, and the `*_count` columns on `osm_pbf_files`). `destroy_source` drops the upper layers of one file out of the shared `osm_data` — its element rows, found through the chunks they point at, and its chunks too when layer 2 goes, so the chunk index survives a `--recreate` of the osm-data stage — without a vacuum, which would rewrite every other source; `destroy_base` drops the layers of a base and vacuums; the catalogue is kept.

`compact` is the end of a build: `ANALYZE`, `PRAGMA optimize`, `wal_checkpoint(TRUNCATE)` and `VACUUM` on a base, answering with the bytes before and after; `remove_osm_data_files` deletes the `osm_data` with its `-wal` and `-shm` and answers with the bytes freed.

`seal_bases` is the last line of a build: the bases it wrote and their indexes lose their write bits (`seal_path`, a file or a directory with everything in it, but tantivy's lock files, which it takes to read), and nothing ever lifts them: a base built once is final. `is_sealed` is the one test, and every writer asks it first — `base_pool` takes no row for a sealed country (dropped and counted), the house-number stage and `--recreate` leave a sealed base out, a write stage of `geolite exec` leaves a sealed base as it is and refuses one named — so `open_writable` meeting a sealed file is a programming error and panics. the catalogue is never sealed: it is the ledger of every build.

every writable connection runs in WAL mode; the schema is stamped via `PRAGMA user_version` (`SCHEMA_VERSION`) into the catalogue and into every base: a writable open refuses a file stamped with another version. `has_column` takes the schema so it can also ask an attached database; `admin_levels.merged_way_ids` is still added on open to a base that predates it.

`jsonb.rs` is the generic codec behind every `payload` column: sqlite's jsonb binary format — headers, size classes, objects and arrays — with no knowledge of what is written; each element folder's `payload` module writes its own shape with it, once per element in the extraction's decoder threads, and every reader gets it back with `JSON(payload)`. `admin_levels.merged_way_ids` is the one json column that does not go through it: a short array written a few thousand times by the street merge, bound as json text under `JSONB(?n)`.
