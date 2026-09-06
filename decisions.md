# decisions

## **2026-09-05**

1. added e2e tests to ensure regression quality.
2. split the cargo release from the docker release to decouple them.
3. the domain refactor is delivered one domain at a time, and each domain gets a patch version.

## **2026-08-02**

1. add more unit tests to cover more than 90% before start refactoring some parts.

## **2026-06-28** (before public release)

1. rewrite from deno/node to rust due limitations on memory control and multi threading.
2. pre-process admin level hierarchies to optimize text search.
3. drop the usage of spatiallite (sqlite extension) to decouple from sqlite.
4. split sqlite database between osm data and data to speed up optimization.
5. drop sqlite fts (trigam or unicode61) in flavor of tantivy.
