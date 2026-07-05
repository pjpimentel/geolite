# geolite <a href="https://www.buymeacoffee.com/pjpimentel"><img align=right width=150 src="https://img.buymeacoffee.com/button-api/?text=buy%20me%20a%20coffee&emoji=&slug=pjpimentel&button_colour=5F7FFF&font_colour=ffffff&font_family=Inter&outline_colour=000000&coffee_colour=FFDD01" /></a>
##### _this is a *work in progress* and, for now, new patch versions have no backward-compatibility guarantees_
## open source geocode

[![actions](https://github.com/pjpimentel/geolite/actions/workflows/11_ci_master.yml/badge.svg?branch=master)](https://github.com/pjpimentel/geolite/actions/workflows/11_ci_master.yml)
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
$ docker run pjpimentel/geolite:0.0.1 -- --version
# pre-built countries geocodes
$ docker run -p 8080:8080 pjpimentel/geolite:prebuilt-brazil
# open http://localhost:8080
```

---

## docs

1. source code modules
    1. [database](src/database/readme.md)
    1. [osm-pbf-file](src/osm_pbf_file/readme.md)
    1. [extract](src/extract/readme.md)
    1. [index](src/index/readme.md)
    1. [optimize](src/optimize/readme.md)
    1. [query](src/query/readme.md)
    1. [cli](src/cli/)
    1. [http](src/http/readme.md)
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
    1. [merge separately-built databases](#merge-separately-built-databases)

---

## docker images

### base image

```bash
$ docker run pjpimentel/geolite:0.0.1 -- --version
$ docker run -t -v ./data:/data pjpimentel/geolite:0.0.1 build brazil
```

### pre-built images

_size matters: once a country's data grows too large for a docker image, its pre-built image stops being updated and may be dropped._

```bash
docker run -p 8080:8080 pjpimentel/geolite:prebuilt-brazil
docker run -p 8080:8080 pjpimentel/geolite:prebuilt-brazil-YYYYMMDDHHMM
# open http://localhost:8080
```

#### available country list

| # | country | docker cmd |
|---|---------|------------|
| 1 | [brazil](https://hub.docker.com/layers/pjpimentel/geolite/prebuilt-brazil) | `docker run -p 8080:8080 pjpimentel/geolite:prebuilt-brazil` |
| 2 | [portugal](https://hub.docker.com/layers/pjpimentel/geolite/prebuilt-portugal) | `docker run -p 8080:8080 pjpimentel/geolite:prebuilt-portugal` |
| 3 | [argentina](https://hub.docker.com/layers/pjpimentel/geolite/prebuilt-argentina) | `docker run -p 8080:8080 pjpimentel/geolite:prebuilt-argentina` |
| 4 | [bolivia](https://hub.docker.com/layers/pjpimentel/geolite/prebuilt-bolivia) | `docker run -p 8080:8080 pjpimentel/geolite:prebuilt-bolivia` |
| 5 | [chile](https://hub.docker.com/layers/pjpimentel/geolite/prebuilt-chile) | `docker run -p 8080:8080 pjpimentel/geolite:prebuilt-chile` |
| 6 | [colombia](https://hub.docker.com/layers/pjpimentel/geolite/prebuilt-colombia) | `docker run -p 8080:8080 pjpimentel/geolite:prebuilt-colombia` |
| 7 | [ecuador](https://hub.docker.com/layers/pjpimentel/geolite/prebuilt-ecuador) | `docker run -p 8080:8080 pjpimentel/geolite:prebuilt-ecuador` |
| 8 | [guyana](https://hub.docker.com/layers/pjpimentel/geolite/prebuilt-guyana) | `docker run -p 8080:8080 pjpimentel/geolite:prebuilt-guyana` |
| 9 | [paraguay](https://hub.docker.com/layers/pjpimentel/geolite/prebuilt-paraguay) | `docker run -p 8080:8080 pjpimentel/geolite:prebuilt-paraguay` |
| 10 | [peru](https://hub.docker.com/layers/pjpimentel/geolite/prebuilt-peru) | `docker run -p 8080:8080 pjpimentel/geolite:prebuilt-peru` |
| 11 | [suriname](https://hub.docker.com/layers/pjpimentel/geolite/prebuilt-suriname) | `docker run -p 8080:8080 pjpimentel/geolite:prebuilt-suriname` |
| 12 | [uruguay](https://hub.docker.com/layers/pjpimentel/geolite/prebuilt-uruguay) | `docker run -p 8080:8080 pjpimentel/geolite:prebuilt-uruguay` |
| 13 | [venezuela](https://hub.docker.com/layers/pjpimentel/geolite/prebuilt-venezuela) | `docker run -p 8080:8080 pjpimentel/geolite:prebuilt-venezuela` |
| 14 | [netherlands](https://hub.docker.com/layers/pjpimentel/geolite/prebuilt-netherlands) | `docker run -p 8080:8080 pjpimentel/geolite:prebuilt-netherlands` |
| 15 | [switzerland](https://hub.docker.com/layers/pjpimentel/geolite/prebuilt-switzerland) | `docker run -p 8080:8080 pjpimentel/geolite:prebuilt-switzerland` |

---

## cookbook

### execute all necessary steps
```bash
$ geolite build brazil
# or with docker
$ docker run --rm -t -v ./geolite-data:/.geolite pjpimentel/geolite:latest build brazil
$ docker run --rm -p 8080:8080 -v ./geolite-data:/.geolite pjpimentel/geolite:latest http-server
```

### extract each data from a pbf file
```bash
$ geolite osm-pbf-file download brazil
$ geolite extract osm-pbf-blob-chunks brazil
$ geolite extract osm-pbf-data brazil
$ geolite extract osm-admin-levels
$ geolite extract osm-house-numbers
```

### index extracted data
```bash
$ geolite index admin-levels-hierarchy
$ geolite index user-friendly-name
$ geolite index coordinates
```

### optimize data
```bash
$ geolite optimize delete-intermediary-data
$ geolite optimize sqlite-file
```

### query
```bash
$ geolite query "-23.970949,-46.318730"
$ geolite query "rua castro alves, embare, santos, sao paulo"
```

### explore
```bash
$ geolite http-server
```

### merge separately-built databases
```bash
# machine 1
$ geolite --sqlite-path sudeste.sqlite3 build sudeste
# machine 2
$ geolite --sqlite-path nordeste.sqlite3 build nordeste

# merge all builds into a new combined database (created fresh)
$ geolite merge brazil.sqlite3 sudeste.sqlite3 nordeste.sqlite3 ...

# or with docker 
$ docker run --rm -t -v ./geolite-data:/.geolite pjpimentel/geolite:latest --sqlite-path /.geolite/sudeste.sqlite3 build sudeste
$ docker run --rm -t -v ./geolite-data:/.geolite pjpimentel/geolite:latest --sqlite-path /.geolite/nordeste.sqlite3 build nordeste
$ docker run --rm -t -v ./geolite-data:/.geolite pjpimentel/geolite:latest --sqlite-path /.geolite/brazil.sqlite3 merge /.geolite/brazil.sqlite3 /.geolite/sudeste.sqlite3 /.geolite/nordeste.sqlite3
$ docker run --rm -p 8080:8080 -v ./geolite-data:/.geolite pjpimentel/geolite:latest --sqlite-path /.geolite/brazil.sqlite3 http-server
```


## license: [GNU AGPLv3](LICENSE)
