use prost::Message;
use std::{
  collections::VecDeque,
  fs,
  io::{self, Read},
  sync::{Arc, Condvar, Mutex},
};

pub mod jsonb_encode;
pub mod osm_nodes;
pub mod osm_relations;
pub mod osm_ways;

#[derive(prost::Message)]
pub(super) struct blob_msg {
  #[prost(bytes = "vec", optional, tag = "1")]
  raw: Option<Vec<u8>>,
  #[prost(int32, optional, tag = "2")]
  raw_size: Option<i32>,
  #[prost(bytes = "vec", optional, tag = "3")]
  zlib_data: Option<Vec<u8>>,
}

#[derive(prost::Message)]
struct string_table_msg {
  #[prost(bytes = "vec", repeated, tag = "1")]
  s: Vec<Vec<u8>>,
}

#[derive(prost::Message)]
struct primitive_block_msg {
  #[prost(message, optional, tag = "1")]
  stringtable: Option<string_table_msg>,
  #[prost(message, repeated, tag = "2")]
  primitivegroup: Vec<primitive_group_msg>,
  #[prost(int32, optional, tag = "17")]
  granularity: Option<i32>,
  #[prost(int64, optional, tag = "19")]
  lat_offset: Option<i64>,
  #[prost(int64, optional, tag = "20")]
  lon_offset: Option<i64>,
  #[prost(int32, optional, tag = "18")]
  date_granularity: Option<i32>,
}

#[derive(prost::Message)]
struct primitive_group_msg {
  #[prost(message, repeated, tag = "1")]
  nodes: Vec<node_msg>,
  #[prost(message, optional, tag = "2")]
  dense: Option<dense_nodes_msg>,
  #[prost(message, repeated, tag = "3")]
  ways: Vec<way_msg>,
  #[prost(message, repeated, tag = "4")]
  relations: Vec<relation_msg>,
}

#[derive(prost::Message)]
struct info_msg {
  #[prost(int32, optional, tag = "1", default = "-1")]
  version: Option<i32>,
  #[prost(int64, optional, tag = "2")]
  timestamp: Option<i64>,
  #[prost(int64, optional, tag = "3")]
  changeset: Option<i64>,
  #[prost(int32, optional, tag = "4")]
  uid: Option<i32>,
  #[prost(uint32, optional, tag = "5")]
  user_sid: Option<u32>,
  #[prost(bool, optional, tag = "6")]
  visible: Option<bool>,
}

#[derive(prost::Message)]
struct dense_info_msg {
  #[prost(int32, repeated, tag = "1")]
  version: Vec<i32>,
  #[prost(sint64, repeated, tag = "2")]
  timestamp: Vec<i64>,
  #[prost(sint64, repeated, tag = "3")]
  changeset: Vec<i64>,
  #[prost(sint32, repeated, tag = "4")]
  uid: Vec<i32>,
  #[prost(sint32, repeated, tag = "5")]
  user_sid: Vec<i32>,
  #[prost(bool, repeated, tag = "6")]
  visible: Vec<bool>,
}

#[derive(prost::Message)]
pub(super) struct node_msg {
  #[prost(sint64, tag = "1")]
  id: i64,
  #[prost(uint32, repeated, tag = "2")]
  keys: Vec<u32>,
  #[prost(uint32, repeated, tag = "3")]
  vals: Vec<u32>,
  #[prost(message, optional, tag = "4")]
  info: Option<info_msg>,
  #[prost(sint64, tag = "8")]
  lat: i64,
  #[prost(sint64, tag = "9")]
  lon: i64,
}

#[derive(prost::Message)]
pub(super) struct dense_nodes_msg {
  #[prost(sint64, repeated, tag = "1")]
  id: Vec<i64>,
  #[prost(message, optional, tag = "5")]
  denseinfo: Option<dense_info_msg>,
  #[prost(sint64, repeated, tag = "8")]
  lat: Vec<i64>,
  #[prost(sint64, repeated, tag = "9")]
  lon: Vec<i64>,
  #[prost(int32, repeated, tag = "10")]
  keys_vals: Vec<i32>,
}

