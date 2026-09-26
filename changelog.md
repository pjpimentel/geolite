# changelog

## **planned** - 0.0.17

1. MODIFIED the storage to a single file, without the split between `database.sqlite3` and `database.tantivy`, so a base is one file to copy, publish and merge.
1. ADDED a repository of prebuilt data per region and schema version, with a `geolite fetch <region>` that downloads instead of building, so a user queries without the build.
1. ADDED a study of a geofabrik proxy that mirrors, caches and versions the extracts, so a build is reproducible and survives the origin being down.
1. ADDED a layer compatible with the google geocoding and places sdk (`/maps/api/geocode/json`, `/maps/api/place/*`), so a client of the sdk changes only the endpoint.
1. ADDED a feasibility study of vector tiles built from the pbf and served at `/tiles/{z}/{x}/{y}.pbf`, so it is decided whether geolite serves the map besides the geocode.

## **planned** - 0.0.16

1. ADDED a tui that walks the admin levels like folders, runs queries and shows the ledger counters, so the data is explored in the terminal.
1. REMOVED the web ui served at `/` by `http-server`, so the server serves only the api.
1. ADDED a places api to `http-server`: search by text and by id, autocomplete, the details and the children of a place, so a client browses places instead of only geocoding.
1. ADDED e2e coverage for the downward reads of the hierarchy beside their first consumer, so they debut covered over real data.
1. ADDED a query cache to `http-server` and `query`, bounded by a cli option, so a repeated question answers at once.

## **planned** - 0.0.15

1. MODIFIED `house_number::resolution` to interpolate along the street and to extrapolate from one reference, so a number lands on its most probable point instead of the centroid.
1. ADDED a data exporter, as a sql dump of the domain tables and as `.osm.pbf`, so the data travels to another database and sub-extracts need no osmium.
1. ADDED `src/lib.rs`, with `main.rs` reduced to `cli::run`, so geolite can be used as a dependency.
1. ADDED a study of removing each direct dependency, measured in own code, transitive crates and binary size, so which ones stay is decided by numbers.
1. MODIFIED the workflows to reuse the binary of the quality check in the docker images and in `cargo publish`, so the binary tested is the binary shipped.

## **planned** - 0.0.14

1. ADDED scenarios that pin the thresholds of the hierarchy resolution and the defaults of `src/admin_level/rules.rs`, so a change of rule cannot pass green.
1. ADDED scenarios for the shapes the real extract never brings (compound and `#` house numbers, `drop_values`, the label and the scale errors), so the battery over real data holds them too.
1. REMOVED the island of unit tests that was left, as the scenarios above take over each guard, so one regression battery is left.
1. MODIFIED the cli to run one stage as `geolite exec <group>-<stage>` in place of the `extract`, `index` and `optimize` subcommands and their aggregate runs, each stage refusing to run before the stages it requires, so a stage is addressed by its name and cannot run out of order.

## **2026-09-20** - 0.0.13

1. MODIFIED the similarity, the house number reads, the preset plumbing and the progress report to the domains that own them.
1. ADDED e2e scenarios for `geolite merge` and for the degraded paths the real pipeline never produces.
1. REMOVED the unit tests the e2e battery now covers: the merge, the http helpers and the wkt parse.
1. MODIFIED the pre-merge checks into a lint job beside the coverage one, so they run in parallel.
1. ADDED `optimize merge-admin-levels`, which folds the ways of one street into one row; `build` and `geolite merge` run it before the search indexes.
1. MODIFIED the resolver to ask a folded street one line at a time, so resolving it again writes the same edges.
1. MODIFIED the coordinate service to answer a street only under the areas that hold its nearest point, and to rank two streets at the same distance by id.
1. MODIFIED the text service to answer a street without a number with a point of the street, inside the area of each label.
1. MODIFIED the `wkt` of a folded street to always be a multi-line of the original lines of its ways.
1. ADDED `merged_way_ids` to `admin_levels` and `osm_merged_way_ids` to the response, naming the osm way of each line of a folded street, with no new schema version.
1. MODIFIED `geolite merge` to carry `merged_way_ids` and to accept a source built before the column.
1. ADDED e2e scenarios for the street merge in the pipeline, the ranking of streets at the same distance and the ways of a folded street in the response.

## **2026-09-19** - 0.0.12

1. MODIFIED the friendly name to one rule with or without a house number, so a label with a number keeps the post codes at the end.
1. MODIFIED the house number to resolve before the match is built, so the api dto carries no pipeline state.
1. MODIFIED `attributes.post_code` to one rule in both services, the most specific post code of the path, so the same street answers the same post code by text and by coordinate.
1. ADDED `post_code` to every item of `admin_levels` in the response, so each area answers its own post code.
1. MODIFIED an `admin_levels` row to carry its origin as one value instead of a pair of optionals, removing the panic on an empty pair.
1. MODIFIED the domain folders to the top level of `src/`.
1. MODIFIED `cli` and `http` into `src/interfaces/`.
1. MODIFIED `bounding_geometry` and the wkt parse to one owner, `admin_level::geometry`.
1. MODIFIED the crates.io publish to use Trusted Publishing on the `crates.io` environment, so no long-lived registry token is stored.

## **2026-09-17** - 0.0.11

1. REMOVED the unit tests that the e2e battery already covers.
1. REMOVED the columns of `admin_levels_hierarchy` that the edges already derive.
1. MODIFIED `admin_levels_hierarchy` to one edge per row, with every parent that contains an area.
1. MODIFIED `query`, `/geocode` and the search index to answer one match per path.
1. MODIFIED `matches[].id` to the uuid v5 of the path: an id stored from 0.0.10 no longer matches.
1. MODIFIED the search index to compare the score at three decimals, so two documents that tie rank by area id on every build.
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
