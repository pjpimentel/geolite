# domain

> the model itself, free of i/o

nothing under `domain` imports rusqlite, geozero, tantivy, ureq, tiny_http, clap or prost. the only
external crate allowed here is `geo`, which is a geometry model rather than an i/o concern. the
adapters around this layer do the talking to sqlite, to the full-text index and to the network.

```
kernel/         concepts more than one context needs with the same meaning
  admin_area_id     stable identity of an admin area, packed from the osm way/relation it came from
house_number/   what a door number is, on both sides of the pipeline
  value             the value object: normalize (ingestion) / recognize (query) / compare
  policy            per-region rules: tags, non-values, digit cap, written forms, `#` prefix
  token             finding the number inside a free-text query
  resolution        placing it on a street: exact | interpolated | absent
  link              a number attached to a street, and how it got there
```

## house_number

the concept used to live in four places with **two different rules in two languages**: normalisation
in sql (the extraction query), the street link as a bare `u8`, token recognition in rust (the query
path) and resolution beside it. the two rules disagreed, and the disagreement was a bug: the
ingestion stored `addr:housenumber=82-52` intact while the query rejected that token, kept only the
`52`, matched nothing and fell into a meaningless interpolation.

### two forms, one number

| | what it is | who reads it |
|---|---|---|
| `stored_form` | what gets persisted or displayed | the writer, the api response |
| `comparison_key` | aggressively canonical: no `#`, upper case, `-` between compound parts | equality, and only equality |

the split is what let the colombian nomenclature be fixed **without a rebuild**. the comparison
between a typed number and the stored ones already happened in rust, never in sql, so the key can be
computed in memory on both sides while the bytes on disk stay exactly as they were written.

```
written "16i56"  → stored_form "16i56"  → key "16I-56"
typed   "16I56"  →                        key "16I-56"   → same number
```

### written forms are a regional policy

`82-52` is a compound number in colombia and a range in brazil. nothing in the string tells them
apart — only the region does, so recognition is opt-in per preset:

| form | example | enabled by |
|---|---|---|
| `simple` | `123` | every preset |
| `suffixed` | `123A` (from `12 a`, `12-a`, `12a`) | every preset |
| `compound` | `82-52`, `25B-48`, `16i56` | `COMPOUND_SHAPES` — colombia |
| `#` prefix | `#82`, `# 52-48` | `allow_hash_prefix` — colombia |

a compound split across several tokens (`82 - 52`) is not recognised; only a lone `#` gets a
one-token lookahead. a hyphenated pair whose first part carries a leading zero is never compound,
which is what keeps a postcode (`01310-100`) out.
