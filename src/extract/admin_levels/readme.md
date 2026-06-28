# admin levels

## default

### level 1
- INCLUDE relations with tag admin_level = 1 FROM osm_relations BECAUSE it is the simplest solution.
- EXCLUDE relations without name tag FROM osm_relations BECAUSE it can not produce useful data.

### level 2
- INCLUDE relations with tag admin_level = 2 FROM osm_relations BECAUSE it is the simplest solution.
- EXCLUDE relations without name tag FROM osm_relations BECAUSE it can not produce useful data.

### level 3
- INCLUDE relations with tag admin_level = 3 FROM osm_relations BECAUSE it is the simplest solution.
- EXCLUDE relations without name tag FROM osm_relations BECAUSE it can not produce useful data.

### level 4
- INCLUDE relations with tag admin_level = 4 FROM osm_relations BECAUSE it is the simplest solution.
- EXCLUDE relations without name tag FROM osm_relations BECAUSE it can not produce useful data.

### level 5
- INCLUDE relations with tag admin_level = 5 FROM osm_relations BECAUSE it is the simplest solution.
- EXCLUDE relations without name tag FROM osm_relations BECAUSE it can not produce useful data.

### level 6
- INCLUDE relations with tag admin_level = 6 FROM osm_relations BECAUSE it is the simplest solution.
- EXCLUDE relations without name tag FROM osm_relations BECAUSE it can not produce useful data.

### level 7
- INCLUDE relations with tag admin_level = 7 FROM osm_relations BECAUSE it is the simplest solution.
- EXCLUDE relations without name tag FROM osm_relations BECAUSE it can not produce useful data.

### level 8
- INCLUDE relations with tag admin_level = 8 FROM osm_relations BECAUSE it is the simplest solution.
- EXCLUDE relations without name tag FROM osm_relations BECAUSE it can not produce useful data.

### level 9
- INCLUDE relations with tag admin_level = 9 FROM osm_relations BECAUSE it is the simplest solution.
- EXCLUDE relations without name tag FROM osm_relations BECAUSE it can not produce useful data.

### level 10
- INCLUDE ways with tag place = neighbourhood FROM osm_ways BECAUSE it is the simplest solution.
- INCLUDE ways with tag place = suburb FROM osm_ways BECAUSE in some regions suburbs are the de-facto neighbourhood unit when place=neighbourhood is not mapped.
- EXCLUDE ways without name tag FROM osm_ways BECAUSE it can not produce useful data.

### level 12
- INCLUDE all ways FROM osm_ways BECAUSE it is the simplest solution.
- EXCLUDE ways without name tag FROM osm_ways BECAUSE it can not produce useful data.
- EXCLUDE ways with tag place = neighbourhood FROM osm_ways BECAUSE they were already captured at level 10.
- EXCLUDE ways with tag place = suburb FROM osm_ways BECAUSE they were already captured at level 10.
- EXCLUDE ways with tag leisure = park FROM osm_ways BECAUSE it produce noise in street-level data.
- EXCLUDE ways with tag building FROM osm_ways BECAUSE it produce noise in street-level data.
- EXCLUDE ways with tag waterway FROM osm_ways BECAUSE it produce noise in street-level data.
