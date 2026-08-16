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
  repository        the ddl, the indexes, the eleven queries and the upsert
  spatial_index     the rtree of every level's bounding box
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
path into a table is through its entity.

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
