# index
> build lookup structures over extracted admin_levels data

## coordinates

1. drops and recreates `admin_levels_rtree`
2. loads all `admin_levels` rows that have geometry, in batches
3. computes the bounding box of each geometry and inserts into the rtree

## hierarchy

1. loads all non-street admin boundaries into memory with their polygon rings and centroids
2. builds an in-memory rtree for fast spatial candidate lookup
3. for each boundary (level ASC), finds the smallest enclosing parent via point-in-polygon
4. writes ancestor id chain and user-friendly name (`"street, city, country"`) to `admin_levels_hierarchy`
5. processes streets in batches against the same in-memory tree

## user_friendly_name

1. builds (or rebuilds) the FTS5 index over `admin_levels_hierarchy.user_friendly_name`
