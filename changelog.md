# changelog

## **2026-XX-XX** - 0.0.6

1. REMOVED `src/pbf`. the wire format moved into `osm_pbf_file`, which is the domain that knows how to interpret the file, as `message` and `compression`; the tag filter moved to `osm_tag::policy`, which is the vocabulary it filters and which the pbf format has no opinion about.
1. REMOVED 539 of the 554 comment lines in `src/domain`. what survives is six blocks: three workarounds and three notes where an innocent-looking change would serialise the extraction pipeline. names and types carry the rest, and folder-level explanation lives in the readme.
1. MODIFIED every repository in the domain to hold the same shape: the ddl grouped at the top, every other sql const immediately above the single function that uses it, and the ddl block always in the order create, drop, create indexes, drop indexes.
1. MODIFIED the index over `osm_pbf_blob_chunks` to carry its table's name, retiring the old name so an existing database does not keep both.
1. ADDED the file-level doc comment that six files in the domain were missing, and moved every other one above its `use` block so a file explains itself before it imports.
1. REMOVED `admin_level/scale.test.rs`, an empty file whose `mod tests;` made the level scale look covered when it has no tests at all.
1. ADDED `src/domain/osm_tag`, the shared openstreetmap tag vocabulary: the keys geolite interprets, the keys whose values are a closed set, and the sql that reads a tag out of a stored payload. it is the first folder under `domain` that is not a table.
1. REMOVED the duplicated post code rule, which was written out byte for byte in both the way and the relation repository, along with its country code sibling.
1. MODIFIED the thirteen way filters to be composed from the tag vocabulary instead of hand-written sql fragments, so a filter names a key and a value rather than a string.
1. MODIFIED `src/database/name_select.rs` into `osm_tag::select::coalesce_of`; all four of its callers were already in the domain.
1. MODIFIED the cli tag validation to use `osm_tag::key::is_valid_key`, keeping "what a key may contain" next to "how a key is written".
1. ADDED `src/domain/admin_level_hierarchy`, the seventh vertical slice: the `admin_levels_hierarchy` table now owns its ddl and queries, the containment algorithm that fills it, and the `user_friendly_name` rule. `src/index/` is down to the tantivy index alone.
1. MODIFIED `pending_total` and `pending_street_ids` to live with the hierarchy instead of with `admin_level`, which was joining a table it does not own.
1. MODIFIED the `user_friendly_name` rule into one definition, replacing the three places inside the resolver that composed it.
1. MODIFIED the rtree builder to live beside the rtree it fills, in `admin_level/spatial_index`.
1. ADDED `src/domain/osm_pbf_file`, the sixth vertical slice: the `osm_pbf_files` table now owns its ddl and queries, the geofabrik catalogue, the parallel download, the osm header reader, and the `osm_pbf_blob_chunks` byte index with the scan that fills it. `src/osm_pbf_file/` no longer exists and `src/extract/` is down to three stages.
1. REMOVED the dead row struct of `osm_pbf_files` and the query that read it, which nothing had called since they were written.
1. MODIFIED the geofabrik catalogue to take a connection instead of a path, so that the domain never opens a database of its own.
1. MODIFIED `osm_pbf_file download` to open one connection per invocation instead of two per file.
1. ADDED the three remaining pbf wire messages (`blob_header_msg`, `header_block_msg`, `header_bbox_msg`) to `src/pbf/message.rs`, where the other ten already were.
1. ADDED `src/domain/osm_relation`, the fifth vertical slice: the `osm_data.osm_relations` table now owns its entity, its pbf decoder, its jsonb payload and its repository. `src/extract/osm_data` is now just the pipeline.
1. REMOVED the dead `ToSql`/`FromSql` implementations for `osm_relation`, closing the last arm of the persistence-to-extraction dependency cycle.
1. MODIFIED `build_name_select` into its own leaf file so that a repository can use it without importing the module that creates its table.
1. ADDED `src/domain/osm_way`, the fourth vertical slice: the `osm_data.osm_ways` table now owns its entity, its pbf decoder, its jsonb payload, its extraction filters and its repository.
1. MODIFIED the way extraction filters so that the meaning (`way_filter`) lives in the domain and only its translation into sql stays in the repository.
1. REMOVED the dead `ToSql`/`FromSql` implementations for `osm_way`.
1. ADDED `src/domain/osm_node`, the third vertical slice: the `osm_data.osm_nodes` table now owns its entity, its pbf decoder, its jsonb payload and its repository.
1. ADDED `src/pbf`, the osm pbf wire format — the protobuf messages, blob decompression and the tag policy — extracted from the extraction stage so that no domain depends on a stage.
1. ADDED `src/database/jsonb.rs`, the generic sqlite jsonb writer, split out of the per-element encoders.
1. REMOVED the dead `ToSql`/`FromSql` implementations for `osm_node`, which nothing had used since the jsonb encoder replaced them.
1. ADDED `src/domain/admin_level`, the first vertical slice: the `admin_levels` table now owns its entity, its `level` scale, its identity, its geometry codec, its queries and its rtree. `src/database/admin_levels.rs` was removed.
1. REMOVED the dependency from the persistence layer to the query layer by moving `bounding_box` into the domain.
1. ADDED the `admin_level` value object to the shared kernel, replacing the `osm_admin_level` enum, the duplicated `level_name` table, the `STREET_LEVEL` constant and the level literals embedded in sql.
1. MODIFIED the presets to declare levels by name (`admin_level::city`) instead of by number.
1. MODIFIED `--admin-level`, `--last-admin-levels` and `?last_admin_levels` to reject a level outside the named scale (0, 11, 13, 15..29) instead of accepting it and matching nothing.
1. MODIFIED the query dto `admin_level` to `query_admin_level`. the json payload is unchanged; the component name in `/openapi.json` is not.
1. ADDED the `house_number` domain (`src/domain/house_number`): the value object, the per-region policy, query token extraction, the exact/interpolated/absent resolver and the street link — one definition where there used to be four.
1. ADDED the `admin_area_id` value object to the shared kernel, absorbing the way/relation id packing that lived in the persistence layer.
1. ADDED support for the colombian compound nomenclature (`82-52`, `25B-48`, `16i56`) and the `#` prefix, enabled on the colombia preset. no rebuild required — stored values are untouched.
1. MODIFIED the house number extraction query to return the raw tag value, moving trimming, the drop list and the canonical letter suffix into the domain.
1. MODIFIED the house number link strategy from a bare `u8` to `link_strategy`, keeping the stored codes unchanged.
1. REMOVED unit tests redundant with e2e tests.
1. MODIFIED release pipeline, splitting `try_publish` into `crates_publish` and `docker_publish`.
1. MODIFIED `Dockerfile` to build from `cargo install` instead of the local source tree.

## **2026-08-30** - 0.0.5

1. REMOVED unit test files from crates publish.
1. MODIFIED build command to not download pbf index.
1. ADDED initial end to end test solution.

## **2026-08-04** - 0.0.4

1. REMOVED dead code from the cli download command handler (the download runner never returns without output).
1. MODIFIED cli progress bars to stay hidden under cargo test, keeping the test output clean.
1. ADDED initial presets for guyana, paraguay, peru, suriname, uruguay, venezuela, netherlands and switzerland.
1. ADDED more tests to cli mod to cover all command handlers, clap parsing and exit paths.

## **2026-08-02** - 0.0.3

1. ADDED initial presets for chile, colombia and ecuador.
1. ADDED user-agent to http requests.
1. ADDED more tests to extract mod.

## **2026-07-11** - 0.0.2

1. REMOVED .test files from sonar analysis to reduce noise.
1. MODIFIED http-server files to fix sonar security issues.
1. MODIFIED cargo.lock to fix audit report.
1. ADDED new tests to osm_pbf_file mod to improve coverage.
1. ADDED initial presets for portugal, argentina and bolivia.
1. ADDED signal handler for shutdown (ctrl + c).

## **2026-06-28** - 0.0.1

1. first public version :)