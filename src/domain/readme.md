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
  http_client       the one ureq agent the slice uses
```

every folder follows the same shape: `entity` is the row, `repository` is its sql, and the value
objects and services sit alongside. everything a concept needs is in one place, and the only write
path into a table is through its entity. `osm_pbf_file` is a folder without an `entity`, for the
reason given below.

## the shape every folder holds to

- **the code carries its own explanation.** a comment is written only for a workaround or where
  there is real risk of a performance mistake. what a thing is and what it does belongs in its name
  and its type, and what a *folder* is belongs here, in this file.
- **inside a repository, the ddl comes first**: `SQL_CREATE` and `SQL_CREATE_INDEXES` grouped at
  the top, then a type named after the table that implements `domain::table` with them; the trait
  carries `create_table` and `create_indexes`, so they are never written twice. a table nobody
  drops has no `SQL_DROP`. **every other `SQL_` const sits immediately above the one function that
  uses it**, so the const travels with its query rather than piling up in a wall at the top.
- **the ddl is the exception on purpose.** it is the schema rather than a query: it is what you open
  the file to find, and it is the anchor a release is compared against, byte for byte, with
  `sed -n '/^const SQL_CREATE: /,/^";$/p'`: a difference there is a schema change, and a schema
  change bumps `SCHEMA_VERSION`.
- `table` is `pub(crate)`: only the connection lifecycle creates tables, and only the stage that
  fills a table creates its indexes. everything else a repository exposes is `pub`.
- an index is named `<table>_search_by_<purpose>`.
- **a folder's use cases are methods on one type declared in `mod.rs`** — `osm_pbf_file::list` is
  the first; `download`, `delete` and `extract` follow. the cli parses arguments, opens the
  connection and prints, nothing else.
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
| `node_count`, `way_count`, `relation_count`, `osm_data_extracted_at` | the osm-data stage |
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
creates, the database; asking for the geofabrik catalogue without one is a programming error and
panics.

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
across the per-element decoders in `src/extract/osm_data` would make the wire format depend on a
stage, which is backwards — and it would break the one thing that makes a transcription
reviewable, which is reading the field numbers side by side against the spec:
[PBF_Format](https://wiki.openstreetmap.org/wiki/PBF_Format).

**why the format lives here and `jsonb` does not.** both are codecs, and the difference is
ownership: the jsonb encoder in `src/extract/osm_data/jsonb_encode.rs` is sqlite's storage format,
used by whoever writes a payload and owned by no concept; these messages are the format of **this
file**, and the file is a concept with a folder. a format shared by nobody in particular stays
outside; a format that belongs to someone lives with them.

### the byte layout — `blob_index`

`osm_data.osm_pbf_blob_chunks` is a second table subordinate to the first, the same arrangement
`admin_levels` has with `admin_levels_rtree` in `src/database/admin_levels.rs`: a chunk is a byte
range **of a file** and means nothing without one. `blob_scanner` fills it by walking the file front
to back, reading only the length-prefixed blob headers and skipping every body — the one pass that
never decompresses anything. the extraction pipeline then reads it to know which ranges to hand each
decoder thread.

