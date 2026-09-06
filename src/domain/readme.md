# domain

> one folder per concept, each owning everything that concept needs

the domain is organised as vertical slices. a concept's folder holds its model and methods, its
policy, its services and — one patch release at a time — its own persistence. the technical
modules that came before (`extract`, `index`, `query`, `optimize`, `database`) are being emptied
into these folders and disappear as the concepts arrive.

```
osm_pbf_file/   a source `.osm.pbf` file — the `osm_pbf_files` table
  repository        the ddl, the index and the ten writes and reads
  catalog           the `source` enum, the endpoint it resolves, and the listing of each source
  download          the parallel range download and the md5 verdict
  message           the protobuf structs the file is written in
  compression       taking a blob's bytes out of it, raw or zlib
  header            the file's first blob → the `osm_header_*` columns
  blob_index        the file's byte layout — the `osm_data.osm_pbf_blob_chunks` table
  blob_scanner      the pass that walks the file and fills it
  osm_data          the pass that walks the data blobs and fills the three element tables — and
                    `tag_policy`, lodged here until `osm_tag` exists
  http_client       the one ureq agent the slice uses
osm_node/       an openstreetmap node — the `osm_data.osm_nodes` table
  entity            the node itself: id, coordinates, tags
  decoder           the pbf wire form, plain and dense, into nodes
osm_way/        an openstreetmap way — the `osm_data.osm_ways` table
  entity            the way itself: id, node references, tags
  decoder           the pbf wire form, with its delta-encoded node references
osm_relation/   an openstreetmap relation — the `osm_data.osm_relations` table
  entity            the relation, its members and their types
  decoder           the pbf wire form, with its delta-encoded member ids
```

every folder follows the same shape: `entity` is the row, `repository` is its sql, and the value
objects and services sit alongside. everything a concept needs is in one place, and the only write
path into a table is through its entity. `osm_pbf_file` is a folder without an `entity`, for the
reason given below; `osm_node`, `osm_way` and `osm_relation` arrived with their entity and decoder
only — their payload encoding and their persistence still sit in `src/database` and come with each
one's own slice.

## the shape every folder holds to

- **the code carries its own explanation.** a comment is written only for a workaround or where
  there is real risk of a performance mistake. what a thing is and what it does belongs in its name
  and its type, and what a *folder* is belongs here, in this file.
- **inside a repository, the ddl comes first**: `SQL_CREATE` and `SQL_CREATE_INDEXES` grouped at
  the top, then a type named after the table that implements `domain::table` with them; the trait
  carries `create_table` and `create_indexes`, so they are never written twice. a table nobody
  drops has no `SQL_DROP`. **every other `SQL_` const is declared inside the one function that uses
  it, as its first item**; only sql that two functions share stays at module level, so the scope of
  a query says who runs it.
- **the ddl is the exception on purpose.** it is the schema rather than a query: it is what you open
  the file to find, and it is the anchor a release is compared against, byte for byte, with
  `sed -n '/^const SQL_CREATE: /,/^";$/p'`: a difference there is a schema change, and a schema
  change bumps `SCHEMA_VERSION`.
- `table` is `pub(crate)`: only the connection lifecycle creates tables, and only the stage that
  fills a table creates its indexes. everything else a repository exposes is `pub`.
- an index is named `<table>_search_by_<purpose>`.
- **a folder's use cases are methods on one type declared in `mod.rs`** — `osm_pbf_file::list`,
  `extract_blob_chunks`, `extract_osm_header` and `extract_osm_data` are the first four; `download`
  and `delete` follow. the cli parses arguments, resolves the input, opens the connection and
  prints, nothing else.
- `mod.rs` re-exports exactly what production code outside the folder names, and nothing else.
- test scenarios are numbered contiguously from `_00` within their file, and the file names and
  scenario names are in english.

what stays outside the domain is what belongs to no concept in particular: the sqlite connection
lifecycle, the cli and the http server.

## osm_pbf_file

where every build starts: one row per `.osm.pbf` file, from the moment it is found in the geofabrik
catalogue to the counts left behind after extraction.

