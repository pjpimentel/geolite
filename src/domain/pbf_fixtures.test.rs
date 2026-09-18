use prost::Message;

use crate::domain::osm_pbf_file::message::{
  blob_header_msg, blob_msg, dense_info_msg, dense_nodes_msg, header_bbox_msg, header_block_msg,
  info_msg, node_msg, primitive_block_msg, primitive_group_msg, relation_msg, string_table_msg,
  way_msg,
};

#[derive(Clone, Copy)]
pub(crate) enum blob_compression {
  raw,
  zlib,
  // neither raw nor zlib_data — reaches the panic!("unsupported blob compression")
  none,
}

pub(crate) fn make_blob(payload: &[u8], compression: blob_compression) -> Vec<u8> {
  let blob = match compression {
    blob_compression::raw => blob_msg {
      raw: Some(payload.to_vec()),
      raw_size: Some(payload.len() as i32),
      zlib_data: None,
    },
    blob_compression::zlib => {
      use std::io::Write;
      let mut encoder =
        flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());
      encoder.write_all(payload).expect("failed to deflate blob");
      blob_msg {
        raw: None,
        raw_size: Some(payload.len() as i32),
        zlib_data: Some(encoder.finish().expect("failed to finish deflate")),
      }
    }
    blob_compression::none => blob_msg {
      raw: None,
      raw_size: Some(payload.len() as i32),
      zlib_data: None,
    },
  };
  blob.encode_to_vec()
}

// frames a blob the way the file does: [be_u32(header_len)][blob_header][blob]
pub(crate) fn make_chunk(kind: &str, blob: &[u8]) -> Vec<u8> {
  let header = blob_header_msg {
    r#type: kind.to_string(),
    datasize: blob.len() as u32,
  }
  .encode_to_vec();

  let mut out = (header.len() as u32).to_be_bytes().to_vec();
  out.extend_from_slice(&header);
  out.extend_from_slice(blob);
  out
}

pub(crate) fn write_pbf(path: &str, chunks: &[Vec<u8>]) {
  std::fs::write(path, chunks.concat().as_slice()).expect("failed to write fixture pbf");
}

// the block string table; index 0 is always "" by the convention of the format
struct string_table {
  entries: Vec<String>,
  index: std::collections::HashMap<String, u32>,
}

impl string_table {
  fn new() -> Self {
    Self {
      entries: vec![String::new()],
      index: std::collections::HashMap::new(),
    }
  }

  fn intern(&mut self, s: &str) -> u32 {
    if let Some(&i) = self.index.get(s) {
      return i;
    }
    let i = self.entries.len() as u32;
    self.entries.push(s.to_string());
    self.index.insert(s.to_string(), i);
    i
  }

  fn into_msg(self) -> string_table_msg {
    string_table_msg {
      s: self.entries.into_iter().map(|e| e.into_bytes()).collect(),
    }
  }
}

pub(crate) struct header_spec {
  pub bbox: Option<(f64, f64, f64, f64)>,
  pub required_features: Vec<String>,
  pub optional_features: Vec<String>,
  pub writingprogram: Option<String>,
  pub source: Option<String>,
  pub replication_timestamp: Option<i64>,
  pub replication_sequence_number: Option<i64>,
  pub replication_base_url: Option<String>,
}

impl Default for header_spec {
  // every field is filled by default, so the decoder exercises each of them
  fn default() -> Self {
    Self {
      // binary-exact fractions, so the degrees -> nanodegrees -> degrees round trip is exact
      // and the tests can compare the wkt literally
      bbox: Some((-9.5, -9.0, 38.75, 38.5)),
      required_features: vec!["OsmSchema-V0.6".to_string(), "DenseNodes".to_string()],
      optional_features: vec!["Has_Metadata".to_string()],
      writingprogram: Some("geolite-test".to_string()),
      source: Some("fixture".to_string()),
      replication_timestamp: Some(1_700_000_000),
      replication_sequence_number: Some(4_242),
      replication_base_url: Some("https://example.invalid/replication".to_string()),
    }
  }
}

