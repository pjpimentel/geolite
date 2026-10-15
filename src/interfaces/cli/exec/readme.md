# exec
> `geolite exec <group>-<stage>`: one stage of the pipeline per call, refused before the stages it requires

`stage` names the twelve stages, in the order `build` runs them. it is the value a handler asserts
with: `stage::x.require(&stage_context { catalog, file, bases })` walks `x.requires()` and, for
the first one that did not run, exits 1 with `x requires y — run \`geolite exec y\` first`. the
context is what the stage works on: the catalogue with the osm_data of its source attached, when it
has a source, and the bases it reads or writes. a stage with a source asserts right after resolving
it, so the ledger still records the file; a stage over bases asserts after each base opens. `build`
calls the same handlers, so it carries the same asserts and always passes them.

the three pbf stages and the two extraction stages take a source (`extract-osm-admin-levels
brazil`); the index and optimize stages take the countries whose bases they run over, every base of
the data path when none is named; `optimize-delete-intermediary-data` takes the source whose pbf
and osm_data it deletes.

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
| `optimize-delete-isolated-admin-levels` | admin-levels-hierarchy | — |
| `optimize-merge-admin-levels` | admin-levels-hierarchy | — |
| `index-addresses` | admin-levels-hierarchy | — |
| `index-coordinates` | osm-admin-levels | — |
| `optimize-delete-intermediary-data` | osm-house-numbers, admin-levels-hierarchy | — |
| `optimize-sqlite-file` | — | — |

the evidence is read from the rows a stage leaves, never from a ledger of runs. the house numbers
are the one stage whose legitimate outcome may be no row, so the ledger count stamped by the stage
is the fallback. the ledger counts what a source brought: the two extraction stages stamp it, and
the stages that delete or fold afterwards leave it where it was.
