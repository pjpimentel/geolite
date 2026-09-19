# osm_node, osm_way, osm_relation

the three element tables of the `osm_data` database, one folder each: the entity and its decoder,
the storage row, the jsonb payload and the repository — ddl, index, bulk insert and, for ways and
relations, the candidate queries the admin-level stages read.

a decoder takes the wire form from `osm_pbf_file::message` — the block's string table and, for
nodes, its `block_scale` (granularity and offsets) — and a `tag_policy`, and returns entities. the
dependency runs one way: the file's `osm_data` pass calls the element decoders; no element folder
knows the pass exists.

## the payload — `payload` and the row

a row is `(id, osm_pbf_chunk_id, payload)`, and `payload` is sqlite's jsonb binary written by hand.
`database::jsonb` is the generic codec — headers, size classes, objects and arrays, a scratch pool —
and each folder's `payload::encode` is the shape: `lat`, `lon` and `tags` for a node, `refs` and
`tags` for a way, `tags` and `members` (`type` as `n`/`w`/`r`, `id`, `role`) for a relation. the
codec knows no entity, and the paths every reader uses (`$.tags.…`, `$.refs`, `$.members`) are
fixed by the writer in the same folder. the row is encoded in the decoder threads
(`osm_node_row::encode` and its siblings), not at insert time: the single writer thread only binds
blobs, which is what keeps the pipeline parallel. the three inserts share
`database::insert_in_chunks`, one multi-row statement per ten thousand rows, sized under sqlite's
variable limit.

## the filters — `osm_way::filter` and `repository`

`way_filter` is the meaning — which ways a level wants (`include_place_neighbourhood`,
`exclude_building`, …); `admin_level::rules` selects with it and a preset may override it. the sql
of each variant lives in the way's repository alone, built from `osm_tag::select` over typed keys
and values, so the vocabulary and its translation are separate and the translation has one owner.

## the candidate queries and the expression index

`osm_relation::repository` answers which relations of a level are still to index and
`osm_way::repository` which ways; both anti-join `main.admin_levels`. the relation queries filter
on the `admin_level` tag through the expression index `osm_relations_search_by_admin_level` and
keep that expression as a bare literal, for the reason given under `osm_tag`: wrapping it makes
sqlite drop the index and scan every relation of the file.