pub(crate) fn header_blob(spec: &header_spec, compression: blob_compression) -> Vec<u8> {
  const NANO: f64 = 1e9;
  let block = header_block_msg {
    bbox: spec.bbox.map(|(left, right, top, bottom)| header_bbox_msg {
      left: (left * NANO) as i64,
      right: (right * NANO) as i64,
      top: (top * NANO) as i64,
      bottom: (bottom * NANO) as i64,
    }),
    required_features: spec.required_features.clone(),
    optional_features: spec.optional_features.clone(),
    writingprogram: spec.writingprogram.clone(),
    source: spec.source.clone(),
    osmosis_replication_timestamp: spec.replication_timestamp,
    osmosis_replication_sequence_number: spec.replication_sequence_number,
    osmosis_replication_base_url: spec.replication_base_url.clone(),
  };
  make_blob(&block.encode_to_vec(), compression)
}

pub(crate) fn header_chunk() -> Vec<u8> {
  make_chunk(
    "OSMHeader",
    &header_blob(&header_spec::default(), blob_compression::zlib),
  )
}

#[derive(Clone)]
pub(crate) struct node_spec {
  pub id: i64,
  pub lat: f64,
  pub lon: f64,
  pub tags: Vec<(String, String)>,
}

#[derive(Clone)]
pub(crate) struct way_spec {
  pub id: i64,
  pub refs: Vec<i64>,
  pub tags: Vec<(String, String)>,
}

// a relation member: (type 0=node/1=way/2=relation, id, role)
#[derive(Clone)]
pub(crate) struct relation_spec {
  pub id: i64,
  pub members: Vec<(i32, i64, String)>,
  pub tags: Vec<(String, String)>,
}

pub(crate) fn node(id: i64, lat: f64, lon: f64, tags: &[(&str, &str)]) -> node_spec {
  node_spec {
    id,
    lat,
    lon,
    tags: owned(tags),
  }
}

pub(crate) fn way(id: i64, refs: &[i64], tags: &[(&str, &str)]) -> way_spec {
  way_spec {
    id,
    refs: refs.to_vec(),
    tags: owned(tags),
  }
}

pub(crate) fn relation(
  id: i64,
  members: &[(i32, i64, &str)],
  tags: &[(&str, &str)],
) -> relation_spec {
  relation_spec {
    id,
    members: members
      .iter()
      .map(|&(t, i, r)| (t, i, r.to_string()))
      .collect(),
    tags: owned(tags),
  }
}

fn owned(tags: &[(&str, &str)]) -> Vec<(String, String)> {
  tags
    .iter()
    .map(|&(k, v)| (k.to_string(), v.to_string()))
    .collect()
}

pub(crate) struct block_spec {
  pub dense: Vec<node_spec>,
  pub plain: Vec<node_spec>,
  pub ways: Vec<way_spec>,
  pub relations: Vec<relation_spec>,
  pub granularity: i32,
  pub date_granularity: i32,
  pub lat_offset: i64,
  pub lon_offset: i64,
  // true emits info/denseinfo, exercising those fields in the decoder
  pub with_info: bool,
  // false omits granularity/offsets, exercising the defaults of the decoder
  pub emit_block_options: bool,
}

impl Default for block_spec {
  fn default() -> Self {
    Self {
      dense: Vec::new(),
      plain: Vec::new(),
      ways: Vec::new(),
      relations: Vec::new(),
      granularity: 100,
      date_granularity: 1000,
      lat_offset: 0,
      lon_offset: 0,
      with_info: true,
      emit_block_options: true,
    }
  }
}

impl block_spec {
  // turns degrees into the format's raw integer, honouring granularity and offset:
  //   degrees = (offset + granularity * raw) * 1e-9
  fn to_raw(&self, degrees: f64, offset: i64) -> i64 {
    ((degrees * 1e9 - offset as f64) / self.granularity as f64).round() as i64
  }
}

