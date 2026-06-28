pub fn run(data_path: &str, sqlite_path: &str, on_drop: impl Fn(&str, u64)) {
  let osm = crate::database::osm_data_path(sqlite_path);
  let siblings = [
    osm.clone(),
    format!("{osm}-wal"),
    format!("{osm}-shm"),
  ];
  let mut osm_bytes = 0u64;
  for path in &siblings {
    if std::path::Path::new(path).exists() {
      osm_bytes += std::fs::metadata(path).map(|m| m.len()).unwrap_or(0);
      std::fs::remove_file(path).expect("failed to remove osm_data sibling file");
    }
  }
  on_drop("osm_data.sqlite3", osm_bytes);

  let Ok(entries) = std::fs::read_dir(data_path) else {
    return;
  };
  for entry in entries.flatten() {
    let path = entry.path();
    let name = path.file_name().and_then(|n| n.to_str()).unwrap_or_default();
    if !name.ends_with(".osm.pbf") {
      continue;
    }
    let bytes = entry.metadata().map(|m| m.len()).unwrap_or(0);
    std::fs::remove_file(&path).expect("failed to remove osm.pbf file");
    on_drop(name, bytes);
  }
}
