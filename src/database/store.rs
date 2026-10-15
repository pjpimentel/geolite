use std::collections::HashMap;
use std::path::Path;

use rusqlite::Connection;

use super::table;

pub const CATALOG_FILE: &str = "catalog.sqlite3";

pub fn catalog_path(data_path: &str) -> String {
  Path::new(data_path)
    .join(CATALOG_FILE)
    .to_string_lossy()
    .into_owned()
}

pub fn stem_of(pbf_path: &str) -> String {
  let name = Path::new(pbf_path)
    .file_name()
    .and_then(|name| name.to_str())
    .unwrap_or(pbf_path);
  name
    .strip_suffix(".osm.pbf")
    .or_else(|| name.strip_suffix(".pbf"))
    .unwrap_or(name)
    .to_string()
}

pub fn osm_data_path(data_path: &str, stem: &str) -> String {
  Path::new(data_path)
    .join(format!("{stem}.osm_data.sqlite3"))
    .to_string_lossy()
    .into_owned()
}

pub fn base_path(data_path: &str, iso: &str) -> String {
  Path::new(data_path)
    .join(format!("{}.sqlite3", iso.to_ascii_lowercase()))
    .to_string_lossy()
    .into_owned()
}

pub fn index_path_of(base_path: &str) -> String {
  crate::admin_level_hierarchy::search_index::default_path_for(base_path)
    .map(|p| p.to_string_lossy().into_owned())
    .unwrap_or_else(|| format!("{base_path}.tantivy"))
}

#[derive(Clone)]
pub struct base_files {
  pub iso: String,
  pub sqlite: String,
  pub index: String,
}

impl base_files {
  pub fn of(sqlite: &str) -> Self {
    let iso = iso_of_base(sqlite).unwrap_or_else(|| {
      Path::new(sqlite)
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or("base")
        .to_string()
    });
    base_files {
      iso,
      index: index_path_of(sqlite),
      sqlite: sqlite.to_string(),
    }
  }

  pub fn file_name(&self) -> String {
    Path::new(&self.sqlite)
      .file_name()
      .and_then(|name| name.to_str())
      .unwrap_or(&self.sqlite)
      .to_string()
  }

  pub fn is_sealed(&self) -> bool {
    super::is_sealed(&self.sqlite)
  }
}

pub fn iso_of_base(path: &str) -> Option<String> {
  let stem = Path::new(path).file_name()?.to_str()?.strip_suffix(".sqlite3")?;
  let is_iso = stem.len() == 2 && stem.bytes().all(|b| b.is_ascii_lowercase());
  is_iso.then(|| stem.to_string())
}

pub fn bases(data_path: &str) -> Vec<base_files> {
  let Ok(entries) = std::fs::read_dir(data_path) else {
    return vec![];
  };
  let mut found: Vec<base_files> = entries
    .flatten()
    .filter_map(|entry| {
      let path = entry.path().to_string_lossy().into_owned();
      iso_of_base(&path).map(|iso| base_files {
        iso,
        index: index_path_of(&path),
        sqlite: path,
      })
    })
    .collect();
  found.sort_by(|a, b| a.iso.cmp(&b.iso));
  found
}

pub fn open_catalog(data_path: &str) -> Connection {
  let conn = super::open_writable(&catalog_path(data_path));
  crate::osm_pbf_file::osm_pbf_files::create_table(&conn);
  conn
}

pub fn open_source(data_path: &str, stem: &str) -> Connection {
  let conn = open_catalog(data_path);
  let osm_data = osm_data_path(data_path, stem);
  conn
    .execute("ATTACH DATABASE ?1 AS osm_data", [&osm_data])
    .expect("failed to attach osm_data");
  conn
    .execute_batch(
      "PRAGMA osm_data.journal_mode=WAL;
       PRAGMA osm_data.synchronous=NORMAL;
       PRAGMA osm_data.cache_size=-32768;",
    )
    .expect("failed to set osm_data pragmas");
  crate::osm_pbf_file::osm_pbf_blob_chunks::create_table(&conn);
  crate::osm_node::osm_nodes::create_table(&conn);
  crate::osm_way::osm_ways::create_table(&conn);
  crate::osm_relation::osm_relations::create_table(&conn);
  conn
}

pub fn open_base_write(path: &str) -> Connection {
  let conn = super::open_writable(path);
  crate::admin_level::admin_levels::create_table(&conn);
  crate::admin_level::repository::add_merged_way_ids(&conn);
  crate::admin_level_hierarchy::admin_levels_hierarchy::create_table(&conn);
  crate::admin_level::spatial_index::create_table(&conn);
  crate::house_number::house_numbers::create_table(&conn);
  conn
}

pub fn open_base_readonly(path: &str) -> Connection {
  super::open_readonly(path)
}

pub fn open_base_immutable(path: &str) -> Connection {
  super::open_immutable(path)
}

