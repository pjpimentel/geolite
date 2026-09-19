# address

an address resolved from what the user typed: `Rua Januário dos Santos, 197` from a text, or the
nearest streets from a `lat,lon` pair. the concept used to be `src/query`, the last technical module
with rules of its own — the shape of the response, the reading of the input, the ranking, the
filters, the label template and the two services — and it is the second folder that is not a
table: it composes what `admin_level`, `admin_level_hierarchy` and `house_number` read and writes
nothing. the move was a move: the json of every query is byte-identical to the one `src/query`
produced, checked over the same database and the same index.

## the input — `input`

`query_input::parse` reads `<lat>,<lon>` — two numbers around one comma, whitespace tolerated,
latitude within ±90 and longitude within ±180 — and everything else is a text. the reverse order
(`lon,lat`) is not detected: nothing in the string tells the two apart, so the geographic convention
wins. the domain has no dispatcher on purpose: the cli and the http server call `parse` themselves
and pick the service, because "a text without an index" is their decision — the cli exits, the http
server answers 503 and keeps answering coordinates.

## the two services — `text` and `coordinates`

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
1.0. the house number of each street is resolved before its matches are built, then comes the sort
by score with similarity breaking the tie, then the filters.

`query_by_coordinates` asks the rtree for the streets around the point (`RTREE_DELTA_DEG`), keeps
the lines only, projects the point onto each one (`ClosestPoint`, haversine) and sorts by level and
distance. there is no distance cap on streets: `min_quality` runs on the candidates and, when no
`last_admin_levels` was asked, the cut at ten happens before the loads. the match carries the
closest point and the distance in metres, and the nearest house number within 50 m is appended as
level 30.

## the house-number step — `house_number`

the adapter between a street and `house_number`, run once per street before any match is built, so
a match is composed once and nothing is re-rendered afterwards. on the text path `from_query` has
`token::first_house_number` pick the number left after the street's own name tokens are removed and
`resolution::resolve` place it; on `exact` and `interpolated` the match is built on the number's
point, with level 30 on the ladder and in the label, and `similarity` gains +0.01: a street split
into several osm segments shares one score, and the nudge is what lifts the segment that placed the
number above the bare ones. on the coordinate path `nearest_to` keeps only the nearest stored
number within 50 m — tighter than the 100 m of the street quality on purpose, since a number is a
point and a street is a line.

## the response — `entity`

the seven types of the json (`query_output`, `query_service`, `query_match`, `admin_level`,
`query_match_attributes`, `query_house_number`, `house_number_match`) are the openapi schema and
keep their names; coordinates are rounded to five decimals (`round5`). the ladder of a match is
built once, in `match_sources`: the paths of the ids climbed from the edges, the metadata of the
ids and their ancestors, and the wkt only when `include_wkt` asks for it (the polygons of countries
and states are megabytes).

**one answer is one path, not one area.** a street inside two neighbourhoods answers twice, once
per path, and `matches[].id` is the uuid v5 of that path — the area ids from the root down to the
leaf, joined the way a directory path reads. it is stable while the path is, distinct between two
paths of the same area, and the same on both services. an area the hierarchy never saw still
answers once, with an empty path.

the two services still differ in three deliberate places, each at its call site rather than inside
the shared code:

| | text | coordinates |
|---|---|---|
| ancestors of one level | the chain's order | the chain reversed, so general → specific |
| `attributes.post_code` | the most specific ancestor with one | the same, then the street's own |
| the point | the centroid | the closest point on the street |

the post code rule is an open item in the backlog; the other two are the behaviour the tests pin.

## the label — `label`

`friendly_name_format` is a template over the ladder: `{admin_level_<N>_name}` and
`{house_number}` (the alias of level 30). the parse is strict — any other `{...}`, an unterminated
one or a level that is not a `u8` is an error — and it runs at the boundary
(`validate_friendly_name_format` is the cli `value_parser` and the http check), so `render` never
sees a bad template. a placeholder without a level swallows the literal that follows it
(`"{a}, {b}, {c}"` with `b` missing renders `a, c`) and the result is trimmed of commas and
whitespace. without a template the label is `place_label` over the match's own path: the names from
the area outward, the house number right after the area's own name, then the post codes from the
root inward — one rule with or without a number, following the path rather than the ladder, so both
services write the same label for the same path.

## the filters — `filter`

`parse_min_quality` reads the threshold at both edges — it is the cli `value_parser` of
`--min-quality` and the http check of `quality`, like `validate_friendly_name_format` — so the
filter never sees a value outside `[0, 1]`, which would silently empty or bypass the cut.

the region of `--bounding-wkt` is `admin_level::geometry::bounding_geometry`: the polygon for the
exact containment and its envelope for the rtree. the last pass of both services runs in one order — quality (the
similarity, or `1 - distance / 100 m` for a coordinate), the exact containment in the polygon (the
rtree tested the envelope only), the leaf level against `last_admin_levels` — and cuts at
`MAX_RESULTS` (ten) only after every filter, so no filter discards a match that would have made the
cut.

## what still belongs elsewhere

the move was pure, and two pieces sit here until their owners take them, each an item in the
backlog: `doc_text` and `token_coverage` replicate the index pipeline and belong to
`admin_level_hierarchy::search_index`; the 50 m rule and `numbers_by_street` belong to
`house_number`. the rtree read behind the coordinate service is `admin_level::spatial_index::nearest`
now, and the wkt of a match comes from `admin_level::repository::wkt_by_ids`.
