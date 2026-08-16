# pbf

> the openstreetmap pbf wire format

a `.osm.pbf` file is a sequence of length-prefixed blobs. a data blob decompresses into a *primitive
block*, which carries a string table and groups of nodes, ways and relations. coordinates are
delta-encoded against the block's granularity and offsets, and tags are pairs of indexes into the
string table — which is why the block-level fields matter to every element decoder.

```
message      the protobuf structs of the format
blob         decompressing a blob (raw or zlib)
tag_policy   which tags survive extraction: an include list, an ignore list, or neither
```

this module belongs to no table and is not a pipeline stage, so it sits beside `database` rather
than inside either. the element domains read it to decode themselves; the extraction pipeline reads
it to walk the file.

not to be confused with [`osm_pbf_file`](../osm_pbf_file/readme.md), which is the geofabrik
catalogue and the download — this is the format those files are written in.
