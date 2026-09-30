# admin_level_hierarchy

which area contains which, read either way: outward from a street to the country, to say
`"Rua Castro Alves, Embaré, Santos, São Paulo, Brasil"`, and downward from the country to its
states, cities and neighbourhoods, the way a directory is walked. the folder took
`src/index/hierarchy.rs`, `src/database/admin_levels_hierarchy.rs`, the tantivy index and the pass
that built it; `src/index/` is gone with them.

## why it is not two columns of `admin_level`

an area has any number of parents — a street crossing two neighbourhoods belongs to both, a
neighbourhood straddling two cities to both — so the row is an **edge**, `(admin_level_id,
parent_id)`, and an area has one row per parent. that is a table of its own and not two columns of
`admin_levels`, which holds one row per area. the same test that kept the rtree inside
`admin_level` keeps this out of it:

| | `admin_levels_rtree` | `admin_levels_hierarchy` |
|---|---|---|
| what it stores | a bounding box, recomputable in milliseconds | which area contains which, answered by sampled geometry |
| who reads it | only `admin_level`'s own coordinate query | the tantivy index, the query path, the street merge, the cli's `optimize-*` stages and the tui |

the tantivy document is one per **path**, not per admin level — a path, not the area, is the unit
of search and the unit of an answer, which is why the search index lives here and not in
`admin_level`.

## the edge points upward, the read goes down

the table stores nothing but the edge. a root has one row with a `parent_id` of `NULL`, so an area
that is resolved always has a row and `roots()` is a read of the index rather than a scan for
absence. the index `admin_levels_hierarchy_search_by_parent` covers both directions of the walk:
`children_of(id)` and `roots()` read it, for the tui that walks the tree like folders, `parents_of(id)` reads the primary key.

everything longer than one step is derived. `paths::paths_of(id, edges)` enumerates the paths of
an area up to the roots — most specific first and without the area itself — taking the parents in
id order and, under each parent, its own paths in the order it enumerates them. the position of a
path in that list is its **ordinal**, and it is stored nowhere: the index build and the query call
the same function over the same edges, so the ordinal in a search hit names the same path the
query rebuilds. the enumeration stops at `MAX_PATHS`, counting what it dropped, and a depth cap
ends a cycle instead of looping.

the query climbs with `repository::ancestry_of`, one statement per layer up to the roots, and the
index build reads the whole table once with `load_all_edges`.

the tree is **sparse and not strictly ranked**: a street inside no neighbourhood attaches straight
to its city, and an area may sit inside another at the same level when that one is larger.

## the resolver — `resolver`

`run` answers "who contains whom" for every area the table does not know yet. it loads every area
below street level with its rings, centroid and area into memory, builds an in-memory rtree over
their boxes, then walks the levels in ascending order so that a parent is always finished before
its children look it up. each level runs in two phases: the geometry of the whole level in
parallel against the entries as they are, then a sequential pass, largest area first, that reduces
the candidates and records what sits above each area. a same-level parent is always strictly
larger, so that order is topological and nothing is read before it is written. streets never
contain anything, so they come last, in pages, against the same tree — on a file database up to
eight readers scan disjoint id ranges while the owning connection writes, and an in-memory
database is scanned on the calling thread, because a worker could not reopen it.

an area is asked about **where it is**, not where its centroid is: a polygon by a grid of points
over its box kept to its interior, a line by its vertices and the middle of each segment. a street
is a parent's child as soon as one sample falls strictly inside it, so it joins every area it
passes through; any other area has to put a tenth of its samples inside a more general candidate,
or half of them inside a peer of its own level, or a sloppily drawn neighbourhood would swallow the
one beside it. a point exactly on a border counts as neither in nor out: it promotes only a
**street**, and only to an area more specific than everything the street actually entered, which is
the street traced over the line two neighbourhoods share; two areas sharing a boundary are
neighbours, not one inside the other.

a street folded from several ways (below) is asked one line at a time: what a line that runs along
a border is promoted to depends on what that line entered, and asking the whole would let its
neighbours change the answer, so resolving a folded street again writes the edges folding wrote.

what survives is every area the child sits in, at any level, minus the ones another survivor
already hangs from — the street inside a neighbourhood hangs from the neighbourhood, not from its
city as well, while the road that leaves its neighbourhood and runs on into the next city keeps
both. an area nobody contains is a root.

the candidates are measured from the most specific outward and an area already above one that
qualified is never measured at all, because the reduction would drop it: that is what keeps the
ring of a country out of the pass. a ring longer than five hundred edges is indexed by latitude
band when it is loaded, so a point test walks a few dozen edges instead of the whole boundary of a
state.

## the isolated areas — `isolated`

an extract brings rows no address ever names: the neighbouring country whose boundary came with the
file and holds nothing, and the street the resolver found inside no area. `isolated` deletes both
after the resolver and before the street merge, so the fold and the two indexes work over the rows
that stay: it is `geolite exec optimize-delete-isolated-admin-levels`, which `build` and
`geolite merge` run between `index-admin-levels-hierarchy` and `optimize-merge-admin-levels`.

