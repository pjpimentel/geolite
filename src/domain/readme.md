# domain

> one folder per concept, each owning everything that concept needs

the domain is organised as vertical slices. a concept's folder holds its model and methods, its
policy, its services and — one patch release at a time — its own persistence. the technical
modules that came before (`extract`, `index`, `query`, `optimize`, `database`) are being emptied
into these folders and disappear as the concepts arrive.

```
admin_level/    a named administrative area — the `admin_levels` table
  entity            the row as it is written: osm element, level, shape, name, codes
  scale             the closed set of levels, their names and their order
  id                stable identity, packed from the osm way or relation it came from
  geometry          the wkb column codec, its mbr shortcut, the bounding box and the ring assembly
  repository        the ddl, the index, the nine queries and the upsert
  spatial_index     the rtree of every level's bounding box, and the pass that fills it
  rules             which ways each level includes or excludes, and the preset override
  extract           `admin_level::extract(level)`: the stages of a level and the events they emit
  relations         the stage that assembles boundary relations into areas
  place_ways        the stage that reads `place=neighbourhood|suburb` ways at level 10
  streets           the stage that reads named ways at level 12
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
  key               the shape of a key: the json path that reads it, and what a valid key looks like
  policy            which keys survive extraction: an include list, an ignore list, or neither
  select            the sql that reads a tag out of a stored payload
osm_node/       an openstreetmap node — the `osm_data.osm_nodes` table
  entity            the node itself: id, coordinates, tags
  decoder           the pbf wire form, plain and dense, into nodes
osm_way/        an openstreetmap way — the `osm_data.osm_ways` table
  entity            the way itself: id, node references, tags
  decoder           the pbf wire form, with its delta-encoded node references
osm_relation/   an openstreetmap relation — the `osm_data.osm_relations` table
  entity            the relation, its members and their types
  decoder           the pbf wire form, with its delta-encoded member ids
```

every folder follows the same shape: `entity` is the row, `repository` is its sql, and the value
objects and services sit alongside. everything a concept needs is in one place, and the only write
path into a table is through its entity. `admin_level` is the first folder whose table the queries
read, and the first to take its module out of `src/database` and its stage out of `src/extract`
whole. `osm_pbf_file` is a folder without an `entity` and
`osm_tag` the one that is not a table, both for reasons given below; `osm_node`, `osm_way` and
`osm_relation` arrived with their entity and decoder only — their payload encoding and their
persistence still sit in `src/database` and come with each one's own slice.

## the shape every folder holds to

- **the code carries its own explanation.** a comment is written only for a workaround or where
  there is real risk of a performance mistake. what a thing is and what it does belongs in its name
  and its type, and what a *folder* is belongs here, in this file.
- **inside a repository, the ddl comes first**: `SQL_CREATE` and `SQL_CREATE_INDEXES` grouped at
  the top, then a type named after the table that implements `domain::table` with them; the trait
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
  `extract_blob_chunks`, `extract_osm_header` and `extract_osm_data` are the first four; `download`
  and `delete` follow. the cli parses arguments, resolves the input, opens the connection and
  prints, nothing else.
- `mod.rs` re-exports exactly what production code outside the folder names, and nothing else.
- test scenarios are numbered contiguously from `_00` within their file, and the file names and
  scenario names are in english.

what stays outside the domain is what belongs to no concept in particular: the sqlite connection
lifecycle, the cli and the http server.

## admin_level

the `admin_levels` table and everything around it: a country, a state, a city, a neighborhood, a
street — each one a named area with a shape, sitting somewhere on a scale. the ddl, the queries and
the upsert live in `repository`, the rtree in `spatial_index`, the three stages that produce the
rows behind `admin_level::extract(level)`, and the connection lifecycle in `src/database/mod.rs`
calls into the first two; `src/database/admin_levels.rs`, `src/index/coordinates.rs` and
`src/extract/admin_levels/` are gone.

### the scale — `level`

the scale every named area sits on. osm tags levels 1..11 on boundary relations; geolite extends it
with 12 for streets, which are mapped as ways rather than boundaries, and 30 for house numbers,
which are synthesized while answering a query and never stored as an area.

