# house_number

a door number placed on a street: `Rua Januário dos Santos, 197`. the concept used to live in four
places with **two different rules in two languages** — the normalisation in the sql of the
extraction query, the link to the street as a bare `u8`, the token recognition in rust on the
query path, and the resolution beside it. the two rules disagreed, and the disagreement was a
bug: the extraction stored `addr:housenumber=82-52` intact while the query rejected that token,
kept only the `52`, matched nothing and fell into a meaningless interpolation. the folder took
`src/database/house_numbers.rs`, `src/extract/house_numbers.rs` and the parser of
`src/query/house_number.rs`; `src/extract/` and `src/query/` are gone with them, and what carries
data between the database and this folder on the query path is `address::house_number`.

## two forms, one number — `value`

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

## written forms are a regional policy — `policy`

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
number and the street (`number_tags`, `street_tags`, the first non-null wins), the values that
mean "no number" (`drop_values`) and the metres one number takes along a street
(`meters_per_number`: 3 by default, 1 under `brazil`, where the fixture measures a metre a number);
the preset is where a region fills it in.

## the link — `entity` and `strategy`

`house_number_link` is the row as it is written: the osm node, the street it was attached to (an
`admin_level_id`), the number, the point on the street and how the attachment was decided.
`link_strategy` is that decision — `by_name` when the node's `addr:street` named a street of the
tile, `by_proximity` otherwise — and `code()` pins the two to the `0` and `1` the column stores;
nothing reads the column back yet, so there is no reading of the code.

the link is not final: `optimize-merge-admin-levels` moves the numbers of a way it folds into
another street to the street that survives (`repository::repoint_streets`), before the way's row
goes, and the numbers of a street that `optimize-delete-isolated-admin-levels` deletes leave with
its row.

## the number in the query — `token`

`first_house_number` walks the query's tokens and returns the first one that `recognize` accepts
and that is not a word of the street's own name — so `25` in `rua 25 de marco 100` is never the
number, and the query is never stripped of digits before the search runs. `has_house_number` is
the cheap check that lets the query path skip the whole lookup when no token can be a number, and
`house_number_words` lists every token `recognize` accepts, the words the search may find no
document for.

## where a number comes from — `scenario`

every street answer carries a number, and `house_number_scenario` is the word the api says for
where it came from, `house_number.kind`, so the enum derives the json and the openapi schema
itself. `from_osm_data` is a stored number, equal to the typed one on the text path or within
50 m of the point on the coordinate path. the other three are presumed from what the street
offers: `presumed_from_multiple_references_from_street` when it stores two or more distinct
numbers, `presumed_from_one_ref_from_street` when it stores one, and `presumed_from_constants`
when it stores none. a presumed number is an estimate, and the word is how far to trust it;
measured against the 473 numbers of the santos fixture, leaving each one out in turn, the median
error is 15 m between two references, 17 m from one, and 160 to 250 m from the constants, against
the 267 m of the point of the street that answered before. the word is only the name of the
scenario: what the number was actually read from is `house_number_origin`, in `resolution`, and
the scenario is what the origin answers when asked for its word.

## the street as one line — `axis`

`street_axis` is the street straightened into one polyline, so that a number can be a distance
along it and a distance can be a number. it grows from the longest line of the geometry, in the
direction that line was drawn, through the lines that touch either end within 20 m
(`JOIN_REACH_IN_METERS`) without turning back — a continuation may bend up to 90°
(`MAX_TURN_IN_DEGREES`), read over the last and first 30 m of the two lines — the tightest turn
first, then the smallest gap. a line the axis never reaches stays off it: a parallel carriageway
meets the end through a u-turn, a branch leaves at a right angle, a loose piece is too far; a
point on any of them reads at the nearest point of the axis. without the turn limit the two
carriageways of an avenue close a loop and every distance along it doubles (measured: the error
between two references goes from 28 m to 86 m at the third quartile). a gap the axis jumps
becomes a segment of it, a bias of at most 20 m per join.

`chainage_of` is the distance along the axis to the point of it nearest to a point, with how far
off the axis the point lies; `point_at` walks the distance back into a point, clamped to the
ends. the projection scales the longitude by the cosine of the latitude, as `geometry::projected`
does, so a metre east is a metre north.

## placing it on the street — `resolution`

the **references** of a street are its stored numbers with a leading value that are not compound,
within 100 m of the axis (`REFERENCE_MAX_DISTANCE_FROM_AXIS_IN_METERS`: the far carriageway of a
wide avenue counts, a piece the axis never reached does not), one per distinct value, the first
in node id order, read as (node, value, chainage). the **factor** of a street with two or more
of them is the signed metres per number between the lowest and the highest; the **direction**
of a street with one is read from the reference itself: the numbering grows from the end of the
axis whose distance to the reference best matches the reference read as metres (`|c − v·m|`
against `|(L − c) − v·m|`, `m` the preset's `meters_per_number`), which is right for 87 of the
92 references on the clean streets of the fixture.

`place` answers the text path with a typed number: equal to a stored one, the stored point
(`from_osm_data`); compound, no number at all — `82-56` between `82-52` and `82-60` is not a
position on a line but a door nobody mapped, so the street answers bare; with two or more
references, the chainage in proportion between the nearest one below and the nearest one above,
or beyond every reference from the nearest extreme at the factor of the street; with one, the
reference's chainage plus the difference in numbers times the metres per number, in the direction
read from it; with none, the number times the metres per number from the start of the axis. the
chainage is clamped to the ends of the axis, so a number the street is too short for lands at the
end its numbering grows to.

`number_at` answers the coordinate path with a point: a stored number within 50 m of it
(`NEAREST_MAX_DISTANCE_IN_METERS`, tighter than the 100 m of the street quality on purpose, since
a number is a point and a street is a line) is read as it is; otherwise the chainage of the
street's closest point becomes a number the same way round — in proportion between the two
references around it, beyond them from the nearest one in chainage at the factor (the anchor's
value when the factor is zero), from the one reference in its direction, or the metres from the
start over the metres per number — rounded, never below one, and never in place of the street's
own point: the coordinate path reads a number, it does not move a match.

what both answer is a `house_number_resolution`: the number, its point and its
`house_number_origin`, the data the number was read from — `osm_node` the one stored node,
`references` every reference of the street in the order of their values, `reference` the one
reference and the metres per number, `constants` the metres per number alone. the origin is what
the resolution knows; `scenario()` is the word the api says for it, and `address::house_number`
reads both into the response.

## the extraction — `extract` and `linker`

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

the tiles are sorted before the fan-out and the links are sorted by node id before the insert, so
the rows land in node id order on every machine, whatever the thread count; the streets are read
in id order too, so the winner of an exact tie between two streets of the same name is the same on
every run. what the cli keeps is the run: the `--recreate` wipe before it, the index and the count
on the ledger after it.

## the repository — `repository`

the ddl, the one index (`house_numbers_search_by_admin_level_id`, the read of every query), the
candidate scan of `osm_data.osm_nodes` (`load_all_candidates`, where `normalize` runs), the
numbers of a set of streets (`numbers_by_street`, one `stored_number` per row — the osm node, the
number and its point — in node id order, the order the first-match rules of the resolution see,
whatever the insertion order, where `from_stored` runs) and the insert. the streets come through
`admin_level::repository` — `streets_with_centroid` for the tiles, `geometry_by_ids` for the
linker — so only the owner writes sql over `admin_levels`.
`osm_pbf_file::repository::update_house_numbers_count` reads the table the other way round, for
the ledger.