#[derive(prost::Message)]
pub(super) struct way_msg {
  #[prost(int64, tag = "1")]
  id: i64,
  #[prost(uint32, repeated, tag = "2")]
  keys: Vec<u32>,
  #[prost(uint32, repeated, tag = "3")]
  vals: Vec<u32>,
  #[prost(message, optional, tag = "4")]
  info: Option<info_msg>,
  #[prost(sint64, repeated, tag = "8")]
  refs: Vec<i64>,
  #[prost(sint64, repeated, tag = "9")]
  lat: Vec<i64>,
  #[prost(sint64, repeated, tag = "10")]
  lon: Vec<i64>,
}

#[derive(prost::Message)]
pub(super) struct relation_msg {
  #[prost(int64, tag = "1")]
  id: i64,
  #[prost(uint32, repeated, tag = "2")]
  keys: Vec<u32>,
  #[prost(uint32, repeated, tag = "3")]
  vals: Vec<u32>,
  #[prost(message, optional, tag = "4")]
  info: Option<info_msg>,
  #[prost(int32, repeated, tag = "8")]
  roles_sid: Vec<i32>,
  #[prost(sint64, repeated, tag = "9")]
  memids: Vec<i64>,
  #[prost(int32, repeated, tag = "10")]
  types: Vec<i32>,
}

pub struct data_opts {
  pub include_nodes: bool,
  pub include_ways: bool,
  pub include_relations: bool,
  #[allow(dead_code)]
  pub ignore_info: bool,
  pub tags_include: Option<Vec<String>>,
  pub tags_ignore: Option<Vec<String>>,
  pub buffer_bytes: usize,
}

struct decoded_blob_output {
  nodes: Vec<osm_nodes::osm_node>,
  ways: Vec<osm_ways::osm_way>,
  relations: Vec<osm_relations::osm_relation>,
}

struct decoded_blob {
  nodes: Vec<crate::database::osm_nodes::osm_node_row>,
  ways: Vec<crate::database::osm_ways::osm_way_row>,
  relations: Vec<crate::database::osm_relations::osm_relation_row>,
}

#[derive(Default)]
struct buffer_data {
  nodes: VecDeque<crate::database::osm_nodes::osm_node_row>,
  ways: VecDeque<crate::database::osm_ways::osm_way_row>,
  relations: VecDeque<crate::database::osm_relations::osm_relation_row>,
}

impl buffer_data {
  fn row_count(&self) -> usize {
    self.nodes.len() + self.ways.len() + self.relations.len()
  }
}

struct raw_blob {
  chunk: crate::database::osm_pbf_blob_chunks::osm_pbf_blob_chunk,
  data: Vec<u8>,
}

struct raw_queue_state {
  items: VecDeque<raw_blob>,
  reader_done: bool,
}

struct raw_queue {
  inner: Mutex<raw_queue_state>,
  not_empty: Condvar,
  not_full: Condvar,
}

struct write_buffer_state {
  current: buffer_data,
  bytes_current: usize,
  decoders_done: bool,
}

struct write_buffer {
  inner: Mutex<write_buffer_state>,
  has_work: Condvar,
  not_too_full: Condvar,
  soft_threshold: usize,
  hard_limit: usize,
  row_threshold: usize,
  // max rows que o writer drena por flush — limita o tamanho de cada transacao
  // pra evitar flushes monstruosos quando o buffer eh enorme e o writer cai atras
  flush_cap_rows: usize,
}

fn decoded_blob_bytes(blob: &decoded_blob) -> usize {
  let nodes_stack =
    blob.nodes.capacity() * std::mem::size_of::<crate::database::osm_nodes::osm_node_row>();
  let nodes_heap: usize = blob.nodes.iter().map(|r| r.payload.capacity()).sum();

  let ways_stack =
    blob.ways.capacity() * std::mem::size_of::<crate::database::osm_ways::osm_way_row>();
  let ways_heap: usize = blob.ways.iter().map(|r| r.payload.capacity()).sum();

  let rels_stack = blob.relations.capacity()
    * std::mem::size_of::<crate::database::osm_relations::osm_relation_row>();
  let rels_heap: usize = blob.relations.iter().map(|r| r.payload.capacity()).sum();

  nodes_stack + nodes_heap + ways_stack + ways_heap + rels_stack + rels_heap
}

pub(crate) struct worker_progress {
  pub thread_id: usize,
  pub chunks_processed: usize,
  pub node_count: usize,
  pub way_count: usize,
  pub relation_count: usize,
}