| level | name | | level | name |
|---:|---|---|---:|---|
| 1 | continent | | 8 | city |
| 2 | country | | 9 | locality |
| 3 | region | | 10 | neighborhood |
| 4 | state | | 12 | street |
| 5 | district | | 14 | address |
| 6 | county | | 30 | house_number |
| 7 | municipality | | | |

**the set is closed on the way in.** `new` accepts only the levels above, and everything that
writes a level goes through it: the entity carries a `level`, the presets list `level`s, and
`--admin-level 11` is refused where it is parsed instead of travelling downstream as a number that
would quietly extract nothing. `name()` returns `&'static str`, not an `Option` — there is no
"unknown" to return, because an unnamed level cannot be constructed.

what comes back out is still a number: the rows the repository returns carry `admin_level: u8`,
because the hierarchy resolver and the query layer compare and sort them as numbers today. typing
those reads is the hierarchy slice's job, which rewrites their main consumer.

ordering is by the level value — a higher level is more specific, so a street sorts after the city
that contains it — and it is implemented explicitly rather than derived, so that moving a variant
cannot silently change it.

### the identity — `admin_level_id`

an area's id is the osm id of the way or relation it came from, shifted left one bit, with the
low bit set for relations: a way and a relation that share an osm id stay distinct, the id is a
pure function of the source and never an insert-order rowid, and `osm_id()` and `kind()` read it
back. `batch_upsert` derives it, and a row with neither a way nor a relation is a programming
error that panics.

### the shape — `geometry`

`admin_geometry` is the `wkb` column: spatialite's blob layout on the way in, decoded with
`SpatiaLiteWkb` on the way out, because geozero's writer omits the byte-order byte and uses its own
sub-geometry separator, which the ISO WKB reader rejects. a blob that cannot be read degrades to an
empty geometry with a warning instead of failing the query. `mbr_center` reads the centre of the
blob's MBR header without decoding the geometry, the shortcut the house-number stage snaps with.
`bounding_box` is the envelope the rtree indexes; it moved here from `query` because the
persistence imported it, and a repository importing from the query layer is the wrong direction.

### the rtree — `spatial_index`

`admin_levels_rtree` is a second table subordinate to the first, the same arrangement
`osm_pbf_blob_chunks` has with `osm_pbf_files`: a box means nothing without the row it bounds. it
is a virtual table with no indexes of its own, dropped and recreated by every run, which is why it
has functions rather than an `impl table`. `run` pages through every row with a geometry and
inserts one box per row; on a file database up to eight readers scan disjoint id ranges in
parallel while the connection that owns the table writes, and an in-memory database is scanned on
the calling thread, because `conn.path()` is empty for it and a worker could not reopen it.

### the extraction — `extract`

`admin_level::extract(conn, level, opts, on_event)` is the one way rows get into the table. the
domain decides what a level is made of; the caller only watches:

| level | stages | reads |
|---|---|---|
| any other | `relations` | boundary relations tagged `admin_level=N`, assembled into areas |
| 10 | `relations`, then `place_ways` | the same, then ways tagged `place=neighbourhood` or `suburb` |
| 12 | `streets` | every named way the exclude rules let through |

`stages_of(level)` returns that list as `stage { level, source, ordinal, of }`, and `extract` runs
it in order, emitting an `extract_event { stage, step }` at each step: `started`; `candidates`,
from relations stages only, with how many relations the level has and how many are not extracted
yet; `progress(progress_report)` after every batch; `finished`. an event carries its stage, so a
consumer needs no state to know what it is looking at. the cli's renderer keeps one bar per stage
and prints the lines it always printed. `extract_opts` carries the thread count of the relations
stage, the name priority and the rule overrides.

what the cli keeps is the run, not the level: the `--recreate` wipe before the loop, and after the
last level the index and the count on the ledger — the index is created once at the end because
maintaining it through the upserts is the expensive way round.