pub(crate) fn data_blob(spec: &block_spec, compression: blob_compression) -> Vec<u8> {
  let mut st = string_table::new();

  let dense = if spec.dense.is_empty() {
    None
  } else {
    let mut ids = Vec::new();
    let mut lats = Vec::new();
    let mut lons = Vec::new();
    let mut keys_vals = Vec::new();
    let (mut id_acc, mut lat_acc, mut lon_acc) = (0i64, 0i64, 0i64);

    for n in &spec.dense {
      let lat_raw = spec.to_raw(n.lat, spec.lat_offset);
      let lon_raw = spec.to_raw(n.lon, spec.lon_offset);
      ids.push(n.id - id_acc);
      lats.push(lat_raw - lat_acc);
      lons.push(lon_raw - lon_acc);
      id_acc = n.id;
      lat_acc = lat_raw;
      lon_acc = lon_raw;

      for (k, v) in &n.tags {
        keys_vals.push(st.intern(k) as i32);
        keys_vals.push(st.intern(v) as i32);
      }
      // 0 ends the tag list of this node
      keys_vals.push(0);
    }

    let count = spec.dense.len();
    Some(dense_nodes_msg {
      id: ids,
      denseinfo: spec.with_info.then(|| dense_info_msg {
        version: vec![1; count],
        timestamp: vec![1_700_000; count],
        changeset: vec![0; count],
        uid: vec![0; count],
        user_sid: vec![0; count],
        visible: vec![true; count],
      }),
      lat: lats,
      lon: lons,
      keys_vals,
    })
  };

  let plain: Vec<node_msg> = spec
    .plain
    .iter()
    .map(|n| {
      let (keys, vals) = intern_tags(&mut st, &n.tags);
      node_msg {
        id: n.id,
        keys,
        vals,
        info: spec.with_info.then(default_info),
        lat: spec.to_raw(n.lat, spec.lat_offset),
        lon: spec.to_raw(n.lon, spec.lon_offset),
      }
    })
    .collect();

  let ways: Vec<way_msg> = spec
    .ways
    .iter()
    .map(|w| {
      let (keys, vals) = intern_tags(&mut st, &w.tags);
      let mut acc = 0i64;
      let refs = w
        .refs
        .iter()
        .map(|&r| {
          let delta = r - acc;
          acc = r;
          delta
        })
        .collect();
      way_msg {
        id: w.id,
        keys,
        vals,
        info: spec.with_info.then(default_info),
        refs,
        lat: vec![],
        lon: vec![],
      }
    })
    .collect();

  let relations: Vec<relation_msg> = spec
    .relations
    .iter()
    .map(|r| {
      let (keys, vals) = intern_tags(&mut st, &r.tags);
      let mut acc = 0i64;
      let mut roles_sid = Vec::new();
      let mut memids = Vec::new();
      let mut types = Vec::new();
      for (member_type, id, role) in &r.members {
        roles_sid.push(st.intern(role) as i32);
        memids.push(id - acc);
        acc = *id;
        types.push(*member_type);
      }
      relation_msg {
        id: r.id,
        keys,
        vals,
        info: spec.with_info.then(default_info),
        roles_sid,
        memids,
        types,
      }
    })
    .collect();

  let block = primitive_block_msg {
    stringtable: Some(st.into_msg()),
    primitivegroup: vec![primitive_group_msg {
      nodes: plain,
      dense,
      ways,
      relations,
    }],
    granularity: spec.emit_block_options.then_some(spec.granularity),
    date_granularity: spec.emit_block_options.then_some(spec.date_granularity),
    lat_offset: spec.emit_block_options.then_some(spec.lat_offset),
    lon_offset: spec.emit_block_options.then_some(spec.lon_offset),
  };

  make_blob(&block.encode_to_vec(), compression)
}

fn intern_tags(st: &mut string_table, tags: &[(String, String)]) -> (Vec<u32>, Vec<u32>) {
  let mut keys = Vec::new();
  let mut vals = Vec::new();
  for (k, v) in tags {
    keys.push(st.intern(k));
    vals.push(st.intern(v));
  }
  (keys, vals)
}

fn default_info() -> info_msg {
  info_msg {
    version: Some(1),
    timestamp: Some(1_700_000),
    changeset: Some(99),
    uid: Some(7),
    user_sid: Some(0),
    visible: Some(true),
  }
}

pub(crate) fn data_chunk(spec: &block_spec, compression: blob_compression) -> Vec<u8> {
  make_chunk("OSMData", &data_blob(spec, compression))
}

pub(crate) fn tiny_pbf() -> Vec<Vec<u8>> {
  vec![
    header_chunk(),
    data_chunk(
      &block_spec {
        dense: vec![node(1, 38.7, -9.1, &[("name", "Marco Zero")])],
        ..Default::default()
      },
      blob_compression::zlib,
    ),
    data_chunk(
      &block_spec {
        ways: vec![way(
          100,
          &[1, 2, 3],
          &[("highway", "residential"), ("name", "Rua Augusta")],
        )],
        ..Default::default()
      },
      blob_compression::zlib,
    ),
    data_chunk(
      &block_spec {
        relations: vec![relation(
          200,
          &[(1, 100, "outer"), (0, 1, "admin_centre"), (2, 300, "subarea")],
          &[("name", "Lisboa"), ("admin_level", "8")],
        )],
        ..Default::default()
      },
      blob_compression::zlib,
    ),
  ]
}

pub(crate) struct tempdir_guard {
  pub path: std::path::PathBuf,
}

