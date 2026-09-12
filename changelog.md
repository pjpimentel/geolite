# changelog

## **2026-XX-XX** - 0.0.9

1. REMOVED `database/house_numbers` mod.
1. REMOVED `extract` mod.
1. REMOVED `optimize` mod.
1. MODIFIED presets: `extract_house_numbers` became `house_numbers`, a `house_number_policy` (`number_tags`, `street_tags`, `drop_values`, `max_digits`, `shapes`, `allow_hash_prefix`).
1. MODIFIED `extract osm-house-numbers`: the tag value is normalised in the domain (`house_number::normalize`) instead of in sql; the stored forms do not change.
1. MODIFIED `extract osm-house-numbers`: the count it prints is the number of rows inserted; it was the number of candidates, so a rerun reported every candidate again instead of 0.
1. MODIFIED `query` and `http-server` under the `colombia` preset: a compound number (`82-52`, `25B-48`, `16i56`) and the `#` prefix are read from the input, match their stored value exactly and are never interpolated; no rebuild required.
1. ADDED `house_number` domain (`entity`, `value`, `policy`, `strategy`, `token`, `resolution`, `repository`, `extract`, `linker`).
1. ADDED `house_number_link::extract`: the house-number stage as one use case of the domain.
1. ADDED `osm_pbf_file::delete`: the file, its blob chunks and its download columns (`path`, `size_bytes`, `md5`, `downloaded_at`) go together.
1. MODIFIED `optimize delete-intermediary-data`: the ledger row of every deleted file forgets its path, so nothing resolves to a file that is gone; the pbf files are reported before the `osm_data` sibling.

## **2026-09-12** - 0.0.8

1. REMOVED `database/admin_levels` mod.
1. REMOVED `extract/admin_levels` mod.
1. REMOVED `database/admin_levels_hierarchy` mod.
1. REMOVED `index` mod (`coordinates`, `hierarchy`, `user_friendly_name`, `admin_levels_hierarchy_tantivy`).
1. MODIFIED `extract osm-admin-levels`: `--admin-level` refuses a level outside the scale, a value that is not a number and an empty list instead of dropping them; the list is checked before `--recreate` touches the database.
1. MODIFIED `query` and `http-server`: `--last-admin-levels` and `?last_admin_levels=` refuse a level outside the scale (the http server answers 400 where it answered an empty list); `--last-admin-levels` tolerates spaces around the values.
1. MODIFIED `admin_levels` reads: a row whose level is outside the scale is skipped with a warning instead of being returned; no release ever writes one.
1. ADDED `admin_level` domain (`entity`, `scale`, `id`, `geometry`, `repository`, `spatial_index`, `rules`, `extract`, `relations`, `place_ways`, `streets`).
1. ADDED `admin_level::extract(level)`: the three extraction stages as one use case of the domain.
1. ADDED `admin_level_hierarchy` domain (`entity`, `label`, `repository`, `resolver`, `search_index`).

## **2026-09-07** - 0.0.7

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