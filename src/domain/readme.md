# domain

> one folder per concept, each owning everything that concept needs

the domain is organised as vertical slices. a concept's folder holds its model and methods, its
policy, its services and — one patch release at a time — its own persistence. the technical
modules that came before (`extract`, `index`, `query`, `optimize`, `database`) are being emptied
into these folders and disappear as the concepts arrive; `index`, `extract`, `optimize` and `query`
are gone.

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
admin_level_hierarchy/  which area contains which — the `admin_levels_hierarchy` table
  entity            the row as it is written: the chain of ancestor ids and the label
  label             the label rule: a name, its parent's finished label, its own post code
  repository        the ddl, the pending queries, the lookup by ids and the insert
  resolver          the pass that finds every area's parent and writes the chain
  search_index      the tantivy index, one document per hierarchy row, and the search over it
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
  house_number      the house-number step of both services; the rule itself is `domain/house_number`
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
whole; `admin_level_hierarchy` followed with the last of `src/index`, which no longer exists;
`house_number` took the last stage of `src/extract`, which is gone too, and is the first folder
whose rule runs on both sides of the database: the same value object normalises what the
extraction writes and recognises what the query reads.
`osm_pbf_file` is a folder without an `entity`; `osm_tag` and `address` are the two that are not a
table — the first is shared vocabulary, the second composes what the others read and writes
nothing — all for reasons given below; `osm_node`, `osm_way` and
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
  `resolve`, `download`, `extract_blob_chunks`, `extract_osm_header`, `extract_osm_data` and
  `delete`. the cli parses arguments, opens the connection and prints, nothing else.
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

what comes back out is a `level` too: every row the repository returns carries
`admin_level: level`, read through `level_of`, which skips a row whose stored value is outside the
scale with a warning — a defence, since no release ever wrote one. `u8` survives in exactly two
places, both of them edges: the `admin_levels.admin_level` column (and the tantivy term that mirrors
it) and the `level` field of the json response. a level outside the scale is refused where it is
read: `--admin-level 11`, `--last-admin-levels 11` and `?last_admin_levels=11` all answer with an
error instead of extracting or filtering nothing; `level::parse` is the one reading of a level
from text.

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

## admin_level_hierarchy

which area contains which, and the label you would read out loud —
`"Rua Castro Alves, Embaré, Santos, São Paulo, Brasil"`. the folder took `src/index/hierarchy.rs`,
`src/database/admin_levels_hierarchy.rs`, the tantivy index and the pass that built it; `src/index/`
is gone with them.

### why it is not two columns of `admin_level`

the row is one-to-one with an `admin_levels` row (`admin_level_id INTEGER PRIMARY KEY REFERENCES
admin_levels(id) ON DELETE CASCADE`) and cannot outlive it, which is the shape of a table extension.
what makes it a concept of its own is the same test that kept the rtree inside `admin_level` and
failed here on both halves:

| | `admin_levels_rtree` | `admin_levels_hierarchy` |
|---|---|---|
| what it stores | a bounding box, recomputable in milliseconds | `user_friendly_name` — **rendered content**, with regional formatting |
| who reads it | only `admin_level`'s own coordinate query | the tantivy index, the query path, and the cli's `optimize` guard |

the tantivy document is one per **hierarchy** row, not per admin level — the hierarchy row, not the
area, is the unit of search, which is why the search index lives here and not in `admin_level`.

### the chain points upward

`ancestor_ids` is a json array of admin level ids ordered from the most specific enclosing area to
the most general — `[bairro, cidade, estado, país]` — denormalised onto the child, because every
read starts from a leaf and works outward. a directory view would want the opposite traversal and is
a separate read model; nothing here provides it yet.

the chain is **sparse and not strictly ranked**: a street whose centroid falls inside no
neighbourhood attaches straight to its city, and an area may sit inside another at the same level
when that one is larger.

### the label — `label`

composed from the parent's *finished* label rather than from the chain of ids, which is what makes
it a single step — every ancestor's own post code is already inside the string it hands down.