### the folder without an entity

the row is a **ledger written in column groups**, each by a different moment of the pipeline:

| columns | written by |
|---|---|
| `origin`, `origin_id`, `origin_name`, `url` | `catalog`, or the first stage that meets the file |
| `path`, `size_bytes`, `md5`, `downloaded_at` | `download` |
| `osm_header_*` | `header` |
| `node_count`, `way_count`, `relation_count`, `osm_data_extracted_at` | `osm_data` |
| `admin_levels_count`, `house_numbers_count` | the admin-level and house-number stages |

and it is read back only by `path`, `url`, `origin_id` and `id`. **nothing ever reads the row
whole** — the struct that could, and the query behind it, sat unused behind `#[allow(dead_code)]`
until this slice landed and deleted them. the ddl in `repository` is the shape.

### where a file comes from — `origin`

`origin` is an enum with data — `local_path(path)`, `geofabrik { id, url }`, `url(url)` — stored
as a code in the `origin` column with its payload spread over generic columns:

| `origin` | code | `origin_id` | `origin_name` | `url` | `path` |
|---|---:|---|---|---|---|
| `local_path` | 0 | none | the file name | none | the path as given |
| `geofabrik` | 1 | the catalogue id | the catalogue name | the pbf url | where the download landed |
| `url` | 2 | none | the file name in the url | the url | where the download landed |

`path` is the identity of a file on disk (`UNIQUE`); `UNIQUE (origin, origin_id)` only binds the
catalogue rows. an extract stage that meets a file first records it as `local_path`; a download of
the same path later takes the download's origin over. a `url` row whose url the catalogue turns out
to list is promoted to `geofabrik` on the next `ls`.

### the catalogue — `catalog`

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
1. caches all regions in `osm_pbf_files` (upsert by `origin_id`)
1. subsequent calls read from sqlite — skips http unless `recreate_cache` is set

### download

1. resolves the origin: geofabrik id → looks up its `url` in sqlite (fetching the index if not
   cached yet); direct url → used as-is
1. if the destination file already exists, skips the download but still verifies its md5 and
   refreshes its metadata (reuses the file)
1. splits the total size into N byte ranges and fetches them in parallel threads
1. merges parts in order into the final `.osm.pbf` file
1. verifies md5 checksum against `<url>.md5` (ok / mismatch / unavailable)
1. records the result (`path`, `size_bytes`, `md5`, `downloaded_at`) in the matching
   `osm_pbf_files` row

### the wire format — `message` and `compression`

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

### the byte layout — `blob_index`

`osm_data.osm_pbf_blob_chunks` is a second table subordinate to the first, the same arrangement
`admin_levels` has with `admin_levels_rtree` in `src/database/admin_levels.rs`: a chunk is a byte
range **of a file** and means nothing without one. `blob_scanner` fills it by walking the file front
to back, reading only the length-prefixed blob headers and skipping every body — the one pass that
never decompresses anything. `osm_data` then reads it to know which ranges to hand each decoder
thread.

### the osm data — `osm_data`

the stage behind `extract osm-pbf-data`, and the file's `extract_osm_data` use case:

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

`tag_policy` — which tag keys survive extraction: an include list, an ignore list, or neither — is
what `--tags-include-list` and `--tags-ignore-list` fill and what every decoder consults. it is not
the file's: the format does not care which tags you keep. it lodges here only because `osm_tag`,
the folder that will own the tag vocabulary, does not exist yet.

## osm_node, osm_way, osm_relation

the three element tables of the `osm_data` database, one folder each, opened with what the
`osm_data` pass needs from them: the entity and the decoder. the storage row, the jsonb payload
and the repository — ddl, index, bulk insert and the candidate queries — still live in
`src/database/osm_*.rs` and `src/database/jsonb.rs`, and move here with each folder's own slice.

a decoder takes the wire form from `osm_pbf_file::message` — the block's string table and, for
nodes, its `block_scale` (granularity and offsets) — and a `tag_policy`, and returns entities. the
dependency runs one way: the file's `osm_data` pass calls the element decoders; no element folder
knows the pass exists.