pub struct progress {
  pub total_chunks: usize,
  pub chunks_done: usize,
  pub node_count: usize,
  pub way_count: usize,
  pub relation_count: usize,
  pub nodes_written: usize,
  pub ways_written: usize,
  pub relations_written: usize,
  pub bytes_flushed: usize,
  pub flushes_done: usize,
  #[allow(dead_code)]
  pub(crate) workers: Vec<worker_progress>,
}

pub fn run(
  pbf: &str,
  chunks: Vec<crate::database::osm_pbf_blob_chunks::osm_pbf_blob_chunk>,
  write_conn: rusqlite::Connection,
  opts: data_opts,
  threads: &u8,
  on_progress: impl Fn(progress) + Send + 'static,
) -> (usize, usize, usize) {
  let total_chunks = chunks.len();
  // n-1 decoders (reader+writer compartilham com decoders no scheduler);
  // 1 writer dedicado. reader eh leve (~0.3s work em 80s total) e nao precisa
  // de core dedicado.
  let worker_threads = threads.saturating_sub(1).max(1);
  let queue_cap = worker_threads as usize * 2;

  let read_q = Arc::new(raw_queue {
    inner: Mutex::new(raw_queue_state {
      items: VecDeque::new(),
      reader_done: false,
    }),
    not_empty: Condvar::new(),
    not_full: Condvar::new(),
  });
  let (prog_tx, prog_rx) = std::sync::mpsc::channel::<prog_event>();
  let buffer_bytes = opts.buffer_bytes;
  let opts = Arc::new(opts);

  // double-buffer: decoders push directly into write_buf.current; writer swaps
  // the full buffer for a new empty one (atomic under the mutex) and flushes
  // outside the lock — decoders push to the new buffer in parallel with the flush
  let write_buf = Arc::new(write_buffer {
    inner: Mutex::new(write_buffer_state {
      current: buffer_data::default(),
      bytes_current: 0,
      decoders_done: false,
    }),
    has_work: Condvar::new(),
    not_too_full: Condvar::new(),
    soft_threshold: buffer_bytes * 4 / 5,
    hard_limit: buffer_bytes,
    row_threshold: 10_000,
    flush_cap_rows: 100_000,
  });

  let writer = writer_thread(write_conn, write_buf.clone(), prog_tx.clone());

  let progress_handle = std::thread::spawn(move || {
    let mut nodes_decoded: usize = 0;
    let mut ways_decoded: usize = 0;
    let mut relations_decoded: usize = 0;
    let mut chunks_done: usize = 0;
    let mut nodes_written: usize = 0;
    let mut ways_written: usize = 0;
    let mut relations_written: usize = 0;
    let mut bytes_flushed: usize = 0;
    let mut flushes_done: usize = 0;
    let mut worker_states: Vec<worker_progress> = (0..worker_threads)
      .map(|i| worker_progress {
        thread_id: i as usize,
        chunks_processed: 0,
        node_count: 0,
        way_count: 0,
        relation_count: 0,
      })
      .collect();
    for event in prog_rx.into_iter() {
      match event {
        prog_event::decoded(counts) => {
          nodes_decoded += counts.nodes;
          ways_decoded += counts.ways;
          relations_decoded += counts.relations;
          chunks_done += 1;
          let w = &mut worker_states[counts.thread_id];
          w.node_count += counts.nodes;
          w.way_count += counts.ways;
          w.relation_count += counts.relations;
          w.chunks_processed += 1;
        }
        prog_event::flushed(counts) => {
          nodes_written += counts.nodes;
          ways_written += counts.ways;
          relations_written += counts.relations;
          bytes_flushed += counts.bytes;
          flushes_done += 1;
        }
      }
      on_progress(progress {
        total_chunks,
        chunks_done,
        node_count: nodes_decoded,
        way_count: ways_decoded,
        relation_count: relations_decoded,
        nodes_written,
        ways_written,
        relations_written,
        bytes_flushed,
        flushes_done,
        workers: worker_states
          .iter()
          .map(|w| worker_progress {
            thread_id: w.thread_id,
            chunks_processed: w.chunks_processed,
            node_count: w.node_count,
            way_count: w.way_count,
            relation_count: w.relation_count,
          })
          .collect(),
      });
    }
    (nodes_decoded, ways_decoded, relations_decoded)
  });

  // decoders: empurram rows direto no write_buf compartilhado
  let mut handles = Vec::new();
  for thread_id in 0..worker_threads as usize {
    handles.push(decode_thread(
      read_q.clone(),
      write_buf.clone(),
      opts.clone(),
      thread_id,
      prog_tx.clone(),
    ));
  }
  drop(prog_tx);

  // leitor: le blobs do pbf e injeta na fila ate encher
  let reader = reader_thread(pbf.to_string(), chunks, read_q.clone(), queue_cap);

  reader.join().unwrap();
  for h in handles {
    h.join().unwrap();
  }
  // sinalizar fim pro writer: nao vem mais row nenhum
  {
    let mut state = write_buf.inner.lock().unwrap();
    state.decoders_done = true;
  }
  write_buf.has_work.notify_all();
  writer.join().unwrap();
  progress_handle.join().unwrap()
}
/////////////////////////////////////////////////////////////////////////////////
fn read_blob_bytes(
  file: &mut fs::File,
  chunk: &crate::database::osm_pbf_blob_chunks::osm_pbf_blob_chunk,
) -> Vec<u8> {
  use io::Seek;
  file
    .seek(io::SeekFrom::Start(chunk.data_first_byte))
    .expect("failed to seek");
  let mut blob_data = vec![0u8; chunk.data_size as usize];
  file
    .read_exact(&mut blob_data)
    .expect("failed to read blob data");
  blob_data
}