// a base is attached to a source connection one at a time: the ledger counts the rows of a file
// through the chunks of its osm_data, and every base of the source has to be asked
pub fn attach_base(conn: &Connection, path: &str) {
  let canonical = std::fs::canonicalize(path)
    .map(|p| p.to_string_lossy().into_owned())
    .unwrap_or_else(|_| path.to_string());
  conn
    .execute("ATTACH DATABASE ?1 AS base", [&format!("file:{canonical}?mode=ro")])
    .expect("failed to attach a base");
}

pub fn detach_base(conn: &Connection) {
  conn
    .execute_batch("DETACH DATABASE base")
    .expect("failed to detach a base");
}

// the bases a build wrote, with their indexes, read-only at the os level from here on: a base
// built once is final
pub fn seal_bases(bases: &[base_files]) {
  let mut sealed: Vec<String> = Vec::new();
  for base in bases {
    for path in [&base.sqlite, &base.index] {
      if Path::new(path).exists() {
        super::seal_path(path);
        sealed.push(file_name_of(path));
      }
    }
  }
  println!("\x1b[1;32msealed\x1b[0m {} — read-only", sealed.join(", "));
}

fn file_name_of(path: &str) -> String {
  Path::new(path)
    .file_name()
    .and_then(|name| name.to_str())
    .unwrap_or(path)
    .to_string()
}

pub fn remove_base(base: &base_files) {
  for suffix in ["", "-wal", "-shm"] {
    let p = format!("{}{suffix}", base.sqlite);
    if Path::new(&p).exists() {
      std::fs::remove_file(&p).expect("failed to remove a base file");
    }
  }
  if Path::new(&base.index).exists() {
    std::fs::remove_dir_all(&base.index).expect("failed to remove the index of a base");
  }
}

pub fn remove_osm_data_files(data_path: &str, stem: &str) -> u64 {
  let sibling = osm_data_path(data_path, stem);
  let mut bytes = 0u64;
  for suffix in ["", "-wal", "-shm"] {
    let p = format!("{sibling}{suffix}");
    if Path::new(&p).exists() {
      bytes += std::fs::metadata(&p).map(|m| m.len()).unwrap_or(0);
      std::fs::remove_file(&p).expect("failed to remove an osm_data file");
    }
  }
  bytes
}

pub fn destroy_source(data_path: &str, stem: &str, osm_pbf_blob_chunks: bool) {
  if osm_pbf_blob_chunks {
    remove_osm_data_files(data_path, stem);
    return;
  }
  let conn = open_source(data_path, stem);
  crate::osm_node::repository::drop_table(&conn);
  crate::osm_way::repository::drop_table(&conn);
  crate::osm_relation::repository::drop_table(&conn);
  conn
    .execute_batch("VACUUM osm_data;")
    .expect("failed to vacuum osm_data");
}

pub fn destroy_base(path: &str, admin_levels: bool, house_numbers: bool) {
  {
    let conn = open_base_write(path);
    if house_numbers {
      crate::house_number::repository::drop_table(&conn);
    }
    if admin_levels {
      crate::admin_level_hierarchy::repository::drop_table(&conn);
      crate::admin_level::spatial_index::drop_table(&conn);
      crate::admin_level::repository::drop_table(&conn);
      crate::house_number::repository::drop_table(&conn);
    }
    conn.execute_batch("VACUUM;").expect("failed to vacuum");
  }
  open_base_write(path);
}

// the bases a source writes: one connection per country, opened on the first row of that country
// and never attached, so the ten attachments sqlite allows are never in the way; a base a build
// sealed takes no row
pub struct base_pool<'a> {
  data_path: &'a str,
  open: HashMap<&'static str, Connection>,
  touched: Vec<&'static str>,
  sealed: Vec<&'static str>,
}

impl<'a> base_pool<'a> {
  pub fn new(data_path: &'a str) -> Self {
    base_pool {
      data_path,
      open: HashMap::new(),
      touched: Vec::new(),
      sealed: Vec::new(),
    }
  }

  pub fn conn_for(&mut self, iso: &str) -> Option<&Connection> {
    let key = crate::presets::iso_key(iso).expect("the country of a row has a boundary");
    if self.sealed.contains(&key) {
      return None;
    }
    if !self.open.contains_key(key) {
      let path = base_path(self.data_path, key);
      if super::is_sealed(&path) {
        self.sealed.push(key);
        return None;
      }
      self.open.insert(key, open_base_write(&path));
      self.touched.push(key);
    }
    self.open.get(key)
  }

  pub fn touched(&self) -> &[&'static str] {
    &self.touched
  }

  pub fn open_bases(&self) -> impl Iterator<Item = (&'static str, &Connection)> {
    self.touched.iter().map(|iso| (*iso, &self.open[iso]))
  }

  // the bases already on disk, the ones a resume reads what it has from
  pub fn on_disk(&self) -> Vec<base_files> {
    bases(self.data_path)
  }

  // the bases on disk a source may still write
  pub fn writable_on_disk(&self) -> Vec<base_files> {
    bases(self.data_path)
      .into_iter()
      .filter(|base| !base.is_sealed())
      .collect()
  }
}