impl tempdir_guard {
  pub fn new(tag: &str) -> Self {
    static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let seq = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let nanos = std::time::SystemTime::now()
      .duration_since(std::time::UNIX_EPOCH)
      .expect("clock before epoch")
      .as_nanos();
    let path = std::env::temp_dir().join(format!(
      "geolite-test-extract-{tag}-{}-{nanos}-{seq}",
      std::process::id()
    ));
    std::fs::create_dir_all(&path).expect("failed to create test dir");
    Self { path }
  }
}

impl Drop for tempdir_guard {
  fn drop(&mut self) {
    let _ = std::fs::remove_dir_all(&self.path);
  }
}

// a scene on disk: the guard wipes the whole directory (pbf + the main sqlite + the
// osm_data sqlite + the -wal/-shm files) when it goes out of scope
pub(crate) struct temp_scene {
  pub guard: tempdir_guard,
  pub pbf_path: String,
  pub db_path: String,
}

pub(crate) fn temp_scene(tag: &str) -> temp_scene {
  let guard = tempdir_guard::new(tag);
  let pbf_path = guard.path.join("fixture.osm.pbf").to_string_lossy().into_owned();
  let db_path = guard.path.join("geolite.sqlite3").to_string_lossy().into_owned();
  temp_scene {
    guard,
    pbf_path,
    db_path,
  }
}

pub(crate) fn memory_db() -> rusqlite::Connection {
  crate::database::open_write(":memory:")
}

fn tag_map(tags: &[(&str, &str)]) -> std::collections::HashMap<String, String> {
  tags
    .iter()
    .map(|&(k, v)| (k.to_string(), v.to_string()))
    .collect()
}

pub(crate) fn insert_node(
  conn: &rusqlite::Connection,
  id: u64,
  lon: f64,
  lat: f64,
  tags: &[(&str, &str)],
) {
  let node = crate::domain::osm_node::osm_node {
    id: id as i64,
    lat,
    lon,
    tags: tag_map(tags),
  };
  let row = crate::domain::osm_node::osm_node_row::encode(
    &node,
    0,
    &mut crate::database::jsonb::encoder::new(),
  );
  crate::domain::osm_node::repository::insert_rows(conn, &[row]);
}

pub(crate) fn insert_way(
  conn: &rusqlite::Connection,
  id: u64,
  refs: &[i64],
  tags: &[(&str, &str)],
) {
  let way = crate::domain::osm_way::osm_way {
    id: id as i64,
    refs: refs.to_vec(),
    tags: tag_map(tags),
  };
  let row = crate::domain::osm_way::osm_way_row::encode(
    &way,
    0,
    &mut crate::database::jsonb::encoder::new(),
  );
  crate::domain::osm_way::repository::insert_rows(conn, &[row]);
}

// members: (type 0=node/1=way/2=relation, id, role)
pub(crate) fn insert_relation(
  conn: &rusqlite::Connection,
  id: u64,
  members: &[(i32, i64, &str)],
  tags: &[(&str, &str)],
) {
  use crate::domain::osm_relation::entity::{osm_member_type, osm_relation, osm_relation_member};

  let relation = osm_relation {
    id: id as i64,
    tags: tag_map(tags),
    members: members
      .iter()
      .map(|&(kind, member_id, role)| osm_relation_member {
        osm_member_type: match kind {
          1 => osm_member_type::way,
          2 => osm_member_type::relation,
          _ => osm_member_type::node,
        },
        id: member_id,
        role: role.to_string(),
      })
      .collect(),
  };
  let row = crate::domain::osm_relation::osm_relation_row::encode(
    &relation,
    0,
    &mut crate::database::jsonb::encoder::new(),
  );
  crate::domain::osm_relation::repository::insert_rows(conn, &[row]);
}

// cria um way fechado (quadrado) com os nodes correspondentes, retornando o way_id

// runs a level stage and returns the progress events it emitted

pub(crate) fn indexed_scene(tag: &str, chunks: &[Vec<u8>]) -> (temp_scene, u32) {
  let scene = temp_scene(tag);
  write_pbf(&scene.pbf_path, chunks);

  let conn = crate::database::open_write(&scene.db_path);
  let file_id = crate::domain::osm_pbf_file::repository::ensure_by_file_path(&conn, &scene.pbf_path);
  crate::domain::osm_pbf_file::blob_scanner::run(&scene.pbf_path, &conn, file_id, |_| {});
  drop(conn);

  (scene, file_id)
}
