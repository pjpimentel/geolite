# osm_tag

the only folder here that is **not a table**. it is shared vocabulary rather than a row, and it has
its own folder rather than a place inside `osm_pbf_file` because a tag carries meaning:
`osm_pbf_file::message` is the format, this is what the format is saying.

## two boundaries, and they matter more than the contents

**it owns the key and the shape of its value** — the osm literal and the json path of a key, what
a valid key looks like, the closed value set where there is one (`value`: `place`, `highway`,
`leisure`), and the sql that reads, compares, coalesces or normalises a tag (`select`). it does
**not** own what a *combination* of tags means. "a way with a `highway` tag and no `building` tag
is a street" is `osm_way::way_filter`; which tags carry a house number is `house_number::policy`.
blurring that would undo the split those slices were built on.

**it names the interpreted vocabulary, not the stored one.** `policy` is the filter the pipeline
runs while decoding — an absent include list means "every tag" — and stays stringly-typed, because
the pipeline stores keys nobody here has heard of. `key::osm_tag` is the closed, typed set of the
keys the code reasons about (`name`, `place`, `highway`, `leisure`, `building`, `waterway`, the
postcode and country aliases), so that a misspelled key is a compile error rather than a query that
quietly returns nothing; a tag chosen by a preset (`name_priority`, the house-number tags) stays a
string and goes through `coalesce_of`. one key is deliberately missing: `admin_level`. the relation
repository filters on it through an expression index, and sqlite only takes such an index when the
query spells the expression exactly as the index does — the bare `'$.tags.admin_level'` — so that
predicate is written once, as a literal, next to the index that serves it.

`policy` sits beside `key` because a policy belongs with the vocabulary it filters. it is **not**
the pbf file's — the format does not care which tags you keep, `osm_pbf_file` never reads it, and
what fills it are the `--tags-include-list` and `--tags-ignore-list` flags at the edge.

## why the path is always quoted

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