the rule names two levels and reads the edges as they are. a **country without children** is a row
of level 2 that is the `parent_id` of no edge; a **street without parents** is a row of level 12
with no edge but the root one. a child is a child whatever it holds, so a country whose states are
empty stays, and no other level is touched: a state without children and a neighbourhood without
parents are left alone. the rule runs once and not to a fixed point — a country that was the only
child of another goes, and the outer one is without children only for the next run. it has no floor
either: where every row is isolated the base ends empty, and the stage after it refuses.

the edges and the house numbers of a row leave with it, because both reference `admin_levels(id)`
and cascade on delete, and a row that goes is the parent of none that stays, so the hierarchy still
covers every row. the search index and the rtree are cleared before the first write, as the street
merge clears them; a run that finds nothing isolated touches neither.

## the street merge — `street_merge`

a street mapped in several ways answers several times under one label, and its numbers sit on
whichever way they were linked to. `street_merge` folds the ways of one street into one row after
the resolver has said which areas each way is in and before the search index is built, because the
rule needs the areas and the index needs the rows: it is `geolite exec optimize-merge-admin-levels`,
which `build` and `geolite merge` run between `optimize-delete-isolated-admin-levels` and
`index-addresses`. no index stage folds on its own.

two ways of the same name are one street when they **share an area**, their post codes agree (a
missing code agrees with any, two different codes do not) and they **touch or come within 20 m**: the
end of one is within `REACH_IN_METERS` of the line of the other, which is what an avenue with a
median puts between the two halves of the street that crosses it. the rule runs to a fixed point over
the pieces as they grow, so two pieces that share an area only through what each has already
absorbed still join, which is what a second run would find: a second run finds nothing. a street
with no area never folds, since nothing says it is one street.

the row that survives is the one with the smallest id of the piece, and it keeps that way. its
geometry is always a multi-line holding the line of every way as the way has it, its own first and
the others in id order, and `merged_way_ids` names the way of each line by position, so the ways
whose rows are gone can still be told. a way the street already holds is told by its id, not by its
coordinates, and is not folded again: two ways drawn over the same points both stay, and folding
again what was folded changes nothing. its areas are the reduced union of the areas of its ways, which is what the
resolver answers on the folded geometry. a street across two neighbourhoods is one row with two
parents and answers once under each. the numbers of a way move to the survivor before its row goes,
because `house_numbers.admin_level_id` cascades on delete, and the edges of the ways that go leave
with them.

the search index and the rtree hold one entry per row, so the command clears both before its first
write and says so; a run that finds nothing to fold touches neither. it needs the hierarchy to be
complete, and it refuses otherwise.

## the search index — `search_index`

one tantivy document per hierarchy row: the area's own name with its post code in both forms
(`01310-100` and `01310100`), and the names of every ancestor concatenated, each of the two in
three field variants — folded (lower case, no diacritics), strict (as written) and lower
(diacritics kept). a preset's abbreviations are expanded in both directions into the folded text,
so `rua` finds `r.` and back.

`search` runs three queries, each one only when the one before is empty. the strict one demands
every token exactly, in the name or in the ancestry, and wins when it finds anything — phrase order
and the strict and lower forms only re-rank that set, never widen it. the second one is the strict
one without demanding the words the caller names as optional, which the text service fills with
the words that read as a house number: `11` in `euclides da cunha 11 gonzaga santos` is a word no
document holds, so the documents covering every other word come back, and one that does hold the
number (`25` in `rua 25 de marco 100`) ranks above the ones that do not; the phrase is then the
one the demanded tokens make. it is skipped when no word is optional or none is left to demand.
the loose one takes exact and fuzzy terms as optional clauses, which covers a typo, an extra word
or partial coverage; it ranks by bm25 alone, so a document repeating half of the query outranks
one covering most of it — which is why a house number must not be what sends a query there.
the score is the raw bm25 of whichever query found the document. `last_admin_levels` and the
region filter are Must clauses with boost 0.0: they restrict the document set without touching the
score. why exact and fuzzy carry separate
boosts, and why short tokens get one edit of tolerance, is written next to the boosts in the
source.

one document per path, so the two paths of a street crossing two neighbourhoods are two
documents, and naming one of the neighbourhoods ranks its path first. a hit answers the area id
and the ordinal of the path, which is what the query needs to rebuild the same path.

`coverage(query_tokens, own, ancestors)` is the similarity the text service reports: the share of
the query's tokens found in a document's text, the text rebuilt through the same pipeline the build
runs (the name with its post code in both forms, then `tokenize`), so the similarity and the index
agree token for token.

two documents with the same score rank by area id, then by ordinal, ascending. the tie-break is
the collector's own sort key, read from the `admin_level_id` and `ordinal` fast fields inside
tantivy: the multi-threaded writer lays the documents out differently on every build, and the
scored collector prunes with block-wand and drops a document that merely ties the threshold, so a
sort after the fact would still see a different set per layout — the ids in the key are what make
the ranking a contract. `load` refuses an index missing either fast field: one built by an earlier
version reads as absent, and the cli asks for `geolite exec index-addresses`. the score in that
key is rounded to three decimals: bm25 separates two identical documents by an ulp, according to the
segment each one fell in, and the raw value would let that noise decide ahead of the ids.

`build` reads the names, post codes and levels through `admin_level::repository::load_all_names`,
so the index knows the table only through its owner. `run` is what the cli's
`geolite exec index-addresses` calls: the build, with the row count reported around it.
