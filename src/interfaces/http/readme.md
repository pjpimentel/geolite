# http
> multi-threaded http server exposing the query api

run the server, then open `/` for the web ui or `/docs` for the swagger api reference:

```sh
geolite http-server
# web ui:   http://localhost:8080/
# api docs: http://localhost:8080/docs
```
`--port 0` lets the os choose a free port; the `listening` line names the one it took and the bases
it serves.

the server opens every base of the data path, one tantivy index per base and, in each worker, one
immutable connection per base, so it writes nothing beside them and serves from read-only media,
and sees a base rebuilt under it only when started again; `/geocode` asks them all, or the ones `countries=br,ar` names (a code without a
base is a 400), and joins the answers as [`address`](../../address/readme.md) does; a text is a 503
only when no base asked has its index. `/status` reports one entry per base, `file` the name of its
sqlite (`br/data.sqlite3`), and `is_ok` is every service of every base; the answer of `/geocode`
names the bases it asked in `databases`, one entry each in the same shape. `--sqlite-path` serves one base alone.

every json answer carries an `etag`, and a request whose `If-None-Match` names it — as one tag, in
a list, weak under `W/` or as `*` — is answered `304` with no body and the etag again, so a client
holding an answer re-asks for the price of the headers. the etag of a `/geocode` answer is derived
from the question, not from the body: the md5 of the input, every option and the version of the
geolite answering. a `304` is decided before sqlite or tantivy are asked anything, so it costs
nothing, and a `200` pays the md5 of a short string instead of the body. an upgrade changes every
etag; a base rebuilt by the same version does not, so a client
holding an answer of the old base is told `304` until the next upgrade. the `/status` report and
`/openapi.json` carry the md5 of their body. an error carries no etag and is never a `304`. the
preflight allows `If-None-Match` and the answer exposes `ETag`, so a browser client can send the
one and read the other; the logic is [`etag.rs`](etag.rs).
