# osm_pbf_file

where every build starts: one row per `.osm.pbf` file, from the moment it is found in the geofabrik
catalogue to the counts left behind after extraction.

## the folder without an entity

the row is a **ledger written in column groups**, each by a different moment of the pipeline:

| columns | written by |
|---|---|
| `origin`, `origin_id`, `origin_name`, `url`, `origin_wkt` | `catalog`, or the first stage that meets the file |
| `path`, `size_bytes`, `md5`, `downloaded_at` | `download` |
| `osm_header_*` | `header` |
| `node_count`, `way_count`, `relation_count`, `osm_data_extracted_at` | `osm_data` |
| `admin_levels_count`, `house_numbers_count` | the admin-level and house-number stages |

and it is read back only by `path`, `url`, `origin_id` and `id`. **nothing ever reads the row
whole** — the struct that could, and the query behind it, sat unused behind `#[allow(dead_code)]`
until this slice landed and deleted them. the ddl in `repository` is the shape.

## where a file comes from — `origin`

`origin` is an enum with data — `local_path(path)`, `geofabrik { id, url }`, `url(url)` — stored
as a code in the `origin` column with its payload spread over generic columns:

| `origin` | code | `origin_id` | `origin_name` | `url` | `origin_wkt` | `path` |
|---|---:|---|---|---|---|---|
| `local_path` | 0 | none | the file name | none | none | the path as given |
| `geofabrik` | 1 | the catalogue id | the catalogue name | the pbf url | the region's coverage polygon, as wkt | where the download landed |
| `url` | 2 | none | the file name in the url | the url | none | where the download landed |

`path` is the identity of a file on disk (`UNIQUE`); `UNIQUE (origin, origin_id)` only binds the
catalogue rows. an extract stage that meets a file first records it as `local_path`; a download of
the same path later takes the download's origin over. a `url` row whose url the catalogue turns out
to list is promoted to `geofabrik` on the next `ls`, and receives its coverage with it.

## the catalogue — `catalog`

`osm_pbf_file::list(source, custom_endpoint, recreate_cache)` answers for both sources, and the
`listing` it returns has the shape of the source asked for:

| `source` | needs the database | endpoint | `listing` |
|---|---|---|---|
| `geofabrik` | yes | the enum resolves `GEOFABRIK_ENDPOINT`; `custom_endpoint` overrides it (`--ls-endpoint`) | `geofabrik(Vec<geofabrik_entry>)`: id, name, url |
| `local` | no | none | `local(Vec<local_pbf>)`: path, size in bytes |

the facade holds an optional connection so that `ls local` never opens, and therefore never
creates, the database; every other use case asks for one, and running it without one is a
programming error that panics.

for `geofabrik`:

1. fetches the geofabrik GeoJSON index from the resolved endpoint
1. caches all regions in `osm_pbf_files` (upsert by `origin_id`), each with its coverage polygon
   converted from the index's GeoJSON to wkt in `origin_wkt` — the dialect of `osm_header_bbox_wkt`
   and of the api; a region without a readable geometry keeps the column null
1. subsequent calls read from sqlite — skips http unless `recreate_cache` is set

what the user types as a source is read by one rule, `input_kind::of`: `http://` or `https://`
makes a url, a `.pbf` suffix or a path separator makes a local path, anything else is a geofabrik
id — `build` and `download` used to decide this each with a heuristic of its own. `resolve(input)`
turns any of the three into the file on disk: the path as given, the name under `data_path`, or
the `path` the ledger holds for an id, a geofabrik id or a url; `local_file(input)` is its
filesystem half, for the moment in a build when the database does not exist yet.

## download

1. resolves the origin: geofabrik id → looks up its `url` in sqlite (fetching the index if not
   cached yet); direct url → used as-is
1. if the destination file already exists, skips the download but still verifies its md5 and
   refreshes its metadata (reuses the file)
1. splits the total size into N byte ranges and fetches them in parallel threads
1. merges parts in order into the final `.osm.pbf` file
1. verifies md5 checksum against `<url>.md5` (ok / mismatch / unavailable)
1. records the result (`path`, `size_bytes`, `md5`, `downloaded_at`) in the matching
   `osm_pbf_files` row

`osm_pbf_file::download(&origin, threads, on_event)` is steps 2 to 6 in one call: the transfer,
the md5 verdict and the ledger row; step 1 is `resolve_geofabrik_url`, kept apart because the cli
reports it. the file is named by `origin::file_name`, the last segment of the url, the one rule the
transfer, the ledger and the progress bar share.

## delete

