# query
> resolve a free-text or coordinate input into an address

## coordinates

1. parses `lat,lon` from the input string
2. queries `admin_levels_rtree` with a coarse bbox, then ranks candidates by haversine distance to the closest point on each street
3. returns the matching streets sorted by distance (closest first); there is no fixed distance cap on streets — only the optional `--min-quality` / `--bounding-wkt` filters remove candidates (contrast the 50 m cap on house numbers in step 4)
4. enriches each match with the nearest house number (if within 50 m)

## address

0. the input is **not** stripped of numbers: the fts runs on the full text (step 2), because a
   number can be part of the street name (`25` in `rua 25 de marco`) and removing it would break
   the match. a house number is shaped as 1–5 digits with at most one trailing letter (`123`,
   `123a`), a single trailing comma tolerated (`35,`), no hyphen — so postcodes (`01310-100`,
   `01310100`) are never treated as house numbers. resolution happens per candidate in step 5.
1. tokenises the input (lowercase + ascii-fold)
2. searches the tantivy index `admin_levels_hierarchy_tantivy` with a two-stage strategy:
   - **Q1 strict**: every query token must match exactly in `name` or `hier` (`Must` per token). returns only docs that cover 100% of the query exactly. if non-empty, this is the result. on top of that backbone, Q1 adds `Should` boosts — a phrase-order bonus and case/diacritic-sensitive matches against the `*_strict` / `*_lower` field variants — that only re-rank the matching set, never widen it.
   - **Q2 loose** (only if Q1 is empty): per-token `Should` clauses with exact + fuzzy on both fields. handles typos, extra words, or partial coverage.
3. loads geometry, hierarchy and metadata for each hit
4. orders by BM25 `score` (primary), breaking ties by `similarity` — so two segments of the same street (same score) are ordered by similarity, which the house-number nudge in step 5 feeds. returns up to 10 matches with two relevance signals:
   - `similarity` = token coverage (fraction of query tokens that appear exactly in the doc's indexed text); 1.0 means every word of the query was found. a house number counts as an uncovered token, so `rua x 100` scores below 1.0
   - `score` = raw BM25 score from tantivy (no transformation); unbounded, only meaningful relative to other matches in the same response
5. resolves a house number against the `house_numbers` of each street match (by value, not
   proximity). per street, the street's own name tokens are removed from the query (so every
   number that belongs to the name — `25` and `2024` in `rua 25 de marco de 2024` — is ignored)
   and the **first** remaining numeric token is taken as the house number, then resolved into
   `house_number.kind`:
   - **exact**: a stored number equals it; the match coordinate moves to the house-number point
   - **interpolated**: no exact match, but it falls between two known numbers on the street —
     the coordinate is a linear interpolation between their points
   - **absent**: a number was taken but couldn't be placed (the street is still returned, unchanged)

   when no numeric token remains for a street, `house_number` is omitted. on `exact`/`interpolated`
   the number is appended as an `admin_level` (level 30), `friendly_name` is re-rendered to include
   it, and `similarity` gets a +0.01 nudge. a street split into several osm segments shares the same
   name (same score), so this nudge is what lifts the segment that placed the number above the bare
   ones in the tie-break of step 4 — both are still returned.

## house_number

enriches an existing set of street matches with the closest house number node.
every stored house number comes from an osm node; the `strategy` column records how it was linked to its street:

- **by_proximity** snapped to the nearest street geometry
- **by_name** its `addr:street` tag matched the street's name

## filters

every result set ends with a shared pass (both the coordinate and address paths):

- `--min-quality` drops matches below a quality threshold (distance-derived for coordinates, token-coverage for text)
- `--bounding-wkt` keeps only matches whose final point falls inside the polygon
- `--last-admin-levels` keeps only matches whose leaf admin level is in the set
- `--include-wkt` attaches each level's WKT geometry to the output (off → omitted)

results are then truncated to `MAX_RESULTS` (10).
