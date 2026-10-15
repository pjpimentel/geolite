use std::sync::OnceLock;
use utoipa::{OpenApi, ToSchema};

// swagger ui page served at /docs; loads the spec from /openapi.json and the ui assets from a cdn.
pub(crate) const SWAGGER_HTML: &str = r##"<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
  <title>GeoLite API — Swagger UI</title>
  <link rel="stylesheet" href="https://unpkg.com/swagger-ui-dist@5/swagger-ui.css">
  <style>
    /* cap the readable content width (swagger-ui defaults to 1460px) */
    .swagger-ui .wrapper {
      max-width: 1024px;
    }
    /* no endpoint groups: hide the tag section header so endpoints render directly */
    .swagger-ui .opblock-tag {
      display: none;
    }
  </style>
</head>
<body>
  <div id="swagger-ui"></div>
  <script src="https://unpkg.com/swagger-ui-dist@5/swagger-ui-bundle.js" crossorigin></script>
  <script>
    window.onload = function () {
      window.ui = SwaggerUIBundle({
        url: "/openapi.json",
        dom_id: "#swagger-ui",
        defaultModelsExpandDepth: -1,
      });
    };
  </script>
</body>
</html>
"##;

// doc-only schemas: the real handlers in `handle()` write these JSON shapes by hand, so there is no
// rust type to derive them from. they exist purely to feed the openapi document.
#[derive(ToSchema)]
#[allow(dead_code)]
pub(crate) struct ApiError {
  pub(crate) error: String,
}

// doc-only path functions: never called. routing lives in `handle()`'s match; these carry the
// `#[utoipa::path]` metadata that `ApiDoc` collects via `paths(...)`.
/// geocode an address, or reverse-geocode `lat,lon` coordinates.
#[utoipa::path(
  get,
  path = "/geocode",
  params(
    ("query" = String, Query, description = "free-text address, or `lat,lon` coordinates to reverse geocode"),
    ("friendly_name_format" = Option<String>, Query, description = "template for `friendly_name`, e.g. `{admin_level_8_name}` and/or `{house_number}`"),
    ("quality" = Option<f64>, Query, minimum = 0.0, maximum = 1.0, description = "minimum match quality in `[0, 1]`; lower-quality matches are dropped, a value outside the range is a 400"),
    ("bounding_wkt" = Option<String>, Query, description = "WKT POLYGON or MULTIPOLYGON; keeps only matches inside it"),
    ("last_admin_levels" = Option<String>, Query, description = "comma-separated admin levels (e.g. `8,10`); keeps matches whose last admin level is one of them (OR); a house number is not a level, a numbered street ends at `12`"),
    ("include_wkt" = Option<bool>, Query, description = "include admin-level `wkt` geometry in the response (default `true`; set `false` to omit)"),
    ("country" = Option<String>, Query, description = "comma-separated iso 3166-1 alpha-2 codes (e.g. `br,ar`); asks only the bases of those countries, a code without a base is a 400; absent, every base of the data path answers"),
    ("If-None-Match" = Option<String>, Header, description = "the `etag` of an answer already held (a list, a weak `W/` tag and `*` count); when it names this answer the response is a 304 with no body"),
  ),
  responses(
    (status = 200, description = "geocoding result", body = crate::address::query_output,
      headers(
        ("etag" = String, description = "the etag of the answer, derived from the question and the version of geolite, quoted; sent back in `If-None-Match` it turns an unchanged answer into a 304 before anything is computed"),
      )),
    (status = 304, description = "the answer is the one `If-None-Match` names: no body, the same `etag`, nothing computed"),
    (status = 400, description = "missing or invalid query parameter", body = ApiError),
    (status = 503, description = "text_to_address service unavailable (index not loaded)", body = ApiError),
    (status = 500, description = "response serialization error", body = ApiError),
  ),
)]
#[allow(dead_code)]
fn geocode() {}

/// service and per-database availability (returns 503 when `is_ok` is false).
#[utoipa::path(
  get,
  path = "/status",
  params(
    ("If-None-Match" = Option<String>, Header, description = "the `etag` of a report already held; when it names this one the response is a 304 with no body"),
  ),
  responses(
    (status = 200, description = "all services available", body = crate::interfaces::http::status::status_output,
      headers(("etag" = String, description = "the md5 of the body, quoted; sent back in `If-None-Match` it turns an unchanged report into a 304"))),
    (status = 304, description = "the report is the one `If-None-Match` names: no body, the same `etag`"),
    (status = 503, description = "one or more services unavailable", body = crate::interfaces::http::status::status_output),
  ),
)]
#[allow(dead_code)]
fn status() {}

#[derive(OpenApi)]
#[openapi(
  info(
    title = "geolite",
    description = "",
  ),
  paths(geocode, status),
  components(schemas(
    crate::address::query_output,
    crate::address::query_service,
    crate::address::query_match,
    crate::address::admin_level,
    crate::address::query_match_attributes,
    crate::address::query_house_number,
    crate::house_number::house_number_scenario,
    crate::interfaces::http::status::status_output,
    crate::interfaces::http::status::database_status,
    ApiError,
  )),
)]
struct ApiDoc;

// the document is immutable, so build it once and reuse the json across requests.
pub(crate) fn spec_json() -> &'static str {
  static SPEC: OnceLock<String> = OnceLock::new();
  SPEC.get_or_init(|| {
    let mut doc = ApiDoc::openapi();
    // utoipa auto-fills contact from the Cargo.toml authors field; drop it from the contract.
    doc.info.contact = None;
    doc.to_json().expect("openapi spec serialization")
  })
}
