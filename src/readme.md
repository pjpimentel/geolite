# src

> one folder per concept, each owning everything that concept needs

the domain is organised as vertical slices, one folder per concept directly in `src/`, beside
`cli`, `database` and `http`: the domain is the program itself, not one module among the others.
a concept's folder holds its model and methods, its policy, its services and its own persistence.
the technical modules that came before (`extract`, `index`, `query`, `optimize`) were emptied into
these folders one patch release at a time and are gone; `database` is what a driver layer is — the connection lifecycle, the schema version, the
`table` trait, the merge, the shared helpers and the generic jsonb codec — and owns no table.

```
admin_level/    a named administrative area — the `admin_levels` table
  entity            the row as it is written: osm element, level, shape, name, codes
  scale             the closed set of levels, their names and their order
  id                stable identity, packed from the osm way or relation it came from
  geometry          the wkb column codec, its mbr shortcut, the bounding box and the ring assembly
  repository        the ddl, the index, the reads every consumer of the table goes through, and the upsert
  spatial_index     the rtree of every level's bounding box, the pass that fills it, and the nearest streets to a point
  rules             which ways each level includes or excludes, and the preset override
  extract           `admin_level::extract(level)`: the stages of a level and the events they emit
  relations         the stage that assembles boundary relations into areas
  place_ways        the stage that reads `place=neighbourhood|suburb` ways at level 10
  streets           the stage that reads named ways at level 12
admin_level_hierarchy/  which area contains which — the `admin_levels_hierarchy` table
  entity            the row as it is written: one edge, child to parent
  paths             the paths of an area up to the roots, enumerated from the edges
  label             the label rule: the names outward, then the post codes inward
  repository        the ddl, the pending queries, the reads of the tree and the insert
  resolver          the pass that finds every parent of every area and writes the edges
  search_index      the tantivy index, one document per path, and the search over it
house_number/   a door number placed on a street — the `house_numbers` table
  entity            the row as it is written: node, street id, number, point, strategy
  value             the number itself: `normalize` (extraction), `recognize` (query), one comparison key
  policy            per-region rules: tags, non-values, digit cap, written forms, `#` prefix
  strategy          how the number was attached to its street: by_proximity | by_name, and the stored codes
  token             finding the number inside a free-text query, skipping the street's own name
  resolution        placing a wanted number on a street: exact | interpolated | absent
  repository        the ddl, the index, the candidate scan, the street reads, the numbers of a street and the insert
  extract           `house_number_link::extract(policy)`: candidates, tiles of 2°, the fan-out and the batches it reports
  linker            the pass over one tile: by name first, then the nearest street within 0.15°, projected onto it
address/        an address resolved from a text or from a coordinate — not a table
  entity            the response as the api renders it: the match, its level ladder, its attributes, built from one set of loads
  input             what the user typed: a `lat,lon` pair or a text
  label             the friendly-name template: parse, validate, render, and the default order
  filter            the region of `--bounding-wkt`, and the shared last pass: quality, region, last levels, the cut at ten
  text              `address::query_by_text`: the search, the matches, the house number, the sort
  coordinates       `address::query_by_coordinates`: the nearest streets, the matches, the nearest number
  house_number      the house-number step of both services; the rule itself is `house_number`
osm_pbf_file/   a source `.osm.pbf` file — the `osm_pbf_files` table
  repository        the ddl, the index and the ten writes and reads
  catalog           the `source` enum, the endpoint it resolves, and the listing of each source
  download          the parallel range download and the md5 verdict
  message           the protobuf structs the file is written in
  compression       taking a blob's bytes out of it, raw or zlib
  header            the file's first blob → the `osm_header_*` columns
  blob_index        the file's byte layout — the `osm_data.osm_pbf_blob_chunks` table
  blob_scanner      the pass that walks the file and fills it
  osm_data          the pass that walks the data blobs and fills the three element tables
  http_client       the one ureq agent the slice uses
osm_tag/        the openstreetmap tag vocabulary — shared, and not a table
  key               the keys the code reasons about, their osm literal and their json path; what a valid key looks like
  value             the keys whose values are a closed set: place, highway, leisure
  policy            which keys survive extraction: an include list, an ignore list, or neither
  select            the sql that reads a tag out of a stored payload, compares it or normalises it