fn decode_raw_blob(
  raw: &raw_blob,
  opts: &data_opts,
  encoder: &mut jsonb_encode::encoder,
) -> decoded_blob {
  let mut nodes = Vec::new();
  let mut ways = Vec::new();
  let mut relations = Vec::new();

  match raw.chunk.chunk_type {
    crate::database::osm_pbf_blob_chunks::chunk_type::data => {
      let output = decode_blob(&raw.data, opts);
      for n in output.nodes {
        let mut payload = Vec::with_capacity(128);
        encoder.encode_osm_node(&mut payload, &n);
        nodes.push(crate::database::osm_nodes::osm_node_row {
          id: n.id as u64,
          osm_pbf_chunk_id: raw.chunk.id,
          payload,
        });
      }
      for w in output.ways {
        let mut payload = Vec::with_capacity(128 + w.refs.len() * 4);
        encoder.encode_osm_way(&mut payload, &w);
        ways.push(crate::database::osm_ways::osm_way_row {
          id: w.id as u64,
          osm_pbf_chunk_id: raw.chunk.id,
          payload,
        });
      }
      for r in output.relations {
        let mut payload = Vec::with_capacity(128 + r.members.len() * 32);
        encoder.encode_osm_relation(&mut payload, &r);
        relations.push(crate::database::osm_relations::osm_relation_row {
          id: r.id as u64,
          osm_pbf_chunk_id: raw.chunk.id,
          payload,
        });
      }
    }
    crate::database::osm_pbf_blob_chunks::chunk_type::header => {}
  }

  decoded_blob {
    nodes,
    ways,
    relations,
  }
}

// struct decode_blob_output {
//     ways: Vec<>,
//     nodes: Vec<>,
//     relations: Vec<>,
// }

fn decode_blob(blob_data: &[u8], opts: &data_opts) -> decoded_blob_output {
  let blob = blob_msg::decode(blob_data).expect("failed to decode blob");
  let raw = super::decompress_blob(&blob);
  let block =
    primitive_block_msg::decode(raw.as_slice()).expect("failed to decode primitive block");

  let empty_st = string_table_msg::default();
  let st = block.stringtable.as_ref().unwrap_or(&empty_st);
  let strings: Vec<&str> = st
    .s
    .iter()
    .map(|b| std::str::from_utf8(b).unwrap_or(""))
    .collect();

  let granularity = block.granularity.unwrap_or(100) as i64;
  let lat_offset = block.lat_offset.unwrap_or(0);
  let lon_offset = block.lon_offset.unwrap_or(0);
  let date_granularity = block.date_granularity.unwrap_or(1000) as i64;

  let mut nodes = Vec::new();
  let mut ways = Vec::new();
  let mut relations = Vec::new();

  for group in &block.primitivegroup {
    if opts.include_ways {
      ways.extend(osm_ways::decode(&group.ways, &strings, opts));
    }
    if opts.include_relations {
      relations.extend(osm_relations::decode(&group.relations, &strings, opts));
    }
    if opts.include_nodes {
      nodes.extend(osm_nodes::decode_nodes(
        &group.nodes,
        &strings,
        granularity,
        lat_offset,
        lon_offset,
        date_granularity,
        opts,
      ));
      if let Some(dense) = &group.dense {
        nodes.extend(osm_nodes::decode_dense_nodes(
          dense,
          &strings,
          granularity,
          lat_offset,
          lon_offset,
          date_granularity,
          opts,
        ));
      }
    }
  }

  decoded_blob_output {
    nodes,
    ways,
    relations,
  }
}

