# domain

> one folder per concept, each owning everything that concept needs

the domain is organised as vertical slices. a concept's folder holds its model and methods, its
policy, its services and — as each slice lands — its own persistence. the technical modules that
came before (`extract`, `index`, `query`, `optimize`, `database`) are being emptied into these
folders and disappear as the concepts arrive.

```
admin_level/    a named administrative area — the `admin_levels` table
  entity            the row as it is written: osm element, level, shape, name, codes
  scale             the closed set of levels, their names and their order
  id                stable identity, packed from the osm way/relation it came from
  geometry          the wkb column codec, its mbr shortcut and the bounding box
  repository        the ddl, the indexes, the nine queries and the upsert
  spatial_index     the rtree of every level's bounding box, and the pass that fills it
admin_level_hierarchy/  which area contains which — the `admin_levels_hierarchy` table
  entity            the write shape, the read shape, and the ancestor chain's encoding
  label             the `user_friendly_name` rule: own name, parent's label, post code
  repository        the ddl, the lookup, the insert and what is still pending
  resolver          the containment algorithm: point-in-polygon against an in-memory rtree
osm_tag/        the openstreetmap tag vocabulary — shared, and not a table
  key               the keys geolite interprets, their osm literal and their json path
  value             the keys whose values are a closed set: place, highway, leisure
  select            the sql that reads a tag out of a stored payload
osm_pbf_file/   a source `.osm.pbf` file — the `osm_pbf_files` table
  repository        the ddl, the index and the twelve writes and reads
  catalog           the geofabrik index, cached in the table, and the local listing
  download          the parallel range download and the md5 verdict
  header            the file's first blob → the `osm_header_*` columns
  blob_index        the file's byte layout — the `osm_data.osm_pbf_blob_chunks` table
  blob_scanner      the pass that walks the file and fills it
  http_client       the one ureq agent the slice uses
osm_node/       an openstreetmap node — the `osm_data.osm_nodes` table
  entity            the node itself, and the storage row with its encoded payload
  decoder           the pbf wire form, plain and dense, into nodes
  payload           the jsonb written into the `payload` column
  repository        the ddl, the index and the bulk insert
osm_relation/   an openstreetmap relation — the `osm_data.osm_relations` table
  entity            the relation, its members, and the storage row with its payload
  decoder           the pbf wire form, with its delta-encoded member ids
  payload           the jsonb written into the `payload` column
  repository        the ddl, the index, the bulk insert and the candidate queries
osm_way/        an openstreetmap way — the `osm_data.osm_ways` table
  entity            the way itself, and the storage row with its encoded payload
  decoder           the pbf wire form, with its delta-encoded node references
  payload           the jsonb written into the `payload` column
  filter            which ways a level wants, as meaning rather than as sql
  repository        the ddl, the index, the bulk insert and the candidate queries
house_number/   a door number placed on a street — the `house_numbers` table
  entity            the row as it is written: node, street, number, point, strategy
  value             the value object: normalize (ingestion) / recognize (query) / compare
  policy            per-region rules: tags, non-values, digit cap, written forms, `#` prefix
  strategy          how the number was attached: by_proximity | by_name
  token             finding the number inside a free-text query
  resolution        placing it on a street: exact | interpolated | absent
  repository        the ddl, the indexes, the candidate scan and the insert
```

every folder follows the same shape: `entity` is the row, `repository` is its sql, and the value
objects and services sit alongside. everything a concept needs is in one place, and the only write
path into a table is through its entity. `osm_pbf_file` is the one folder without an `entity`, for a
reason given below.

the three osm element folders are where the storage row is public rather than private to the
repository.
the extraction pipeline builds rows in several decoder threads at once and sizes its write buffer
from the encoded payload, so the encoding has to happen before the insert — `encode` is still the
only way to build a row, and it takes the element.

what stays outside the domain is what belongs to no concept in particular: the sqlite connection
lifecycle, the cli and the http server.

## admin_level

the `admin_levels` table and everything around it: a country, a state, a city, a neighborhood, a
street — each one a named area with a shape, sitting somewhere on a scale.

it is the first slice to carry its own persistence. `src/database/admin_levels.rs` no longer exists;
the ddl, the queries and the upsert live in `repository`, and the connection lifecycle in
`src/database/mod.rs` calls into it to create and drop the table.

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

**the set is closed.** `new` accepts only the levels above, so a level the scale does not name never
travels through the pipeline as a bare integer that quietly matches nothing. two consequences worth
knowing:

- `name()` returns `&'static str`, not an `Option` — there is no "unknown" to return, because an
  unnamed level cannot be constructed.