it is not the same thing as `address::label::render_friendly_name`, which renders a user-supplied template
(`{admin_level_8_name}`) over the resolved areas at query time. the two diverge, and reconciling
them is the open `place_label` task in the backlog; `label` is where it will land.

### the resolver — `resolver`

`run` answers "who contains whom" for every area the table does not know yet. it loads every area
below street level with its rings, centroid and area into memory, builds an in-memory rtree over
their boxes, then walks the levels in ascending order so that a parent is always finished before
its children look it up. inside one level the areas resolve in parallel against the entries as
they were when the level started, so a chain between peers (a neighbourhood inside a larger one
inside the city) comes back one step short; a second pass over the level, largest area first,
re-applies the first ancestor's finished chain and propagates it transitively. streets never
contain anything, so they come last, in pages, against the same tree — on a file database up to
eight readers scan disjoint id ranges while the owning connection writes, and an in-memory
database is scanned on the calling thread, because a worker could not reopen it.

for one area the parent is, at the most specific level that has one, the smallest area whose
polygon contains the centroid, considering only a lower level or the same level with a larger
area. an area nobody contains is a root, and its label is its own name.

### the search index — `search_index`

one tantivy document per hierarchy row: the area's own name with its post code in both forms
(`01310-100` and `01310100`), and the names of every ancestor concatenated, each of the two in
three field variants — folded (lower case, no diacritics), strict (as written) and lower
(diacritics kept). a preset's abbreviations are expanded in both directions into the folded text,
so `rua` finds `r.` and back.

`search` runs two queries with a fallback: the strict one demands every token exactly, in the name
or in the ancestry, and wins when it finds anything — phrase order and the strict and lower forms
only re-rank that set, never widen it; the loose one runs only when the strict one is empty, with
exact and fuzzy terms as optional clauses, which covers a typo, an extra word or partial coverage.
the score is the raw bm25 of whichever query found the document. `last_admin_levels` and the
region filter are Must clauses with boost 0.0: they restrict the document set without touching the
score. why exact and fuzzy carry separate boosts, and why short tokens get one edit of tolerance,
is written next to the boosts in the source.

`build` still reads `admin_levels` with sql of its own, as `house_number::repository` does for its
streets — the two places in the domain that read another folder's table directly; they move behind
`admin_level::repository` when the reads are shared. `run` is what the cli's `index user-friendly-name` calls: the build, with the row count
reported around it.

## house_number

a door number placed on a street: `Rua Januário dos Santos, 197`. the concept used to live in four
places with **two different rules in two languages** — the normalisation in the sql of the
extraction query, the link to the street as a bare `u8`, the token recognition in rust on the
query path, and the resolution beside it. the two rules disagreed, and the disagreement was a
bug: the extraction stored `addr:housenumber=82-52` intact while the query rejected that token,
kept only the `52`, matched nothing and fell into a meaningless interpolation. the folder took
`src/database/house_numbers.rs`, `src/extract/house_numbers.rs` and the parser of
`src/query/house_number.rs`; `src/extract/` and `src/query/` are gone with them, and what carries
data between the database and this domain on the query path is `address::house_number`.

### two forms, one number — `value`

| | what it is | who reads it |
|---|---|---|
| `stored_form` | what gets persisted or displayed | the writer, the api response |
| `comparison_key` | aggressively canonical: no `#`, upper case, `-` between compound parts | equality, and only equality |

`normalize` is the writing side, a port of the sql `CASE` that ran at extraction: spaces trimmed,
the empty value and the preset's non-values (`s/n`) dropped, a lone trailing letter joined to the
digits in upper case (`12 a`, `12-a`, `12a` → `12A`), everything else kept as written. `recognize`
is the reading side, what the query path asks of every token: a single trailing comma tolerated,
the shape checked against the policy, the digits capped at `max_digits` so a postcode never reads
as a number. `from_stored` is the third way in, for what the table hands back. the two forms are
what let the colombian nomenclature be fixed **without a rebuild**: the comparison between a typed
number and the stored ones already happened in rust, never in sql, so the key can be computed in
memory on both sides while the bytes on disk stay exactly as they were written.

