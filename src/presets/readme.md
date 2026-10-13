# presets
> what varies by country: the levels an address names, the name tag, the house-number policy, the abbreviations, and the boundary that says which country a row is in

```
mod.rs            the presets, one const per country and `DEFAULT`, and the lookups `for_country` and `name_tags`
boundaries/       the boundary of each country, one `<iso>.wkt` compiled into the binary by `include_str!`
```

a preset is a country: its `country` carries the iso 3166-1 alpha-2 code and the wkt of its
boundary, and `COUNTRIES` lists them. a row is extracted, indexed and answered under the preset of
the country whose boundary holds it (`admin_level::country_index`), never under a flag and never
under what the extract happens to hold: a sub-region extract (`sudeste`) carries no closed boundary
of brazil, and a country relation clipped by the extract never closes into a polygon, so the
boundary has to come with the binary. `DEFAULT` has no country and answers for what falls in no
boundary.

## the boundaries — `boundaries/`

the outlines come from [natural earth](https://www.naturalearthdata.com) 1:10m cultural vectors,
admin 0 countries, public domain (`ne_10m_admin_0_countries.geojson`, the `master` branch of
`nvkelso/natural-earth-vector` on 2026-10-12), one feature per `ADM0_A3`. each ring is simplified
by douglas-peucker at 0.01° (about a kilometre), rings left with fewer than four points are
dropped, and the result is written as one `MULTIPOLYGON` with four decimals, `lon lat`. the fifteen
files weigh about 340 kb together. the 1:50m set is a quarter of that, but its coast runs outside
the santos estuary and leaves six hundred streets of the fixture in the sea; at 1:10m only a
submarine cable is. an outline still leaves a pier or a harbour outside the polygon, which is why
the country index reaches 0.05° past the boundary.

to regenerate, run a script over the geojson that filters the fifteen codes, simplifies and writes
the files; the one used lives outside the repository and takes the geojson, the output directory
and the tolerance. a newer natural earth release moves coastlines by metres: rebuild the e2e world
after it.