- a level outside the scale is refused where it is read: `--admin-level 11`, `--last-admin-levels 11`
  and `?last_admin_levels=11` all answer with an error instead of extracting or filtering nothing.

ordering is by the level value — a higher level is more specific, so a street sorts after the city
that contains it. this is what the hierarchy resolution and the friendly name both rely on, and it
is implemented explicitly rather than derived, so that moving a variant cannot silently change it.

`u8` survives in exactly two places, both of them edges: the `admin_levels.admin_level` column and
the `level` field of the json response.

## osm_tag

the only folder here that is **not a table**. it is shared vocabulary, the way
`admin_level::scale` is a scale rather than a row, and it sits under `domain` rather than beside
`pbf` because a tag carries meaning: `pbf` is the format, this is what the format is saying.

### two boundaries, and they matter more than the contents

**it owns the key and the shape of its value** — the osm literal, the json path, the closed value
set where there is one, the normalisation. it does **not** own what a *combination* of tags means.
"a way with a `highway` tag and no `building` tag is a street" is `osm_way::way_filter`; which tags
carry a house number is `house_number::policy`. blurring that would undo the split those slices
were built on.

**it names the interpreted vocabulary, not the stored one.** `pbf::tag_policy` keeps whatever the
file carries — an absent include list means "every tag" — and stays stringly-typed on purpose,
because the pipeline stores keys nobody here has heard of. the enum is the subset the code reasons
about, so a misspelled key is a compile error instead of a query that quietly returns nothing.

### why the path is always quoted

sqlite is forgiving about a bare key in a json path — `:` and `-` both work unquoted, which is why
the hand-written paths this replaced were correct. two characters are not forgiving:

| key | bare path | quoted path |
|---|---|---|
| `addr:postcode` | works | works |
| `ISO3166-1` | works | works |
| `a.b` | **NULL, no error** — read as a nested path | works |
| `c[1]` | **NULL, no error** — read as an array index | works |

no key osm uses today contains either, and `is_valid_key` rejects both, so this is a latent hazard
rather than a live bug. quoting always is simply the one form that cannot be silently wrong — and
it is what the name select had been doing since before this folder existed.

## admin_level_hierarchy

which area contains which, and the label you would read out loud —
`"Rua Castro Alves, Embaré, Santos, São Paulo, Brasil"`.

### why it is not two columns of `admin_level`

the row is one-to-one with an `admin_levels` row (`admin_level_id INTEGER PRIMARY KEY REFERENCES
admin_levels(id) ON DELETE CASCADE`) and cannot outlive it, which is the shape of a table extension.
what makes it a concept of its own is the same test that kept the rtree inside `admin_level` and
failed here on both halves:

| | `admin_levels_rtree` | `admin_levels_hierarchy` |
|---|---|---|
| what it stores | a bounding box, recomputable in milliseconds | `user_friendly_name` — **rendered content**, with regional formatting |
| who reads it | only `admin_level`'s own coordinate query | the tantivy index, the query path, and `optimize` |

the tantivy document is one per **hierarchy** row, not per admin level — the hierarchy row, not the
area, is the unit of search.

### the chain points upward

`ancestor_ids` is a json array of admin level ids ordered from the most specific enclosing area to
the most general — `[bairro, cidade, estado, país]` — denormalised onto the child, because every
read starts from a leaf and works outward. a directory view would want the opposite traversal and is
a separate read model; nothing here provides it yet.

