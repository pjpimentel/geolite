# interfaces
> the ways into the domain: each one reads its input, opens the connection, calls the domain and renders the answer

```
cli/    the `geolite` command: parses the arguments, resolves the preset and the paths, and runs one subcommand
http/   the http server that `geolite http-server` starts: `/geocode`, `/status`, `/openapi.json`, `/docs`
```

an interface owns no rule. what a query means, how a region is parsed or how a label is built
lives in the concept folders, so the cli and the http server answer the same question the same
way, and an interface added later reuses the domain instead of repeating it.

the cli is the entry point: `main` calls `cli::run`, which resolves the preset and the paths once
and dispatches; `http-server` is a subcommand that binds the port and hands over to `http::serve`.
any other interface is launched the same way, as a subcommand of the cli.

## what still belongs elsewhere

`cli/build.rs` chains the stages of a build, and each stage draws its own progress bar in
`cli/extract` and `cli/index`. that sequencing is the domain's, not the cli's, and one progress
report shared by every stage is an item in the backlog.
