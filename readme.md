# geolite <a href="https://www.buymeacoffee.com/pjpimentel"><img align=right width=150 src="https://img.buymeacoffee.com/button-api/?text=buy%20me%20a%20coffee&emoji=&slug=pjpimentel&button_colour=5F7FFF&font_colour=ffffff&font_family=Inter&outline_colour=000000&coffee_colour=FFDD01" /></a>
##### _this is a *work in progress* and, for now, new patch versions have no backward-compatibility guarantees_
## open source geocode

[![actions](https://github.com/pjpimentel/geolite/actions/workflows/11_ci_master.yml/badge.svg?branch=master)](https://github.com/pjpimentel/geolite/actions/workflows/11_ci_master.yml)
[![quality](https://sonarcloud.io/api/project_badges/measure?branch=master&project=geolite&metric=alert_status)](https://sonarcloud.io/dashboard?branch=master&id=geolite)
[![coverage](https://sonarcloud.io/api/project_badges/measure?branch=master&project=geolite&metric=coverage)](https://sonarcloud.io/dashboard?branch=master&id=geolite)
[![security](https://sonarcloud.io/api/project_badges/measure?branch=master&project=geolite&metric=security_rating)](https://sonarcloud.io/dashboard?branch=master&id=geolite)

---

## getting started

### with docker

```bash
# command line
$ docker run pjpimentel/geolite:0.0.1 -- --version
# pre-built countries geocodes
$ docker run -p 8080:8080 pjpimentel/geolite:prebuilt-brazil
# open http://localhost:8080
```

### with cargo

```bash
$ cargo install geolite
$ geolite --version
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
    1. [cli](src/cli/readme.md)
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
    1. [attach distinct databases](#attach-distinct-databases)

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

1. [brazil](https://hub.docker.com/layers/pjpimentel/geolite/prebuilt-brazil)
1. [portugal](https://hub.docker.com/layers/pjpimentel/geolite/prebuilt-portugal)
<!-- 1. [argentina](https://hub.docker.com/layers/pjpimentel/geolite/prebuilt-argentina)
1. [chile](https://hub.docker.com/layers/pjpimentel/geolite/prebuilt-chile)
1. [uruguay](https://hub.docker.com/layers/pjpimentel/geolite/prebuilt-uruguay)
1. [paraguay](https://hub.docker.com/layers/pjpimentel/geolite/prebuilt-paraguay)
1. [bolivia](https://hub.docker.com/layers/pjpimentel/geolite/prebuilt-bolivia)
1. [peru](https://hub.docker.com/layers/pjpimentel/geolite/prebuilt-peru)
1. [ecuador](https://hub.docker.com/layers/pjpimentel/geolite/prebuilt-ecuador)
1. [colombia](https://hub.docker.com/layers/pjpimentel/geolite/prebuilt-colombia)
1. [venezuela](https://hub.docker.com/layers/pjpimentel/geolite/prebuilt-venezuela)
1. [guyana](https://hub.docker.com/layers/pjpimentel/geolite/prebuilt-guyana)
1. [suriname](https://hub.docker.com/layers/pjpimentel/geolite/prebuilt-suriname)
1. [french guiana](https://hub.docker.com/layers/pjpimentel/geolite/prebuilt-french-guiana) -->


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