```
written "16i56"  → stored_form "16i56"  → key "16I-56"
typed   "16I56"  →                        key "16I-56"   → same number
```

### written forms are a regional policy — `policy`

`82-52` is a compound number in colombia and a range in brazil. nothing in the string tells them
apart — only the region does, so recognition is opt-in per preset:

| form | example | enabled by |
|---|---|---|
| `simple` | `123` | every preset |
| `suffixed` | `123A` (from `12 a`, `12-a`, `12a`) | every preset |
| `compound` | `82-52`, `25B-48`, `16i56` | `COMPOUND_SHAPES` — colombia |
| `#` prefix | `#82`, `# 52-48` | `allow_hash_prefix` — colombia |

a compound split across several tokens (`82 - 52`) is not recognised; only a lone `#` gets a
one-token lookahead. a hyphenated pair whose first part carries a leading zero is never compound,
which is what keeps a postcode (`01310-100`) out. the policy also names which tags carry the
number and the street (`number_tags`, `street_tags`, the first non-null wins) and the values that
mean "no number" (`drop_values`); the preset is where a region fills it in.

### the link — `entity` and `strategy`

`house_number_link` is the row as it is written: the osm node, the street it was attached to (an
`admin_level_id`), the number, the point on the street and how the attachment was decided.
`link_strategy` is that decision — `by_name` when the node's `addr:street` named a street of the
tile, `by_proximity` otherwise — and `code()` pins the two to the `0` and `1` the column stores;
nothing reads the column back yet, so there is no reading of the code.

### the number in the query — `token`

`first_house_number` walks the query's tokens and returns the first one that `recognize` accepts
and that is not a word of the street's own name — so `25` in `rua 25 de marco 100` is never the
number, and the query is never stripped of digits before the search runs. `has_house_number` is
the cheap check that lets the query path skip the whole lookup when no token can be a number.

### placing it on the street — `resolution`

`resolve` answers with the point of the stored number equal to the wanted one (`exact`); when
there is none and the number is simple or suffixed, with the linear interpolation between the
nearest stored numbers below and above it (`interpolated`); otherwise with `absent`. a compound
number is never interpolated: `82-56` between `82-52` and `82-60` is not a position on a line but
a door nobody mapped.

### the extraction — `extract` and `linker`

`house_number_link::extract(conn, policy, on_progress)` is the one way rows get into the table. it
reads every node of `osm_data.osm_nodes` that carries one of the policy's number tags, keeps the
ones `normalize` accepts, and groups them into tiles of 2°; for every tile it loads the streets
(level 12 rows with a geometry) whose mbr centre falls in the tile or its eight neighbours —
`mbr_center` is the shortcut that reads the centre without decoding the blob — and hands the
tiles round-robin to as many threads as the machine has. `linker` is the work of one tile: an
rtree over the streets' envelopes; for each candidate the street named by its `addr:street`, the
nearest of them when several share the name, and otherwise the nearest street within 0.15°; the
stored point is the candidate projected onto that street, and a street whose geometry is neither
a line nor an area is skipped. the links come back in batches of 500 through
`batch_insert_links`, with a `progress_report { total, processed }` after each — `total` counts
candidates and `processed` the rows inserted, and the count the cli prints is the rows inserted,
not the candidates: a rerun reports `0`, because `INSERT OR IGNORE` on the node id inserts nothing.

the order of the tiles is the order of a `HashMap`, so the order rows are inserted in — and the
winner of an exact tie between two streets of the same name — can differ between two runs of the
same file; the rows themselves do not. sorting the tiles before the fan-out is the open item in
the backlog. what the cli keeps is the run: the `--recreate` wipe before it, the index and the
count on the ledger after it.

### the repository — `repository`