each stage reads its candidates through the legacy repositories (`database::osm_relations`,
`database::osm_ways`) and writes through `repository::batch_upsert`. the relations stage assembles
the member ways of each relation into rings (`geometry::assemble_rings`), closes each ring that
comes back to its start into a polygon wound clockwise, the way spatialite's `st_buildarea` does,
and falls back to a multi-line when nothing closes; a place way closes into one polygon by the same
rule; a street is always a line, even when the way is a ring.

### the rules — `rules`

`extraction_rules { level, include, exclude }` is what a preset can override per level
(`admin_levels_rules`); `resolve_rules` answers with the override when there is one and with the
defaults below otherwise. relations stages take no rules: they are selected by the `admin_level`
tag alone. every source drops elements without a `name`, because a nameless area cannot produce
useful data.

| level | include | because | exclude | because |
|---|---|---|---|---|
| any other, and 10 | relations tagged `admin_level=N` | the simplest selection; neighbourhood boundaries are mapped as relations too | | |
| 10 | ways tagged `place=neighbourhood` | the simplest selection | | |
| 10 | ways tagged `place=suburb` | in some regions suburbs are the de-facto neighbourhood unit when `place=neighbourhood` is not mapped | | |
| 12 | every way | the simplest selection | ways tagged `place=neighbourhood` or `place=suburb` | already captured at level 10 |
| 12 | | | ways tagged `leisure=park`, `building` or `waterway` | noise in street-level data |

the filter vocabulary (`filters`) still lives in `database::osm_ways`, where each variant is a
sql clause; it moves to `osm_way::filter` with that slice, and `rules` is its last consumer
outside `src/database`. this table is the reference for those defaults: a change to `rules.rs`
changes it too (CLAUDE.md, rule 8).

## osm_pbf_file

where every build starts: one row per `.osm.pbf` file, from the moment it is found in the geofabrik
catalogue to the counts left behind after extraction.

### the folder without an entity

the row is a **ledger written in column groups**, each by a different moment of the pipeline:

| columns | written by |
|---|---|
| `origin`, `origin_id`, `origin_name`, `url`, `origin_wkt` | `catalog`, or the first stage that meets the file |
| `path`, `size_bytes`, `md5`, `downloaded_at` | `download` |
| `osm_header_*` | `header` |
| `node_count`, `way_count`, `relation_count`, `osm_data_extracted_at` | `osm_data` |
| `admin_levels_count`, `house_numbers_count` | the admin-level and house-number stages |

and it is read back only by `path`, `url`, `origin_id` and `id`. **nothing ever reads the row
whole** — the struct that could, and the query behind it, sat unused behind `#[allow(dead_code)]`
until this slice landed and deleted them. the ddl in `repository` is the shape.

### where a file comes from — `origin`

`origin` is an enum with data — `local_path(path)`, `geofabrik { id, url }`, `url(url)` — stored
as a code in the `origin` column with its payload spread over generic columns:

| `origin` | code | `origin_id` | `origin_name` | `url` | `origin_wkt` | `path` |
|---|---:|---|---|---|---|---|
| `local_path` | 0 | none | the file name | none | none | the path as given |
| `geofabrik` | 1 | the catalogue id | the catalogue name | the pbf url | the region's coverage polygon, as wkt | where the download landed |
| `url` | 2 | none | the file name in the url | the url | none | where the download landed |

`path` is the identity of a file on disk (`UNIQUE`); `UNIQUE (origin, origin_id)` only binds the
catalogue rows. an extract stage that meets a file first records it as `local_path`; a download of
the same path later takes the download's origin over. a `url` row whose url the catalogue turns out
to list is promoted to `geofabrik` on the next `ls`, and receives its coverage with it.

### the catalogue — `catalog`

`osm_pbf_file::list(source, custom_endpoint, recreate_cache)` answers for both sources, and the
`listing` it returns has the shape of the source asked for:

| `source` | needs the database | endpoint | `listing` |
|---|---|---|---|
| `geofabrik` | yes | the enum resolves `GEOFABRIK_ENDPOINT`; `custom_endpoint` overrides it (`--ls-endpoint`) | `geofabrik(Vec<geofabrik_entry>)`: id, name, url |
| `local` | no | none | `local(Vec<local_pbf>)`: path, size in bytes |

