# changelog

## **2026-XX-XX** - 0.0.7

1. REMOVED `extract/osm_data` mod.
1. ADDED `osm_node`, `osm_way` and `osm_relation` domains with their decoders.
1. ADDED `osm_tag` domain, absorbing `build_name_select` and the tag key validation of the cli.
1. ADDED `origin_wkt` to `osm_pbf_files`, the coverage polygon of every geofabrik region, cached by `ls`; databases built by 0.0.6 gain the column on open.
1. MODIFIED `extract osm-pbf-data`: `--include-nodes`, `--include-ways` and `--include-relations` take an explicit value, and `--recreate` keeps the blob chunk index.
1. MODIFIED `osm_pbf_file` domain, adding `extract_blob_chunks`, `extract_osm_header` and `extract_osm_data` to its facade.
1. MODIFIED `database` mod, adding the `jsonb` encoder moved from `extract/osm_data`.

## **2026-09-06** - 0.0.6

1. REMOVED unit tests redundant with e2e tests.
1. MODIFIED release pipeline, splitting `try_publish` into `crates_publish` and `docker_publish`.
1. MODIFIED `Dockerfile` to build from `cargo install` instead of the local source tree.
1. REMOVED `osm_pbf_file` mod.
1. ADDED `osm_pbf_file` domain.
1. MODIFIED `osm_pbf_files` structure (not backward compatible)
1. MODIFIED `build` to accept a direct url as source; it was refused as a missing local file.

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