`osm_pbf_file::delete(path)` is the undo of the download and of the chunk index: it deletes the
file's blob chunks, clears the four download columns (`path`, `size_bytes`, `md5`,
`downloaded_at`) and removes the file. the row stays, with its header and its counts — the ledger
still says what was extracted from the file, it just no longer points at a file that is gone, so
nothing resolves to it. `optimize-delete-intermediary-data` is this for every `.osm.pbf` under
`data_path`, followed by `database::remove_osm_data_files`, which drops the whole `osm_data`
sibling (chunks, nodes, ways, relations) in one go; the sibling goes last because a writable
connection recreates it.

## the wire format — `message` and `compression`

a `.osm.pbf` file is a sequence of length-prefixed blobs. a data blob decompresses into a *primitive
block*, which carries a string table and groups of nodes, ways and relations. coordinates are
delta-encoded against the block's granularity and offsets, and tags are pairs of indexes into the
string table — which is why the block-level fields matter to every element decoder.

```
blob → primitive block → primitive group → node | dense nodes | way | relation
                       ↘ string table
```

the thirteen protobuf structs stay in one file because they are **nested types of one another**:
`primitive_group_msg` holds `node_msg`, `way_msg` and `relation_msg` as fields. splitting them
across the element folders' decoders would make the wire format depend on what is decoded from it,
which is backwards — and it would break the one thing that makes a transcription reviewable, which
is reading the field numbers side by side against the spec:
[PBF_Format](https://wiki.openstreetmap.org/wiki/PBF_Format).

**why the format lives here and `jsonb` does not.** both are codecs, and the difference is
ownership: the jsonb encoder in `src/database/jsonb.rs` is sqlite's storage format, used by whoever
writes a payload and owned by no concept; these messages are the format of **this file**, and the
file is a concept with a folder. a format shared by nobody in particular stays outside; a format
that belongs to someone lives with them.

## the byte layout — `blob_index`

`osm_data.osm_pbf_blob_chunks` is a second table subordinate to the first, the same arrangement
`admin_levels` has with `admin_levels_rtree` in `admin_level/spatial_index.rs`: a chunk is a byte
range **of a file** and means nothing without one. `blob_scanner` fills it by walking the file front
to back, reading only the length-prefixed blob headers and skipping every body — the one pass that
never decompresses anything. `osm_data` then reads it to know which ranges to hand each decoder
thread.

## the osm data — `osm_data`

the stage behind `geolite exec extract-osm-pbf-data`, and the file's `extract_osm_data` use case:

1. reads every data blob range recorded in `osm_pbf_blob_chunks`
1. decompresses and decodes each blob into a primitive block, and each group into nodes (plain and
   dense), ways and relations, through the decoder of each element folder
1. encodes every element into its jsonb payload, still in the decoder threads
1. bulk-inserts the rows into `osm_nodes`, `osm_ways` and `osm_relations`, and writes `node_count`,
   `way_count`, `relation_count` and `osm_data_extracted_at` on the file's row

**one pass, not one per element.** `data_opts` selects which of the three to keep, but every blob
is inflated and decoded exactly once whatever the selection: a use case per element would read,
inflate and decode the whole file three times, and where a single decoder thread is all there is
the stage would take twice as long. that is why `extract_osm_data` takes a selection instead of
being three methods.

**the pipeline is three kinds of thread and two buffers**, and its numbers are where a performance
mistake would hide:

- one reader, `threads - 1` decoders and one writer. the reader is light — about 0.3s of work in
  an 80s run — and shares a core with the decoders instead of taking one; the writer is sqlite's
  single writer and gets a thread of its own. the reader's queue holds twice as many blobs as there
  are decoders.
- decoders push rows straight into a shared write buffer. the writer wakes at 80% of `buffer_bytes`
  (or at 10 000 rows), swaps the full buffer for an empty one under the lock and flushes outside
  it, so decoders keep pushing while the flush runs; at 100% they block until it drains. the size
  is `--buffer-limit-in-mb`, 60% of the machine's ram by default.
- a flush drains at most 100 000 rows from the front of each deque — nodes first, the table that
  usually dominates the queue, then ways, then relations — so one transaction never grows with the
  backlog. `VecDeque::drain` is O(drained): it advances the ring's head without shifting what
  remains. the bytes accounted to a flush are proportional to the rows drained: an approximation,
  good enough to throttle the decoders by.

which tag keys survive extraction is `osm_tag::tag_policy`, filled by `--tags-include-list` and
`--tags-ignore-list` and consulted by every decoder. `data_opts` carries it and never reads it: the
format does not care which tags you keep.
