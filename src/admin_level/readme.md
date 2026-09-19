# admin_level

the `admin_levels` table and everything around it: a country, a state, a city, a neighborhood, a
street — each one a named area with a shape, sitting somewhere on a scale. the ddl, the queries and
the upsert live in `repository`, the rtree in `spatial_index`, the three stages that produce the
rows behind `admin_level::extract(level)`, and the connection lifecycle in `src/database/mod.rs`
calls into the first two; `src/database/admin_levels.rs`, `src/index/coordinates.rs` and
`src/extract/admin_levels/` are gone.

## the scale — `level`

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

## the identity — `admin_level_id`

an area's id is the osm id of the way or relation it came from, shifted left one bit, with the
low bit set for relations: a way and a relation that share an osm id stay distinct, the id is a
pure function of the source and never an insert-order rowid, and `osm_id()` and `kind()` read it
back. `batch_upsert` derives it, and a row with neither a way nor a relation is a programming
error that panics.

## the shape — `geometry`

`admin_geometry` is the `wkb` column: spatialite's blob layout on the way in, decoded with
`SpatiaLiteWkb` on the way out, because geozero's writer omits the byte-order byte and uses its own
sub-geometry separator, which the ISO WKB reader rejects. a blob that cannot be read degrades to an
empty geometry with a warning instead of failing the query. `mbr_center` reads the centre of the
blob's MBR header without decoding the geometry, the shortcut the house-number stage snaps with.
`bounding_box` is the envelope the rtree indexes; it moved here from `query` because the
persistence imported it, and a repository importing from the query layer is the wrong direction.

## the rtree — `spatial_index`

`admin_levels_rtree` is a second table subordinate to the first, the same arrangement
`osm_pbf_blob_chunks` has with `osm_pbf_files`: a box means nothing without the row it bounds. it
is a virtual table with no indexes of its own, dropped and recreated by every run, which is why it
has functions rather than an `impl table`. `run` pages through every row with a geometry and
inserts one box per row; on a file database up to eight readers scan disjoint id ranges in
parallel while the connection that owns the table writes, and an in-memory database is scanned on
the calling thread, because `conn.path()` is empty for it and a worker could not reopen it.

the rtree's two reads live here too. `nearest(conn, point, envelope)` is what the coordinate
service asks: the rtree narrows the streets to a window of 0.1° around the point inside the
envelope, each candidate's closest point is measured on the ground, and the answer comes most
specific level first, closest first within a level; a street whose blob is missing, is not a
line, or is empty is dropped, and the debug output counts why. `ids_in_bounding_box` is the region
filter of the text search. neither `address` nor `repository` writes sql over the rtree any more.

## the extraction — `extract`

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

each stage reads its candidates through the element repositories (`osm_relation::repository`,
`osm_way::repository`) and writes through `repository::batch_upsert`. the relations stage assembles
the member ways of each relation into rings (`geometry::assemble_rings`), closes each ring that
comes back to its start into a polygon wound clockwise, the way spatialite's `st_buildarea` does,
and falls back to a multi-line when nothing closes; a place way closes into one polygon by the same
rule; a street is always a line, even when the way is a ring.

## the rules — `rules`

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

the filter vocabulary is `osm_way::way_filter`: `rules` selects with it, and only the way's
repository knows what each variant means in sql. this table is the reference for those defaults:
a change to `rules.rs` changes it too (CLAUDE.md, rule 8).
