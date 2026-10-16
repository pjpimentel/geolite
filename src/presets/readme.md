# presets
> what varies by country: the levels an address names, the name tag, the house-number policy and the abbreviations; and the boundary of every country, which says which country a row is in

```
mod.rs            the presets, one const per country that has one and `DEFAULT`, and the lookups `for_country`, `iso_key` and `name_tags`
boundaries/       the boundary of every country of the geofabrik index, one `<iso>.wkt` compiled into the binary by `include_str!`, and `mod.rs`, the generated list of them
```

a row is extracted, indexed and answered under the country whose boundary holds it
(`admin_level::country_index`), never under a flag and never under what the extract happens to
hold: a sub-region extract (`sudeste`) carries no closed boundary of brazil, and a country relation
clipped by the extract never closes into a polygon, so the boundary has to come with the binary.
`boundaries::ALL` lists every country, and `iso_key` answers the code a base is named by. a preset
is the policy of one of them, named by its `iso_3166_1_alpha_2`, and `COUNTRY_PRESETS` lists the
countries that have one; `for_country` answers the preset of a country, `DEFAULT` for every other.

## the boundaries — `boundaries/`

the outline of each country is the one of its [geofabrik](https://download.geofabrik.de) extract:
the `geometry` of the region of the index (`https://download.geofabrik.de/index-v1.json`) whose
`iso3166-1:alpha2` names that country alone, written as one `MULTIPOLYGON`, `lon lat`, with the
coordinates as the index gives them. the 181 files weigh about 730 kb together. the outline is
the polygon the extract is clipped by, so every row of `brazil-latest.osm.pbf` falls inside
`br.wkt` by construction; it runs a few kilometres into the sea and into each neighbour, which is
why a point inside two outlines goes to the one it is deeper inside (`admin_level::country_index`).

two things of the index are corrected on the way: the index gives `VU` to five pacific regions
besides vanuatu and `MH` to the pitcairn islands besides the marshall islands, and only the
country's own region is kept; and a region of several countries (`gcc-states`, `haiti-and-domrep`,
`israel-and-palestine`, `senegal-and-gambia`) cannot be split, so those eleven countries have no
boundary and a row of theirs goes to the nearest country that has one.

to regenerate, download the index and run a script that picks one region per code, writes the
files and `mod.rs`; the one used lives outside the repository and takes the index and the output
directory. geofabrik redraws an outline now and then: rebuild the e2e world after it.
