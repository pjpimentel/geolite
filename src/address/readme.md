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
`rua 25 de marco`), so nothing is stripped before the search; the words that read as a house
number (`token::house_number_words`) are handed to the index as optional, so a number no document
holds does not cost the street covering every other word its place. with a region, the ranking is
restricted inside tantivy to the ids of the region's envelope instead of being filtered after the
fts cut, so a match of the region ranked below the global cap of fifty is not lost. each hit becomes
one match: the level ladder from `match_sources`, the point of the number when one was typed and
placed, else the street's resting point, `score` as the raw bm25 and `similarity` as the token
coverage — the fraction of the query's tokens found exactly in the document's text, a house number
counting as uncovered, so `rua x 100` scores below 1.0. the house number of each street is placed
before its matches are built, then comes the sort by score with similarity breaking the tie, then
the filters.

`query_by_coordinates` asks the rtree for the streets around the point (`RTREE_DELTA_DEG`), keeps
the lines only, projects the point onto each one (`ClosestPoint`, haversine) and sorts by level and
distance. there is no distance cap on streets: the region, `min_quality` and `last_admin_levels`
run on the candidates, whose leaf is the street itself, and the cut at ten happens before the
loads. the match carries the closest point and the distance in metres, and every street answers a
number: the stored one within 50 m of the point, else the one read at the closest point from the
street's own numbers or from the preset's metres per number, answered in `house_number` and named
by `house_number.kind`.

## the address of a path — `path`

`address::at_path(conn, id, path)` answers the one match of an area under a path already known,
the areas from the nearest one outward, without a search: it is what the tui opens at the end of
the folders walked. it holds neither the index nor the policy of the preset, so it is a function
and not a method of `address`. the match is the one the text service builds — the same ladder,
label, attributes and id, and the point of `resting_point` — without a score, a similarity or a
house number, which belong to a question asked.

what the three share is built once, in `match_sources::match_at`: the ladder, the label, the
attributes, the rounded point and the id of the path. each caller adds what is its own, the score
and the similarity of a text, the distance of a coordinate.

## the house-number step — `house_number`

the adapter between a street and `house_number`, run once per street before any match is built, so
a match is composed once and nothing is re-rendered afterwards. on the text path `from_query` has
`token::first_house_number` pick the number left after the street's own name tokens are removed and
`resolution::place` put it on the street's geometry; the match is built on the number's point,
with the number in `house_number` and in the label, and `similarity` gains +0.01 when the number
came from the street's own numbers (`from_osm_data` or presumed from two or more references): a
street split into several osm segments shares one score, and the nudge is what lifts the segment
that holds the number above the ones that presumed it from less. a text without a number, a
compound number the street does not store and a hit that is not a street answer bare. on the
coordinate path `at_point` asks `resolution::number_at` for the number at each candidate's closest
point, the candidate carrying its geometry from `spatial_index::nearest` so nothing is read twice.
the numbers of both paths come from `house_number::repository::numbers_by_street`, and `reported`
turns a resolution into the `house_number` of the response, the one place a number is read from:
the number, the word of its origin (`kind`, the `scenario()` of the origin), the osm nodes it was
read from (`osm_node_ids`) and the metres per number behind it (`meters_per_number`, `null`
unless the arithmetic used it). the ladder holds only areas, and the label reads the object, with
a template (`{house_number}`) or without.

## the response — `entity`

the seven types of the json (`query_output`, `query_service`, `query_match`, `admin_level`,
`query_match_attributes`, `query_house_number`, and `house_number_scenario`, which `house_number`
owns) are the openapi schema and keep their names; coordinates are rounded to five decimals
(`round5`). the ladder of a match is built once, in `match_sources`: the paths of the ids climbed
from the edges, the metadata of the ids and their ancestors, and the wkt only when `include_wkt`
asks for it (the polygons of countries and states are megabytes). every level of the ladder
carries its own `post_code`, `null` when the area has none. a street folded from several ways
carries `osm_merged_way_ids`, the osm way of each line of its `wkt`, in the order of the lines;
the key is absent everywhere else, and it is read on every query rather than under
`include_wkt`, because it also says which ways a street stands for when no shape is asked.

`query_house_number` is the number and where it came from: `kind` the word, `osm_node_ids` the
osm nodes the number was read from — the one node of a stored number, every reference of a street
that presumed it from several, the one reference it was presumed from, none from the constants —
and `meters_per_number` the preset's metres behind a number presumed from one reference or from
the constants, `null` for the other two. every key is always present, so a client reads the
object by its shape and not by its kind.

**one answer is one path, not one area.** a street inside two neighbourhoods answers twice, once
per path, and `matches[].id` is the uuid v5 of that path — the area ids from the root down to the
leaf, joined the way a directory path reads. it is stable while the path is, distinct between two
paths of the same area, and the same on both services. an area the hierarchy never saw still
answers once, with an empty path.

the two services still differ in two deliberate places, each at its call site rather than inside
the shared code:

| | text | coordinates |
|---|---|---|
| ancestors of one level | the chain's order | the chain reversed, so general → specific |
| the point | the number's point, else the resting point of the street | the closest point on the street |

both are the behaviour the tests pin. `attributes.post_code` is one rule on both: the first post
code met walking the path outward from the area itself, the order the label follows.

## the label — `label`

`friendly_name_format` is a template over the match: `{admin_level_<N>_name}` reads the ladder and
`{house_number}` reads `house_number`. the parse is strict — any other `{...}`, an unterminated
one or a level that is not a `u8` is an error — and it runs at the boundary
(`validate_friendly_name_format` is the cli `value_parser` and the http check), so `render` never
sees a bad template. a placeholder without a value — a level the ladder lacks, or
`{house_number}` on a bare match — swallows the literal that follows it (`"{a}, {b}, {c}"` with
`b` missing renders `a, c`) and the result is trimmed of commas and whitespace. without a template
the label is `place_label` over the match's own path: the names from the area outward, the house
number right after the area's own name, then the post codes from the root inward — one rule with
or without a number, following the path rather than the ladder, so both services write the same
label for the same path.

## the filters — `filter`

`parse_min_quality` reads the threshold at both edges — it is the cli `value_parser` of
`--min-quality` and the http check of `quality`, like `validate_friendly_name_format` — so the
filter never sees a value outside `[0, 1]`, which would silently empty or bypass the cut.

the region of `--bounding-wkt` is `admin_level::geometry::bounding_geometry`: the polygon for the
exact containment and its envelope for the rtree. the last pass of both services runs in one
order — quality (the similarity, or `1 - distance / 100 m` for a coordinate), then the exact
containment in the polygon (the rtree tested the envelope only) — and cuts at `MAX_RESULTS` (ten)
only after every filter, so no filter discards a match that would have made the cut.
`last_admin_levels` runs where each service reads its candidates, a term of the index on the text
path and the level of the street on the coordinate path: the leaf of a numbered street is the
street, so `12` keeps it, bare or numbered, and `30` is refused where the levels are parsed, like
any value outside the scale.

## what belongs elsewhere

no rule of another concept sits here any more: the similarity is
`admin_level_hierarchy::search_index::coverage`, the numbers of a street, the 50 m rule and the
presumption along the street are `house_number`'s, the rtree read behind the coordinate service is `admin_level::spatial_index::nearest`
and the wkt of a match comes from `admin_level::repository::wkt_by_ids`, the ways of a folded street
from `merged_way_ids_by_ids` beside it.
