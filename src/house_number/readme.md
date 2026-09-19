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
number and the street (`number_tags`, `street_tags`, the first non-null wins) and the values that
mean "no number" (`drop_values`); the preset is where a region fills it in.

## the link — `entity` and `strategy`

`house_number_link` is the row as it is written: the osm node, the street it was attached to (an
`admin_level_id`), the number, the point on the street and how the attachment was decided.
`link_strategy` is that decision — `by_name` when the node's `addr:street` named a street of the
tile, `by_proximity` otherwise — and `code()` pins the two to the `0` and `1` the column stores;
nothing reads the column back yet, so there is no reading of the code.

## the number in the query — `token`

`first_house_number` walks the query's tokens and returns the first one that `recognize` accepts
and that is not a word of the street's own name — so `25` in `rua 25 de marco 100` is never the
number, and the query is never stripped of digits before the search runs. `has_house_number` is
the cheap check that lets the query path skip the whole lookup when no token can be a number.

## placing it on the street — `resolution`

`resolve` answers with the point of the stored number equal to the wanted one (`exact`); when
there is none and the number is simple or suffixed, with the linear interpolation between the
nearest stored numbers below and above it (`interpolated`); otherwise with `absent`. a compound
number is never interpolated: `82-56` between `82-52` and `82-60` is not a position on a line but
a door nobody mapped.

`nearest` answers the other way round, for the coordinate path: the stored number closest to a
point, within 50 m — tighter than the 100 m of the street quality on purpose, since a number is a
point and a street is a line.

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
numbers of a set of streets with their points (`numbers_by_street`, in node id order — the order
the first-match rules of the resolution see, whatever the insertion order — where `from_stored`
runs) and the
insert. the streets come through `admin_level::repository` — `streets_with_centroid` for the
tiles, `geometry_by_ids` for the linker — so only the owner writes sql over `admin_levels`.
`osm_pbf_file::repository::update_house_numbers_count` reads the table the other way round, for
the ledger.