/////////////////////////////////////////////////////////////////////////////////
fn tag_passes(k: &str, opts: &data_opts) -> bool {
  opts
    .tags_include
    .as_ref()
    .is_none_or(|l| l.iter().any(|i| i == k))
    && opts
      .tags_ignore
      .as_ref()
      .is_none_or(|l| !l.iter().any(|i| i == k))
}

fn filter_tags<'a>(
  strings: &[&'a str],
  keys: &[u32],
  vals: &[u32],
  opts: &data_opts,
) -> Vec<(&'a str, &'a str)> {
  keys
    .iter()
    .zip(vals.iter())
    .filter_map(|(&k_idx, &v_idx)| {
      let k = strings.get(k_idx as usize)?;
      let v = strings.get(v_idx as usize)?;
      if tag_passes(k, opts) {
        Some((*k, *v))
      } else {
        None
      }
    })
    .collect()
}

/////////////////////////////////////////////////////////////////////////////////
fn reader_thread(
  pbf: String,
  chunks: Vec<crate::database::osm_pbf_blob_chunks::osm_pbf_blob_chunk>,
  queue: Arc<raw_queue>,
  queue_cap: usize,
) -> std::thread::JoinHandle<()> {
  std::thread::spawn(move || {
    let mut file = fs::File::open(&pbf).expect("failed to open pbf file");
    for chunk in chunks {
      let data = read_blob_bytes(&mut file, &chunk);
      let raw = raw_blob { chunk, data };
      let mut state = queue.inner.lock().unwrap();
      while state.items.len() >= queue_cap {
        state = queue.not_full.wait(state).unwrap();
      }
      state.items.push_back(raw);
      drop(state);
      queue.not_empty.notify_one();
    }
    let mut state = queue.inner.lock().unwrap();
    state.reader_done = true;
    drop(state);
    queue.not_empty.notify_all();
  })
}

fn decode_thread(
  read_q: Arc<raw_queue>,
  write_buf: Arc<write_buffer>,
  opts: Arc<data_opts>,
  thread_id: usize,
  prog_tx: std::sync::mpsc::Sender<prog_event>,
) -> std::thread::JoinHandle<()> {
  std::thread::spawn(move || {
    let mut encoder = jsonb_encode::encoder::new();
    loop {
      let raw = {
        let mut state = read_q.inner.lock().unwrap();
        loop {
          if let Some(r) = state.items.pop_front() {
            read_q.not_full.notify_one();
            break Some(r);
          }
          if state.reader_done {
            break None;
          }
          state = read_q.not_empty.wait(state).unwrap();
        }
      };
      let Some(raw) = raw else { break };

      let blob = decode_raw_blob(&raw, &opts, &mut encoder);
      let bytes = decoded_blob_bytes(&blob);

      let counts = blob_counts {
        thread_id,
        nodes: blob.nodes.len(),
        ways: blob.ways.len(),
        relations: blob.relations.len(),
      };
      prog_tx.send(prog_event::decoded(counts)).ok();

      // push rows direto no buffer compartilhado; bloqueia se ultrapassar hard_limit
      let mut state = write_buf.inner.lock().unwrap();
      while state.bytes_current >= write_buf.hard_limit {
        state = write_buf.not_too_full.wait(state).unwrap();
      }
      state.current.nodes.extend(blob.nodes);
      state.current.ways.extend(blob.ways);
      state.current.relations.extend(blob.relations);
      state.bytes_current += bytes;
      let should_wake_writer = state.bytes_current >= write_buf.soft_threshold
        || state.current.row_count() >= write_buf.row_threshold;
      drop(state);
      if should_wake_writer {
        write_buf.has_work.notify_one();
      }
    }
  })
}

