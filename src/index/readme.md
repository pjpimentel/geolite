# index
> build lookup structures over extracted admin_levels data

the two stages that come before this one — the containment hierarchy and the bounding-box rtree —
belong to the areas themselves and live in [`domain/admin_level_hierarchy`](../domain/readme.md#admin_level_hierarchy)
and [`domain/admin_level`](../domain/readme.md#admin_level).

## user_friendly_name

1. builds (or rebuilds) the tantivy full-text index (`admin_levels_hierarchy_tantivy`) at the index path (e.g. `database.tantivy`), one document per `admin_levels_hierarchy` row
2. indexes each row's own name (plus post code, both original and digits-only) and its concatenated ancestor names (`hier`), each in folded / strict / lower field variants used for ranking
3. expands abbreviations bidirectionally (e.g. `rua` ↔ `r.`) so a query matches either form
