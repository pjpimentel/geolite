# geolite <a href="https://www.buymeacoffee.com/pjpimentel"><img align=right width=150 src="https://img.buymeacoffee.com/button-api/?text=buy%20me%20a%20coffee&emoji=&slug=pjpimentel&button_colour=5F7FFF&font_colour=ffffff&font_family=Inter&outline_colour=000000&coffee_colour=FFDD01" /></a>
##### _making openstreetmap data usable by anyone: from `.osm.pbf` to the first query answer._
## open source geocode

[![actions](https://github.com/pjpimentel/geolite/actions/workflows/11_crates_publish.yml/badge.svg?branch=master)](https://github.com/pjpimentel/geolite/actions/workflows/11_crates_publish.yml)
[![quality](https://sonarcloud.io/api/project_badges/measure?branch=master&project=geolite&metric=alert_status)](https://sonarcloud.io/dashboard?branch=master&id=geolite)
[![coverage](https://sonarcloud.io/api/project_badges/measure?branch=master&project=geolite&metric=coverage)](https://sonarcloud.io/dashboard?branch=master&id=geolite)
[![crates downloads](https://img.shields.io/crates/d/geolite)](https://crates.io/crates/geolite)

---

## getting started

### with cargo

```bash
$ cargo install geolite
$ geolite --version
$ geolite build brazil # needs +/- 45gb while building then +/- 3gb when finished
$ geolite http-server # open http://localhost:8080
```

### with docker

```bash
# command line
$ docker run pjpimentel/geolite:latest --version
# pre-built geocoder of a continent
$ docker run -p 8080:8080 pjpimentel/geolite:prebuilt-south-america
# open http://localhost:8080
```

---

## docs

1. [source code modules](src/readme.md)
    1. [database](src/database/readme.md)
    1. [osm-pbf-file](src/osm_pbf_file/readme.md)
    1. [osm-tag](src/osm_tag/readme.md)
    1. [osm-node](src/osm_node/readme.md)
    1. [osm-way](src/osm_way/readme.md)
    1. [osm-relation](src/osm_relation/readme.md)
    1. [admin-level](src/admin_level/readme.md)
    1. [admin-level-hierarchy](src/admin_level_hierarchy/readme.md)
    1. [house-number](src/house_number/readme.md)
    1. [address](src/address/readme.md)
    1. [cli](src/interfaces/cli/)
    1. [http](src/interfaces/http/readme.md)
1. [docker images](#docker-images)
    1. [base image](#base-image)
    1. [pre-built images](#pre-built-images)
1. [cookbook](#cookbook)
    1. [execute all necessary steps](#execute-all-necessary-steps)
    1. [extract each data from a pbf file](#extract-each-data-from-a-pbf-file)
    1. [index extracted data](#index-extracted-data)
    1. [optimize data](#optimize-data)
    1. [query](#query)
    1. [explore](#explore)
    1. [run the end-to-end suite](#run-the-end-to-end-suite)

---

## docker images

### base image

```bash
$ docker run pjpimentel/geolite:latest --version
$ docker run -t -v ./geolite-data:/.geolite pjpimentel/geolite:latest build brazil
```

### pre-built images

_size matters: once a region's data grows too large for a docker image, its pre-built image stops being updated and may be dropped._

```bash
docker run -p 8080:8080 pjpimentel/geolite:prebuilt-south-america
docker run -p 8080:8080 pjpimentel/geolite:prebuilt-south-america-YYYYMMDDHHMM
# open http://localhost:8080
```

#### available images

the images are rebuilt on the 1st and the 15th of every month from the latest geofabrik extracts;
the dated tag keeps each build.

| # | region | docker cmd |
|---|--------|------------|
| 1 | [south-america](https://hub.docker.com/layers/pjpimentel/geolite/prebuilt-south-america), one base per country, thirteen in all | `docker run -p 8080:8080 pjpimentel/geolite:prebuilt-south-america` |
| 2 | [portugal](https://hub.docker.com/layers/pjpimentel/geolite/prebuilt-portugal) | `docker run -p 8080:8080 pjpimentel/geolite:prebuilt-portugal` |
| 3 | [netherlands](https://hub.docker.com/layers/pjpimentel/geolite/prebuilt-netherlands) | `docker run -p 8080:8080 pjpimentel/geolite:prebuilt-netherlands` |
| 4 | [switzerland](https://hub.docker.com/layers/pjpimentel/geolite/prebuilt-switzerland) | `docker run -p 8080:8080 pjpimentel/geolite:prebuilt-switzerland` |

---

## cookbook

### execute all necessary steps
```bash
$ geolite build brazil          # br/data.sqlite3 and br/index.tantivy, sealed
$ geolite build south-america   # one base per country, thirteen in all
# or with docker
$ docker run --rm -t -v ./geolite-data:/.geolite pjpimentel/geolite:latest build brazil
$ docker run --rm -p 8080:8080 -v ./geolite-data:/.geolite pjpimentel/geolite:latest http-server
```

### extract each data from a pbf file
```bash
$ geolite osm-pbf-file download brazil
$ geolite exec extract-osm-pbf-blob-chunks brazil
$ geolite exec extract-osm-pbf-header brazil
$ geolite exec extract-osm-pbf-data brazil
# the two extraction stages read everything the osm data holds, every pbf decoded so far
$ geolite exec extract-osm-admin-levels
$ geolite exec extract-osm-house-numbers
```

### index extracted data
```bash
# every base of the data path, or the ones named: `geolite exec index-addresses br`
$ geolite exec index-admin-levels-hierarchy
$ geolite exec index-addresses
$ geolite exec index-coordinates
```

### optimize data
```bash
# deletes a country without children and a street without parents, then folds the ways of one
# street into one row: both read the hierarchy and clear the addresses and coordinates indexes,
# which have to be built again after them
$ geolite exec optimize-delete-isolated-admin-levels
$ geolite exec optimize-merge-admin-levels
$ geolite exec optimize-delete-intermediary-data
$ geolite exec optimize-sqlite-file
```

`geolite exec` runs one stage per call and refuses a stage whose requirements did not run, naming
the one to run first; `geolite exec --help` lists the stages in the order the pipeline runs. a stage
that reads a pbf names it (`extract-osm-pbf-data brazil`); the two extraction stages read everything
the osm data holds; a stage that works on the bases runs over every base of the data path, or over
the countries named (`index-addresses br ar`).
`geolite build` runs `optimize-delete-isolated-admin-levels`, which removes a base it leaves empty,
and `optimize-merge-admin-levels` right after `index-admin-levels-hierarchy`, so the two other
indexes are built once, over the rows that stay. no index stage deletes or folds on its own.

### query
```bash
$ geolite query "-23.970949,-46.318730"
$ geolite query "rua castro alves, embare, santos, sao paulo"
$ geolite query "rua castro alves, embare, santos, sao paulo" --countries br   # the bases named; every base of the data path otherwise
```

### explore
```bash
$ geolite http-server   # open http://localhost:8080
$ geolite tui           # the admin levels walked like folders, one per country at the root
```

### run the end-to-end suite
```bash
$ cargo test --profile e2e --test '*' -- --ignored --nocapture
```

## question and answers

### a) why geolite?

"**geo**" comes from "**geo**code" and "**lite**" comes from "sq**lite**", no special meaning.

### b) where does the data come from?

the data comes from any `.osm.pbf` file (openstreetmap's protobuffer format). this project uses [geofabrik](https://download.geofabrik.de) as the initial source, which extracts its data from [openstreetmap](https://www.openstreetmap.org).

### c) which license applies?

the source code is under [agplv3](LICENSE) and its dependencies keep their own (all permissive: mit, apache-2.0, bsd, zlib, isc). databases built from openstreetmap data — including the ones shipped in the `prebuilt-*` images — are derivative databases under the [odbl](https://opendatacommons.org/licenses/odbl/), filtered and restructured for geocoding: © [openstreetmap contributors](https://www.openstreetmap.org/copyright).

### d) why build another geocoder?

two reasons, in this order: to practice rust on a problem large enough to be interesting, and because the alternatives i tried were a bit too complex for a quick run.

## license: [GNU AGPLv3](LICENSE)

---

##### _this is a *work in progress* and, for now, new patch versions have no backward-compatibility guarantees_