osm_node/       an openstreetmap node — the `osm_data.osm_nodes` table
  entity            the node itself: id, coordinates, tags; and the storage row with its encoded payload
  decoder           the pbf wire form, plain and dense, into nodes
  payload           the jsonb written into the `payload` column
  repository        the ddl, the index and the bulk insert
osm_way/        an openstreetmap way — the `osm_data.osm_ways` table
  entity            the way itself: id, node references, tags; and the storage row with its encoded payload
  decoder           the pbf wire form, with its delta-encoded node references
  payload           the jsonb written into the `payload` column
  filter            which ways a level wants, as meaning rather than as sql
  repository        the ddl, the index, the bulk insert, the candidate query and the coordinates of a way
osm_relation/   an openstreetmap relation — the `osm_data.osm_relations` table
  entity            the relation, its members and their types; and the storage row with its encoded payload
  decoder           the pbf wire form, with its delta-encoded member ids
  payload           the jsonb written into the `payload` column
  repository        the ddl, the index, the bulk insert, the candidate queries and the coordinates of a relation
```

every folder follows the same shape: `entity` is the row, `repository` is its sql, and the value
objects and services sit alongside. everything a concept needs is in one place, and the only write
path into a table is through its entity. `admin_level` is the first folder whose table the queries
read, and the first to take its module out of `src/database` and its stage out of `src/extract`
whole; `admin_level_hierarchy` followed with the last of `src/index`, which no longer exists;
`house_number` took the last stage of `src/extract`, which is gone too, and is the first folder
whose rule runs on both sides of the database: the same value object normalises what the
extraction writes and recognises what the query reads.
`osm_pbf_file` is a folder without an `entity`; `osm_tag` and `address` are the two that are not a
table — the first is shared vocabulary, the second composes what the others read and writes
nothing — all for reasons given in each folder's `readme.md`; `osm_node`, `osm_way` and `osm_relation` were the last to
take their persistence out of `src/database`, which owns no table any more.

## the shape every folder holds to

- **the code carries its own explanation.** a comment is written only for a workaround or where
  there is real risk of a performance mistake. what a thing is and what it does belongs in its name
  and its type, and what a *folder* is belongs in its `readme.md`.
- **inside a repository, the ddl comes first**: `SQL_CREATE` and `SQL_CREATE_INDEXES` grouped at
  the top, then a type named after the table that implements `database::table` with them; the trait
  carries `create_table` and `create_indexes`, so they are never written twice. a table nobody
  drops has no `SQL_DROP`. **every other `SQL_` const is declared inside the one function that uses
  it, as its first item**; only sql that two functions share stays at module level, so the scope of
  a query says who runs it.
- **the ddl is the exception on purpose.** it is the schema rather than a query: it is what you open
  the file to find, and it is the anchor a release is compared against, byte for byte, with
  `sed -n '/^const SQL_CREATE: /,/^";$/p'`: a difference there is a schema change. one that only
  adds a nullable column is applied to existing databases by the connection lifecycle — an
  `ALTER TABLE` guarded by `database::has_column`, as `add_origin_wkt` does — and keeps
  `SCHEMA_VERSION`; a change that makes builds incompatible bumps it.
- `table` is `pub(crate)`: only the connection lifecycle creates tables, and only the stage that
  fills a table creates its indexes. everything else a repository exposes is `pub`.
- an index is named `<table>_search_by_<purpose>`.
- **a folder's use cases are methods on one type declared in `mod.rs`** — `osm_pbf_file::list`,
  `resolve`, `download`, `extract_blob_chunks`, `extract_osm_header`, `extract_osm_data` and
  `delete`. the cli parses arguments, opens the connection and prints, nothing else.
- `mod.rs` re-exports exactly what production code outside the folder names, and nothing else.
- test scenarios are numbered contiguously from `_00` within their file, and the file names and
  scenario names are in english.

what stays outside the concept folders is what belongs to no concept in particular: the sqlite
connection lifecycle with the `table` trait, the cli and the http server.
