# http
> multi-threaded http server exposing the query api

run the server, then open `/` for the web ui or `/docs` for the swagger api reference:

```sh
geolite http-server
# web ui:   http://localhost:8080/
# api docs: http://localhost:8080/docs
```
`--port 0` lets the os choose a free port; the `listening` line names the one it took.