the facade holds an optional connection so that `ls local` never opens, and therefore never
creates, the database; every other use case asks for one, and running it without one is a
programming error that panics.

for `geofabrik`:

1. fetches the geofabrik GeoJSON index from the resolved endpoint
1. caches all regions in `osm_pbf_files` (upsert by `origin_id`), each with its coverage polygon
   converted from the index's GeoJSON to wkt in `origin_wkt` — the dialect of `osm_header_bbox_wkt`
   and of the api; a region without a readable geometry keeps the column null
1. subsequent calls read from sqlite — skips http unless `recreate_cache` is set

### download

1. resolves the origin: geofabrik id → looks up its `url` in sqlite (fetching the index if not
   cached yet); direct url → used as-is
1. if the destination file already exists, skips the download but still verifies its md5 and
   refreshes its metadata (reuses the file)
1. splits the total size into N byte ranges and fetches them in parallel threads
1. merges parts in order into the final `.osm.pbf` file
1. verifies md5 checksum against `<url>.md5` (ok / mismatch / unavailable)
1. records the result (`path`, `size_bytes`, `md5`, `downloaded_at`) in the matching
   `osm_pbf_files` row

### the wire format — `message` and `compression`

a `.osm.pbf` file is a sequence of length-prefixed blobs. a data blob decompresses into a *primitive
block*, which carries a string table and groups of nodes, ways and relations. coordinates are
delta-encoded against the block's granularity and offsets, and tags are pairs of indexes into the
string table — which is why the block-level fields matter to every element decoder.

```
blob → primitive block → primitive group → node | dense nodes | way | relation
                       ↘ string table
```

