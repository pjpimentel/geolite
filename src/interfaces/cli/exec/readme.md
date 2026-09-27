# exec
> `geolite exec <group>-<stage>`: one stage of the pipeline per call, refused before the stages it requires

`stage` names the eleven stages, in the order `build` runs them. it is the value a handler asserts
with: `stage::x.require(&conn, file)` walks `x.requires()` and, for the first one that did not run,
exits 1 with `x requires y — run \`geolite exec y\` first`. a per-file stage asserts right after
resolving each input, so the ledger still records the file; a global stage asserts right after
the connection opens. `build` and `geolite merge` call the same handlers, so they carry the same
asserts and always pass them.

`exec_commands` is the clap surface: one variant per stage, named by `stage::name()`, carrying the
arguments of the stage. two enums because clap needs the arguments on the variant and the assert
needs a value without them.

| stage | requires | ran when |
|---|---|---|
| `extract-osm-pbf-blob-chunks` | — | the file has rows in `osm_pbf_blob_chunks` |
| `extract-osm-pbf-header` | blob-chunks of the file | — |
| `extract-osm-pbf-data` | blob-chunks of the file | a ledger row carries `osm_data_extracted_at` |
| `extract-osm-admin-levels` | osm-pbf-data | an `admin_levels` row with geometry exists |
| `extract-osm-house-numbers` | osm-admin-levels | a `house_numbers` row exists, or a ledger row carries `house_numbers_count` |
| `index-admin-levels-hierarchy` | osm-admin-levels | edges exist and no area is pending |
| `optimize-merge-admin-levels` | admin-levels-hierarchy | — |
| `index-user-friendly-name` | admin-levels-hierarchy | — |
| `index-coordinates` | osm-admin-levels | — |
| `optimize-delete-intermediary-data` | osm-house-numbers, admin-levels-hierarchy | — |
| `optimize-sqlite-file` | — | — |

the evidence is read from the rows a stage leaves, never from a ledger of runs: `geolite merge`
does not copy `osm_pbf_files`, so a merged base answers the same asserts through its rows. the
house numbers are the one stage whose legitimate outcome may be no row, so the ledger count stamped
by the stage is the fallback.
