# decisions

## **2026-06-28** (before public release)

1. rewrite from deno/node to rust due limitations on memory control and multi threading.
2. pre-process admin level hierarchies to optimize text search.
3. drop the usage of spatiallite (sqlite extension) to decouple from sqlite.
4. split sqlite database between osm data and data to speed up optimization.
5. drop sqlite fts (trigam or unicode61) in flavor of tantivy.