the chain is **sparse and not strictly ranked**: a street whose centroid falls inside no
neighbourhood attaches straight to its city, and an area may sit inside another at the same level
when that one is larger.

### the label

composed from the parent's *finished* label rather than from the chain of ids, which is what makes
it a single step — every ancestor's own post code is already inside the string it hands down.

it is not the same thing as `query::render_friendly_name`, which renders a user-supplied template
(`{admin_level_8_name}`) over the resolved areas at query time. the two diverge, and reconciling
them is the open `place_label` task; `label` is where it will land.

## osm_pbf_file

where every build starts: one row per `.osm.pbf` file, from the moment it is found in the geofabrik
catalogue to the counts left behind after extraction.

### the folder without an entity

the row is a **ledger written in column groups**, each by a different moment of the pipeline:

| columns | written by |
|---|---|
| `geofabrik_id`, `geofabrik_name`, `geofabrik_parent`, `geofabrik_url` | `catalog` |
| `file_path`, `size_bytes`, `md5`, `downloaded_at` | `download` |
| `osm_header_*` | `header` |
| `node_count`, `way_count`, `relation_count`, `osm_data_extracted_at` | the osm-data stage |
| `admin_levels_count`, `house_numbers_count` | the admin-level and house-number stages |

and it is read back only by `file_path`, `geofabrik_url` and `id`. **nothing ever reads the row
whole** — the struct that could, and the query behind it, sat unused behind `#[allow(dead_code)]`
until this slice landed and deleted them. the ddl in `repository` is the shape.

### ls

1. fetches the geofabrik GeoJSON index from the configured endpoint
1. caches all regions in `osm_pbf_files` (upsert by `geofabrik_id`)
1. subsequent calls read from sqlite — skips http unless `recreate_cache` is set

### download

1. resolves the source: geofabrik id → looks up `geofabrik_url` in sqlite (fetching the index if not cached yet); direct url → used as-is
1. if the destination file already exists, skips the download but still verifies its md5 and refreshes its metadata (reuses the file)
1. splits the total size into N byte ranges and fetches them in parallel threads
1. merges parts in order into the final `.osm.pbf` file
1. verifies md5 checksum against `<url>.md5` (ok / mismatch / unavailable)
1. records the result (`file_path`, `size_bytes`, `md5`, `downloaded_at`) in the matching `osm_pbf_files` row

### the byte layout — `blob_index`

`osm_data.osm_pbf_blob_chunks` is a second table subordinate to the first, the same arrangement
`admin_level` uses for its rtree: a chunk is a byte range **of a file** and means nothing without
one. `blob_scanner` fills it by walking the file front to back, reading only the length-prefixed
blob headers and skipping every body — the one pass that never decompresses anything. the extraction
pipeline then reads it to know which ranges to hand each decoder thread.

## house_number

the concept used to live in four places with **two different rules in two languages**: normalisation
in sql (the extraction query), the street link as a bare `u8`, token recognition in rust (the query
path) and resolution beside it. the two rules disagreed, and the disagreement was a bug: the
ingestion stored `addr:housenumber=82-52` intact while the query rejected that token, kept only the
`52`, matched nothing and fell into a meaningless interpolation.

### two forms, one number

| | what it is | who reads it |
|---|---|---|
| `stored_form` | what gets persisted or displayed | the writer, the api response |
| `comparison_key` | aggressively canonical: no `#`, upper case, `-` between compound parts | equality, and only equality |

the split is what let the colombian nomenclature be fixed **without a rebuild**. the comparison
between a typed number and the stored ones already happened in rust, never in sql, so the key can be
computed in memory on both sides while the bytes on disk stay exactly as they were written.

```
written "16i56"  → stored_form "16i56"  → key "16I-56"
typed   "16I56"  →                        key "16I-56"   → same number
```

### written forms are a regional policy

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
which is what keeps a postcode (`01310-100`) out.