the ddl, the one index (`house_numbers_search_by_admin_level_id`, the read of every query), the
candidate scan of `osm_data.osm_nodes` (`load_all_candidates`, where `normalize` runs), the
numbers of a set of streets (`by_admin_level_ids`, where `from_stored` runs) and the insert.
`streets_with_centroid` and `streets_wkb_by_ids` read `admin_levels` directly, the way
`search_index::build` does; they move behind `admin_level::repository` when a second consumer
appears. `osm_pbf_file::repository::update_house_numbers_count` reads the table the other way
round, for the ledger.

## address

an address resolved from what the user typed: `Rua Januário dos Santos, 197` from a text, or the
nearest streets from a `lat,lon` pair. the concept used to be `src/query`, the last technical module
with rules of its own — the shape of the response, the reading of the input, the ranking, the
filters, the label template and the two services — and it is the second folder that is not a
table: it composes what `admin_level`, `admin_level_hierarchy` and `house_number` read and writes
nothing. the move was a move: the json of every query is byte-identical to the one `src/query`
produced, checked over the same database and the same index.

### the input — `input`

`query_input::parse` reads `<lat>,<lon>` — two numbers around one comma, whitespace tolerated,
latitude within ±90 and longitude within ±180 — and everything else is a text. the reverse order
(`lon,lat`) is not detected: nothing in the string tells the two apart, so the geographic convention
wins. the domain has no dispatcher on purpose: the cli and the http server call `parse` themselves
and pick the service, because "a text without an index" is their decision — the cli exits, the http
server answers 503 and keeps answering coordinates.

### the two services — `text` and `coordinates`

`address::open(conn, index, house_numbers)` holds the connection, the optional tantivy index and
the house-number policy of the preset; `query_opts` carries what the flags carry
(`friendly_name_format`, `min_quality`, `bounding`, `last_admin_levels`, `include_wkt`) in place
of the five positional arguments the old dispatcher took.

`query_by_text` runs the fts on the whole text — a number can be part of a street name (`25` in
`rua 25 de marco`), so nothing is stripped before the search. with a region, the ranking is
restricted inside tantivy to the ids of the region's envelope instead of being filtered after the
fts cut, so a match of the region ranked below the global cap of fifty is not lost. each hit becomes
one match: the level ladder from `match_sources`, the centroid of the geometry as the point, `score`
as the raw bm25 and `similarity` as the token coverage — the fraction of the query's tokens found
exactly in the document's text, a house number counting as uncovered, so `rua x 100` scores below
1.0. the house-number step comes next, then the sort by score with similarity breaking the tie,
then the filters.

`query_by_coordinates` asks the rtree for the streets around the point (`RTREE_DELTA_DEG`), keeps
the lines only, projects the point onto each one (`ClosestPoint`, haversine) and sorts by level and
distance. there is no distance cap on streets: `min_quality` runs on the candidates and, when no
`last_admin_levels` was asked, the cut at ten happens before the loads. the match carries the
closest point and the distance in metres, and the nearest house number within 50 m is appended as
level 30.

### the house-number step — `house_number`

the adapter between a match and `domain/house_number`. on the text path `token::first_house_number`
picks the number left after the street's own name tokens are removed, `resolution::resolve` places
it, and on `exact` and `interpolated` the point moves to the number, level 30 is appended, the label
is re-rendered and `similarity` gains +0.01: a street split into several osm segments shares one
score, and the nudge is what lifts the segment that placed the number above the bare ones. on the
coordinate path only the nearest stored number within 50 m is appended — tighter than the 100 m of
the street quality on purpose, since a number is a point and a street is a line.

### the response — `entity`

the seven types of the json (`query_output`, `query_service`, `query_match`, `admin_level`,
`query_match_attributes`, `query_house_number`, `house_number_match`) are the openapi schema and
keep their names; coordinates are rounded to five decimals (`round5`). the ladder of a match is
built once, in `match_sources`: the hierarchy rows of the ids, the metadata of the ids and their
ancestors, and the wkt only when `include_wkt` asks for it (the polygons of countries and states are
megabytes). the two services still differ in three deliberate places, each at its call site rather
than inside the shared code:

| | text | coordinates |
|---|---|---|
| ancestors of one level | the chain's order | the chain reversed, so general → specific |
| `attributes.post_code` | the most specific ancestor with one | the same, then the street's own |
| the point | the centroid | the closest point on the street |

the post code rule is an open item in the backlog; the other two are the behaviour the tests pin.

### the label — `label`

`friendly_name_format` is a template over the ladder: `{admin_level_<N>_name}` and
`{house_number}` (the alias of level 30). the parse is strict — any other `{...}`, an unterminated
one or a level that is not a `u8` is an error — and it runs at the boundary
(`validate_friendly_name_format` is the cli `value_parser` and the http check), so `render` never
sees a bad template. a placeholder without a level swallows the literal that follows it
(`"{a}, {b}, {c}"` with `b` missing renders `a, c`) and the result is trimmed of commas and
whitespace. without a template the label is the one the hierarchy stored, and once a house number
is appended it is rebuilt in the default order: the street, the number, then the rest from the most
specific to the least. this renderer and `admin_level_hierarchy::label` are the two label rules the
`place_label` item in the backlog reconciles.

### the filters — `filter`

`bounding_geometry` is the region of `--bounding-wkt`: the polygon for the exact containment and
its envelope for the rtree. the last pass of both services runs in one order — quality (the
similarity, or `1 - distance / 100 m` for a coordinate), the exact containment in the polygon (the
rtree tested the envelope only), the leaf level against `last_admin_levels` — and cuts at
`MAX_RESULTS` (ten) only after every filter, so no filter discards a match that would have made the
cut.

### what still belongs elsewhere

the move was pure, and five pieces sit here until their owners take them, each an item in the
backlog: the rtree read behind `best_admin_levels` belongs to `admin_level::spatial_index`;
`bounding_geometry` and the wkt parse the http module still owns belong to `admin_level::geometry`;
`doc_text` and `token_coverage` replicate the index pipeline and belong to
`admin_level_hierarchy::search_index`; the 50 m rule and `numbers_by_street` belong to
`house_number`; the wkt load of `match_sources` belongs to `admin_level::repository`.

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

what the user types as a source is read by one rule, `input_kind::of`: `http://` or `https://`
makes a url, a `.pbf` suffix or a path separator makes a local path, anything else is a geofabrik
id — `build` and `download` used to decide this each with a heuristic of its own. `resolve(input)`
turns any of the three into the file on disk: the path as given, the name under `data_path`, or
the `path` the ledger holds for an id, a geofabrik id or a url; `local_file(input)` is its
filesystem half, for the moment in a build when the database does not exist yet.

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

`osm_pbf_file::download(&origin, threads, on_event)` is steps 2 to 6 in one call: the transfer,
the md5 verdict and the ledger row; step 1 is `resolve_geofabrik_url`, kept apart because the cli
reports it. the file is named by `origin::file_name`, the last segment of the url, the one rule the
transfer, the ledger and the progress bar share.

### delete

`osm_pbf_file::delete(path)` is the undo of the download and of the chunk index: it deletes the
file's blob chunks, clears the four download columns (`path`, `size_bytes`, `md5`,
`downloaded_at`) and removes the file. the row stays, with its header and its counts — the ledger
still says what was extracted from the file, it just no longer points at a file that is gone, so
nothing resolves to it. `optimize delete-intermediary-data` is this for every `.osm.pbf` under
`data_path`, followed by `database::remove_osm_data_files`, which drops the whole `osm_data`
sibling (chunks, nodes, ways, relations) in one go; the sibling goes last because a writable
connection recreates it.

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
to the way; which tags carry a house number belongs to the house number, and is `house_number::policy`
today. the way's rule still sits in `src/database/osm_ways.rs` and moves into its own folder, not
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
