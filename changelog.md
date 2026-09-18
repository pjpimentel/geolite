# changelog

## **2026-XX-XX** - 0.0.11

1. REMOVED unit tests.
1. REMOVED the columns of `admin_levels_hierarchy` that the edges already derive.
1. MODIFIED `admin_levels_hierarchy` to one edge per row, with every parent that contains an area.
1. MODIFIED `query`, `/geocode` and the search index to answer one match per path.
1. MODIFIED `SCHEMA_VERSION` to 3: an existing database and an index built by 0.0.10 have to be rebuilt.
1. ADDED the reads that walk the hierarchy downward.

## **2026-09-14** - 0.0.10

1. REMOVED the element tables from `database`, which owns no table any more.
1. MODIFIED `query` and the search index to rank two hits with the same score the same way on every build.
1. MODIFIED `extract osm-house-numbers` to link a street's numbers in the same order on every build.
1. MODIFIED `/geocode` to refuse an invalid `quality`, and `http-server` to report the port it bound.
1. ADDED the payload and the repository of `osm_node`, `osm_way` and `osm_relation`.
1. ADDED the typed `osm_tag` selects behind the way filters and the tag aliases.

## **2026-09-13** - 0.0.9

1. REMOVED the `house_numbers`, `extract`, `optimize` and `query` mods.
1. MODIFIED the presets to carry a house number policy.
1. MODIFIED `query` and `http-server` to read a compound house number from the input and never interpolate it.
1. ADDED the `house_number` and `address` domains, with their use cases.
1. ADDED the resolve, download and delete use cases of `osm_pbf_file`.

## **2026-09-12** - 0.0.8

1. REMOVED the `admin_levels`, `admin_levels_hierarchy`, `extract/admin_levels` and `index` mods.
1. MODIFIED the cli and the http server to refuse an admin level outside the scale.
1. ADDED the `admin_level` and `admin_level_hierarchy` domains, with their use cases.

## **2026-09-07** - 0.0.7

1. REMOVED the `extract/osm_data` mod.
1. MODIFIED `extract osm-pbf-data` to take an explicit value on every include flag and to keep the blob chunk index on `--recreate`.
1. ADDED the `osm_node`, `osm_way`, `osm_relation` and `osm_tag` domains.
1. ADDED `origin_wkt` to `osm_pbf_files`, the coverage polygon of every geofabrik region.

## **2026-09-06** - 0.0.6

1. REMOVED the unit tests redundant with the e2e tests.
1. REMOVED the `osm_pbf_file` mod.
1. MODIFIED the release pipeline, splitting the crates publish from the docker publish.
1. MODIFIED `build` to accept a direct url as source.
1. ADDED the `osm_pbf_file` domain (not backward compatible).

## **2026-08-30** - 0.0.5

1. REMOVED the unit test files from the crates publish.
1. MODIFIED `build` to not download the pbf index.
1. ADDED the end to end test solution.

## **2026-08-04** - 0.0.4

1. REMOVED dead code from the cli download command.
1. MODIFIED the cli progress bars to stay hidden under cargo test.
1. ADDED presets for guyana, paraguay, peru, suriname, uruguay, venezuela, netherlands and switzerland.

## **2026-08-02** - 0.0.3

1. ADDED presets for chile, colombia and ecuador.
1. ADDED a user-agent to the http requests.

## **2026-07-11** - 0.0.2

1. REMOVED the test files from the sonar analysis.
1. MODIFIED the http server to fix the sonar security findings.
1. MODIFIED `Cargo.lock` to fix the audit report.
1. ADDED presets for portugal, argentina and bolivia.
1. ADDED a shutdown signal handler.

## **2026-06-28** - 0.0.1

1. first public version :)
