# interfaces
> the ways into the domain: each one reads its input, opens the connection, calls the domain and renders the answer

```
cli/    the `geolite` command: parses the arguments, resolves the paths, and runs one subcommand
http/   the http server that `geolite http-server` starts: `/geocode`, `/status`, `/openapi.json`, `/docs`
tui/    the tree of admin levels that `geolite tui` opens: walked like folders down to the address and the map of a leaf, or printed when stdout is not a terminal
```

an interface owns no rule. what a query means, how a region is parsed or how a label is built
lives in the concept folders, so the cli and the http server answer the same question the same
way, and an interface added later reuses the domain instead of repeating it.

the cli is the entry point: `main` calls `cli::run`, which resolves the data path once and dispatches;
`http-server` is a subcommand that binds the port and hands over to `http::serve`. any other
interface is launched the same way, as a subcommand of the cli: `tui` opens the tree in the
terminal through [`tui`](tui/readme.md). no interface carries a preset: the preset of a row is the
one of its country, decided when the row is extracted and read back from the row by the index and
the query, so a base of many countries answers each one under its own.

`exec <group>-<stage>` runs one stage of the pipeline by its name, after asserting that the stages
it requires ran: [`cli/exec`](cli/exec/readme.md) holds the stages, what each one requires and the
evidence it reads. `build` chains the same handlers over one or more sources, and a second build
into the same data path writes the countries it has no base for.

`query`, `http-server` and `tui` answer over every base of the data path (`served_bases`), or over
the one `--sqlite-path` names; `--countries` and the `countries` parameter narrow a question to the
countries named (`select_bases`).

every long pass of the domain reports a `progress_report`, and `cli/progress` draws all
of them with one bar: `bar(prefix, label)` and `advance`.

## what still belongs elsewhere

`cli/build.rs` chains the stages of a build. that sequencing is the domain's, not the cli's.
