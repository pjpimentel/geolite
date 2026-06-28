use std::{
  fs,
  io::{Read, Write},
  path::{Path, PathBuf},
  sync::Arc,
};

pub enum download_event<'a> {
  download_start {
    total: u64,
    #[allow(dead_code)]
    threads: u8,
  },
  download_progress {
    delta: u64,
  },
  merging,
  merge_progress {
    #[allow(dead_code)]
    done: u64,
    #[allow(dead_code)]
    total: u64,
  },
  verifying_md5,
  file_already_exists {
    path: &'a Path,
  },
}

pub enum md5_status {
  ok,
  mismatch { expected: String, actual: String },
  unavailable,
}

pub struct download_output {
  pub path: PathBuf,
  pub total_bytes: u64,
  #[allow(dead_code)]
  pub threads: u8,
  pub actual_md5: String,
  pub md5: md5_status,
}

pub fn run(
  data_path: &str,
  url: &str,
  threads: u8,
  on_event: impl Fn(download_event) + Send + Sync + 'static,
) -> Option<download_output> {
  let on_event: Arc<dyn Fn(download_event) + Send + Sync> = Arc::new(on_event);

  let filename = url.split('/').next_back().unwrap_or("download.osm.pbf");
  let dest = Path::new(data_path).join(filename);

  if dest.exists() {
    on_event(download_event::file_already_exists { path: &dest });
    let total = fs::metadata(&dest).map(|m| m.len()).unwrap_or(0);
    let actual_md5 = compute_md5(&dest);
    let md5 = verify_md5(url, &actual_md5);
    return Some(download_output {
      path: dest,
      total_bytes: total,
      threads,
      actual_md5,
      md5,
    });
  }

  fs::create_dir_all(data_path).expect("failed to create data dir");

  let head = super::agent()
    .head(url)
    .call()
    .expect("head request failed");
  let total: u64 = head
    .headers()
    .get("content-length")
    .and_then(|v| v.to_str().ok())
    .and_then(|v| v.parse().ok())
    .expect("server did not return content-length");

  on_event(download_event::download_start { total, threads });

  let chunk_size = total.div_ceil(threads as u64);
  let handles: Vec<_> = (0..threads)
    .map(|i| {
      let url = url.to_string();
      let part = part_path(&dest, i);
      let start = i as u64 * chunk_size;
      let end = ((i as u64 + 1) * chunk_size - 1).min(total - 1);
      let ev = on_event.clone();
      std::thread::spawn(move || download_range(&url, start, end, &part, &*ev))
    })
    .collect();

  for handle in handles {
    handle.join().expect("download thread panicked");
  }

  on_event(download_event::merging);
  merge_parts(&dest, threads, &*on_event);

  on_event(download_event::verifying_md5);
  let actual_md5 = compute_md5(&dest);
  let md5 = verify_md5(url, &actual_md5);

  Some(download_output {
    path: dest,
    total_bytes: total,
    threads,
    actual_md5,
    md5,
  })
}

fn part_path(dest: &Path, i: u8) -> PathBuf {
  let name = dest
    .file_name()
    .unwrap_or_default()
    .to_string_lossy()
    .into_owned();
  dest.with_file_name(format!("{name}.part{i}"))
}

fn download_range(
  url: &str,
  start: u64,
  end: u64,
  part_path: &Path,
  on_event: &(dyn Fn(download_event) + Send + Sync),
) {
  let range = format!("bytes={start}-{end}");
  let response = super::agent()
    .get(url)
    .header("range", &range)
    .call()
    .expect("range request failed");
  let mut reader = response.into_body().into_reader();
  let mut file = fs::File::create(part_path).expect("failed to create part file");
  let mut buf = [0u8; 64 * 1024];
  loop {
    let n = reader.read(&mut buf).expect("failed to read chunk");
    if n == 0 {
      break;
    }
    file.write_all(&buf[..n]).expect("failed to write part");
    on_event(download_event::download_progress { delta: n as u64 });
  }
}

fn merge_parts(dest: &Path, parallel: u8, on_event: &(dyn Fn(download_event) + Send + Sync)) {
  let part_sizes: Vec<u64> = (0..parallel)
    .map(|i| {
      fs::metadata(part_path(dest, i))
        .map(|m| m.len())
        .unwrap_or(0)
    })
    .collect();
  let total: u64 = part_sizes.iter().sum();
  let mut done: u64 = 0;
  let mut file = fs::File::create(dest).expect("failed to create output file");
  for i in 0..parallel {
    let part = part_path(dest, i);
    let mut src = fs::File::open(&part).expect("failed to open part file");
    std::io::copy(&mut src, &mut file).expect("failed to merge part");
    done += part_sizes[i as usize];
    on_event(download_event::merge_progress { done, total });
    fs::remove_file(&part).expect("failed to remove part file");
  }
}

fn compute_md5(dest: &Path) -> String {
  let mut file = fs::File::open(dest).expect("failed to open file for md5");
  let mut ctx = md5::Context::new();
  let mut buf = vec![0u8; 4 * 1024 * 1024];
  loop {
    let n = file.read(&mut buf).expect("failed to read chunk for md5");
    if n == 0 {
      break;
    }
    ctx.consume(&buf[..n]);
  }
  format!("{:x}", ctx.compute())
}

fn verify_md5(url: &str, actual: &str) -> md5_status {
  let md5_url = format!("{url}.md5");
  let Ok(response) = super::agent().get(&md5_url).call() else {
    return md5_status::unavailable;
  };
  let Ok(body) = response.into_body().read_to_string() else {
    return md5_status::unavailable;
  };
  let expected = body.split_whitespace().next().unwrap_or("").to_string();
  if expected.is_empty() {
    return md5_status::unavailable;
  }
  if actual == expected {
    md5_status::ok
  } else {
    md5_status::mismatch {
      expected,
      actual: actual.to_string(),
    }
  }
}

#[cfg(test)]
#[path = "download.test.rs"]
mod download_test;