the thirteen protobuf structs stay in one file because they are **nested types of one another**:
`primitive_group_msg` holds `node_msg`, `way_msg` and `relation_msg` as fields. splitting them
across the element folders' decoders would make the wire format depend on what is decoded from it,
which is backwards — and it would break the one thing that makes a transcription reviewable, which
is reading the field numbers side by side against the spec:
[PBF_Format](https://wiki.openstreetmap.org/wiki/PBF_Format).

**why the format lives here and `jsonb` does not.** both are codecs, and the difference is
ownership: the jsonb encoder in `src/database/jsonb.rs` is sqlite's storage format, used by whoever
writes a payload and owned by no concept; these messages are the format of **this file**, and the
file is a concept with a folder. a format shared by nobody in particular stays outside; a format
that belongs to someone lives with them.

### the byte layout — `blob_index`

`osm_data.osm_pbf_blob_chunks` is a second table subordinate to the first, the same arrangement
`admin_levels` has with `admin_levels_rtree` in `domain/admin_level/spatial_index.rs`: a chunk is a byte
range **of a file** and means nothing without one. `blob_scanner` fills it by walking the file front
to back, reading only the length-prefixed blob headers and skipping every body — the one pass that
never decompresses anything. `osm_data` then reads it to know which ranges to hand each decoder
thread.

### the osm data — `osm_data`

the stage behind `extract osm-pbf-data`, and the file's `extract_osm_data` use case:

1. reads every data blob range recorded in `osm_pbf_blob_chunks`
1. decompresses and decodes each blob into a primitive block, and each group into nodes (plain and
   dense), ways and relations, through the decoder of each element folder
1. encodes every element into its jsonb payload, still in the decoder threads
1. bulk-inserts the rows into `osm_nodes`, `osm_ways` and `osm_relations`, and writes `node_count`,
   `way_count`, `relation_count` and `osm_data_extracted_at` on the file's row

**one pass, not one per element.** `data_opts` selects which of the three to keep, but every blob
is inflated and decoded exactly once whatever the selection: a use case per element would read,
inflate and decode the whole file three times, and where a single decoder thread is all there is
the stage would take twice as long. that is why `extract_osm_data` takes a selection instead of
being three methods.

**the pipeline is three kinds of thread and two buffers**, and its numbers are where a performance
mistake would hide:

- one reader, `threads - 1` decoders and one writer. the reader is light — about 0.3s of work in
  an 80s run — and shares a core with the decoders instead of taking one; the writer is sqlite's
  single writer and gets a thread of its own. the reader's queue holds twice as many blobs as there
  are decoders.
- decoders push rows straight into a shared write buffer. the writer wakes at 80% of `buffer_bytes`
  (or at 10 000 rows), swaps the full buffer for an empty one under the lock and flushes outside
  it, so decoders keep pushing while the flush runs; at 100% they block until it drains. the size
  is `--buffer-limit-in-mb`, 60% of the machine's ram by default.
- a flush drains at most 100 000 rows from the front of each deque — nodes first, the table that
  usually dominates the queue, then ways, then relations — so one transaction never grows with the
  backlog. `VecDeque::drain` is O(drained): it advances the ring's head without shifting what
  remains. the bytes accounted to a flush are proportional to the rows drained: an approximation,
  good enough to throttle the decoders by.

which tag keys survive extraction is `osm_tag::tag_policy`, filled by `--tags-include-list` and
`--tags-ignore-list` and consulted by every decoder. `data_opts` carries it and never reads it: the
format does not care which tags you keep.

## osm_tag

the only folder here that is **not a table**. it is shared vocabulary rather than a row, and it sits
under `domain` rather than beside the file's format because a tag carries meaning:
`osm_pbf_file::message` is the format, this is what the format is saying.

### two boundaries, and they matter more than the contents

**it owns the key and the shape of its value** — the json path that reads a key, what a valid key
looks like, and the sql that coalesces one tag out of several. it does **not** own what a
*combination* of tags means. "a way with a `highway` tag and no `building` tag is a street" belongs
to the way; which tags carry a house number belongs to the house number. today those rules still
sit in `src/database/osm_ways.rs` and in the presets, and they move into their own folders, not
here.

**it names the interpreted vocabulary, not the stored one.** `policy` is the filter the pipeline
runs while decoding — an absent include list means "every tag" — and stays stringly-typed, because
the pipeline stores keys nobody here has heard of. the closed, typed set of the keys the code
reasons about (`name`, `admin_level`, `place`, `highway`, the postcode and country aliases) arrives
with its first consumer, the way and relation repositories, so that a misspelled key becomes a
compile error rather than a query that quietly returns nothing.

`policy` sits beside `key` because a policy belongs with the vocabulary it filters. it is **not**
the pbf file's — the format does not care which tags you keep, `osm_pbf_file` never reads it, and
what fills it are the `--tags-include-list` and `--tags-ignore-list` flags at the edge.

### why the path is always quoted

sqlite is forgiving about a bare key in a json path — `:` and `-` both work unquoted, which is why
the hand-written paths `select` replaced were correct. two characters are not forgiving:

| key | bare path | quoted path |
|---|---|---|
| `addr:postcode` | works | works |
| `ISO3166-1` | works | works |
| `a.b` | **NULL, no error** — read as a nested path | works |
| `c[1]` | **NULL, no error** — read as an array index | works |

no key osm uses today contains either, and `is_valid_key` rejects both at the cli, so this is a
latent hazard rather than a live bug. quoting always is simply the one form that cannot be silently
wrong — and it is what the name select had been doing since before this folder existed.

## osm_node, osm_way, osm_relation

the three element tables of the `osm_data` database, one folder each, opened with what the
`osm_data` pass needs from them: the entity and the decoder. the storage row, the jsonb payload
and the repository — ddl, index, bulk insert and the candidate queries — still live in
`src/database/osm_*.rs` and `src/database/jsonb.rs`, and move here with each folder's own slice.

a decoder takes the wire form from `osm_pbf_file::message` — the block's string table and, for
nodes, its `block_scale` (granularity and offsets) — and a `tag_policy`, and returns entities. the
dependency runs one way: the file's `osm_data` pass calls the element decoders; no element folder
knows the pass exists.