struct blob_counts {
  thread_id: usize,
  nodes: usize,
  ways: usize,
  relations: usize,
}

struct flush_counts {
  nodes: usize,
  ways: usize,
  relations: usize,
  bytes: usize,
}

enum prog_event {
  decoded(blob_counts),
  flushed(flush_counts),
}

fn writer_thread(
  conn: rusqlite::Connection,
  write_buf: Arc<write_buffer>,
  prog_tx: std::sync::mpsc::Sender<prog_event>,
) -> std::thread::JoinHandle<()> {
  std::thread::spawn(move || {
    loop {
      // pega o buffer cheio (ou aguarda ate ter trabalho); troca por um vazio
      // dentro do lock pra que decoders possam continuar empurrando no novo
      let (nodes_taken, ways_taken, relations_taken, bytes_taken) = {
        let mut state = write_buf.inner.lock().unwrap();
        loop {
          if state.bytes_current >= write_buf.soft_threshold
            || state.current.row_count() >= write_buf.row_threshold
          {
            break;
          }
          if state.decoders_done {
            // drena qualquer resto e sai
            if state.bytes_current == 0 {
              return;
            }
            break;
          }
          state = write_buf.has_work.wait(state).unwrap();
        }
        let was_full = state.bytes_current >= write_buf.hard_limit;
        let rows_before = state.current.row_count();
        let bytes_before = state.bytes_current;

        // drena ate flush_cap_rows do INICIO de cada deque (FIFO — ordem de
        // insercao). VecDeque::drain eh O(n_drenado): avanca o head do ring,
        // sem shift dos elementos remanescentes (que seria O(len) num Vec).
        // ordem de prioridade: nodes -> ways -> relations, pra esvaziar primeiro
        // a tabela mais volumosa que tipicamente domina a fila
        let cap = write_buf.flush_cap_rows;
        let take_nodes = cap.min(state.current.nodes.len());
        let mut nodes_taken: Vec<_> = Vec::with_capacity(take_nodes);
        nodes_taken.extend(state.current.nodes.drain(..take_nodes));
        let remaining = cap - take_nodes;
        let take_ways = remaining.min(state.current.ways.len());
        let mut ways_taken: Vec<_> = Vec::with_capacity(take_ways);
        ways_taken.extend(state.current.ways.drain(..take_ways));
        let remaining = remaining - take_ways;
        let take_relations = remaining.min(state.current.relations.len());
        let mut relations_taken: Vec<_> = Vec::with_capacity(take_relations);
        relations_taken.extend(state.current.relations.drain(..take_relations));

        // bytes_taken proporcional ao numero de rows drenados (aproximacao)
        let drained = take_nodes + take_ways + take_relations;
        let bytes_taken = (bytes_before * drained).checked_div(rows_before).unwrap_or(0);
        state.bytes_current = state.bytes_current.saturating_sub(bytes_taken);

        // libera decoders bloqueados so se o buffer caiu de fato abaixo de hard_limit
        if was_full && state.bytes_current < write_buf.hard_limit {
          write_buf.not_too_full.notify_all();
        }
        (nodes_taken, ways_taken, relations_taken, bytes_taken)
      };

      // flush fora do lock: decoders empurram no novo buffer em paralelo
      let nodes_n = nodes_taken.len();
      let ways_n = ways_taken.len();
      let relations_n = relations_taken.len();
      let tx = conn
        .unchecked_transaction()
        .expect("failed to begin transaction");
      crate::database::osm_nodes::insert_rows(&tx, &nodes_taken);
      crate::database::osm_ways::insert_rows(&tx, &ways_taken);
      crate::database::osm_relations::insert_rows(&tx, &relations_taken);
      tx.commit().expect("failed to commit");

      prog_tx
        .send(prog_event::flushed(flush_counts {
          nodes: nodes_n,
          ways: ways_n,
          relations: relations_n,
          bytes: bytes_taken,
        }))
        .ok();
    }
  })
}
#[cfg(test)]
#[path = "osm_data.test.rs"]
mod tests;
