use super::*;

use crate::extract::pbf_fixtures::{
  self, blob_compression, block_spec, data_chunk, header_chunk, node,
};

/////////////////////////////////////////////////////////////////////////////////
// harness
/////////////////////////////////////////////////////////////////////////////////

fn default_opts() -> data_opts {
  data_opts {
    include_nodes: true,
    include_ways: true,
    include_relations: true,
    ignore_info: true,
    tags_include: None,
    tags_ignore: None,
    // nunca use buffer_bytes < 2: soft_threshold = buffer_bytes * 4 / 5 viraria 0
    // e o writer entraria em laco de flush vazio sem nunca checar decoders_done
    buffer_bytes: 1_073_741_824,
  }
}

#[derive(Clone, Copy, Default)]
struct progress_snapshot {
  chunks_done: usize,
  nodes_written: usize,
  ways_written: usize,
  relations_written: usize,
  flushes_done: usize,
  worker_chunks: usize,
}

struct outcome {
  scene: pbf_fixtures::temp_scene,
  counts: (usize, usize, usize),
  progress: Vec<progress_snapshot>,
}

impl outcome {
  fn conn(&self) -> rusqlite::Connection {
    crate::database::open_readonly(&self.scene.db_path)
  }

  fn row_count(&self, table: &str) -> i64 {
    self
      .conn()
      .query_row(&format!("SELECT COUNT(*) FROM osm_data.{table}"), [], |r| {
        r.get(0)
      })
      .expect("failed to count rows")
  }

  fn payload(&self, table: &str, id: i64) -> serde_json::Value {
    let text: String = self
      .conn()
      .query_row(
        &format!("SELECT JSON(payload) FROM osm_data.{table} WHERE id = ?1"),
        rusqlite::params![id],
        |r| r.get(0),
      )
      .unwrap_or_else(|e| panic!("failed to read {table} id {id}: {e}"));
    serde_json::from_str(&text).expect("payload deve ser json valido")
  }

  fn last_progress(&self) -> progress_snapshot {
    self.progress.last().copied().unwrap_or_default()
  }
}

// indexa os chunks, roda o pipeline e devolve tudo o que os testes precisam observar
fn run_scene(tag: &str, chunks: &[Vec<u8>], opts: data_opts, threads: u8) -> outcome {
  let (scene, file_id) = pbf_fixtures::indexed_scene(tag, chunks);

  let conn = crate::database::open_write(&scene.db_path);
  let blob_chunks = crate::database::osm_pbf_blob_chunks::get_data_chunks(&conn, file_id);
  drop(conn);

  let seen = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
  let sink = seen.clone();

  let write_conn = crate::database::open_write(&scene.db_path);
  let counts = run(
    &scene.pbf_path,
    blob_chunks,
    write_conn,
    opts,
    &threads,
    move |p| {
      sink.lock().expect("progress mutex").push(progress_snapshot {
        chunks_done: p.chunks_done,
        nodes_written: p.nodes_written,
        ways_written: p.ways_written,
        relations_written: p.relations_written,
        flushes_done: p.flushes_done,
        worker_chunks: p.workers.iter().map(|w| w.chunks_processed).sum(),
      });
    },
  );

  let progress = seen.lock().expect("progress mutex").clone();
  outcome {
    scene,
    counts,
    progress,
  }
}

fn tiny_scene(tag: &str) -> outcome {
  run_scene(tag, &pbf_fixtures::tiny_pbf(), default_opts(), 1)
}

/////////////////////////////////////////////////////////////////////////////////
// 00 — pipeline ponta a ponta
/////////////////////////////////////////////////////////////////////////////////

// 00.00: node, way e relation sao decodificados e persistidos com payload jsonb legivel
#[test]
fn _00_00_persists_nodes_ways_and_relations() {
  let out = tiny_scene("od_00_00");

  assert_eq!(out.counts, (1, 1, 1));
  assert_eq!(out.row_count("osm_nodes"), 1);
  assert_eq!(out.row_count("osm_ways"), 1);
  assert_eq!(out.row_count("osm_relations"), 1);

  let n = out.payload("osm_nodes", 1);
  assert!((n["lat"].as_f64().expect("lat") - 38.7).abs() < 1e-7);
  assert!((n["lon"].as_f64().expect("lon") - -9.1).abs() < 1e-7);
  assert_eq!(n["tags"]["name"], "Marco Zero");

  let w = out.payload("osm_ways", 100);
  assert_eq!(w["refs"], serde_json::json!([1, 2, 3]));
  assert_eq!(w["tags"]["highway"], "residential");

  let r = out.payload("osm_relations", 200);
  assert_eq!(r["tags"]["admin_level"], "8");
  assert_eq!(r["members"][0]["type"], "w");
  assert_eq!(r["members"][0]["id"], 100);
  assert_eq!(r["members"][0]["role"], "outer");
}

// 00.01: lista de chunks vazia encerra o writer sem escrever nada
#[test]
fn _00_01_returns_zero_counts_for_empty_chunk_list() {
  let scene = pbf_fixtures::temp_scene("od_00_01");
  pbf_fixtures::write_pbf(&scene.pbf_path, &[]);

  let write_conn = crate::database::open_write(&scene.db_path);
  let counts = run(
    &scene.pbf_path,
    Vec::new(),
    write_conn,
    default_opts(),
    &1,
    |_| {},
  );

  assert_eq!(counts, (0, 0, 0));
}

// 00.02: chunk de header aparece na lista mas nao gera linha nenhuma
#[test]
fn _00_02_skips_header_chunks_without_emitting_rows() {
  let out = run_scene("od_00_02", &[header_chunk()], default_opts(), 1);

  assert_eq!(out.counts, (0, 0, 0));
  assert_eq!(out.row_count("osm_nodes"), 0);
  assert_eq!(out.row_count("osm_ways"), 0);
  assert_eq!(out.row_count("osm_relations"), 0);
}

// 00.03: os tres flags include_* filtram cada tipo de elemento independentemente
#[test]
fn _00_03_honors_include_nodes_ways_and_relations_flags() {
  let only_nodes = run_scene(
    "od_00_03_n",
    &pbf_fixtures::tiny_pbf(),
    data_opts {
      include_ways: false,
      include_relations: false,
      ..default_opts()
    },
    1,
  );
  assert_eq!(only_nodes.counts, (1, 0, 0));

  let only_ways = run_scene(
    "od_00_03_w",
    &pbf_fixtures::tiny_pbf(),
    data_opts {
      include_nodes: false,
      include_relations: false,
      ..default_opts()
    },
    1,
  );
  assert_eq!(only_ways.counts, (0, 1, 0));

  let only_relations = run_scene(
    "od_00_03_r",
    &pbf_fixtures::tiny_pbf(),
    data_opts {
      include_nodes: false,
      include_ways: false,
      ..default_opts()
    },
    1,
  );
  assert_eq!(only_relations.counts, (0, 0, 1));
}

// 00.04: nodes planos e dense nodes coexistindo no mesmo grupo sao ambos decodificados
#[test]
fn _00_04_decodes_plain_nodes_alongside_dense_nodes() {
  let chunk = data_chunk(
    &block_spec {
      dense: vec![node(1, 38.7, -9.1, &[("name", "Denso")])],
      plain: vec![node(2, 38.6, -9.2, &[("name", "Plano")])],
      ..Default::default()
    },
    blob_compression::zlib,
  );
  let out = run_scene("od_00_04", &[chunk], default_opts(), 1);

  assert_eq!(out.counts.0, 2);
  assert_eq!(out.payload("osm_nodes", 1)["tags"]["name"], "Denso");
  assert_eq!(out.payload("osm_nodes", 2)["tags"]["name"], "Plano");
}

// 00.05: granularity e offsets nao-default sao aplicados na conversao de coordenadas
#[test]
fn _00_05_applies_granularity_and_lat_lon_offsets() {
  let chunk = data_chunk(
    &block_spec {
      dense: vec![node(1, 12.5, -3.25, &[])],
      granularity: 1_000,
      lat_offset: 5_000_000,
      lon_offset: -7_000_000,
      ..Default::default()
    },
    blob_compression::zlib,
  );
  let out = run_scene("od_00_05", &[chunk], default_opts(), 1);

  let n = out.payload("osm_nodes", 1);
  assert!((n["lat"].as_f64().expect("lat") - 12.5).abs() < 1e-6);
  assert!((n["lon"].as_f64().expect("lon") - -3.25).abs() < 1e-6);
}

// 00.06: bloco que omite granularity/offsets cai nos defaults (100 / 0 / 0)
#[test]
fn _00_06_falls_back_to_defaults_when_block_omits_options() {
  let chunk = data_chunk(
    &block_spec {
      dense: vec![node(1, 38.7, -9.1, &[])],
      emit_block_options: false,
      ..Default::default()
    },
    blob_compression::zlib,
  );
  let out = run_scene("od_00_06", &[chunk], default_opts(), 1);

  let n = out.payload("osm_nodes", 1);
  assert!(
    (n["lat"].as_f64().expect("lat") - 38.7).abs() < 1e-7,
    "granularity default de 100 deve reproduzir a coordenada original"
  );
}

// 00.07: tags_include mantem apenas as chaves listadas
#[test]
fn _00_07_filters_tags_with_tags_include() {
  let chunk = data_chunk(
    &block_spec {
      dense: vec![node(1, 38.7, -9.1, &[("name", "Alfa"), ("amenity", "cafe")])],
      ..Default::default()
    },
    blob_compression::zlib,
  );
  let out = run_scene(
    "od_00_07",
    &[chunk],
    data_opts {
      tags_include: Some(vec!["name".to_string()]),
      ..default_opts()
    },
    1,
  );

  let tags = &out.payload("osm_nodes", 1)["tags"];
  assert_eq!(tags["name"], "Alfa");
  assert!(tags.get("amenity").is_none(), "amenity deveria ser filtrada");
}

// 00.08: tags_ignore descarta apenas as chaves listadas
#[test]
fn _00_08_filters_tags_with_tags_ignore() {
  let chunk = data_chunk(
    &block_spec {
      dense: vec![node(1, 38.7, -9.1, &[("name", "Alfa"), ("amenity", "cafe")])],
      ..Default::default()
    },
    blob_compression::zlib,
  );
  let out = run_scene(
    "od_00_08",
    &[chunk],
    data_opts {
      tags_ignore: Some(vec!["amenity".to_string()]),
      ..default_opts()
    },
    1,
  );

  let tags = &out.payload("osm_nodes", 1)["tags"];
  assert_eq!(tags["name"], "Alfa");
  assert!(tags.get("amenity").is_none());
}

// 00.09: blob sem compressao percorre o pipeline igual ao blob zlib
#[test]
fn _00_09_decodes_uncompressed_data_blob() {
  let chunk = data_chunk(
    &block_spec {
      dense: vec![node(1, 38.7, -9.1, &[("name", "Cru")])],
      ..Default::default()
    },
    blob_compression::raw,
  );
  let out = run_scene("od_00_09", &[chunk], default_opts(), 1);

  assert_eq!(out.counts.0, 1);
  assert_eq!(out.payload("osm_nodes", 1)["tags"]["name"], "Cru");
}

// 00.10: o agregador de progresso soma decodificacao e flush, e atribui os chunks
// aos workers
#[test]
fn _00_10_reports_progress_for_decode_and_flush() {
  let out = tiny_scene("od_00_10");
  let last = out.last_progress();

  assert!(!out.progress.is_empty(), "deve haver eventos de progresso");
  assert_eq!(last.chunks_done, 4, "3 blobs de dados + 1 header");
  assert_eq!(last.worker_chunks, 4);
  assert_eq!(last.nodes_written, 1);
  assert_eq!(last.ways_written, 1);
  assert_eq!(last.relations_written, 1);
  assert!(last.flushes_done >= 1);
}

// 00.11: com varios decoders o resultado continua exato
#[test]
fn _00_11_processes_every_chunk_with_multiple_decoder_threads() {
  let mut chunks = vec![header_chunk()];
  for i in 0..24i64 {
    chunks.push(data_chunk(
      &block_spec {
        dense: vec![node(i + 1, 38.0 + i as f64 / 100.0, -9.0, &[])],
        ..Default::default()
      },
      blob_compression::zlib,
    ));
  }

  let out = run_scene("od_00_11", &chunks, default_opts(), 4);

  assert_eq!(out.counts, (24, 0, 0));
  assert_eq!(out.row_count("osm_nodes"), 24);
}

// 00.12: buffer pequeno obriga o writer a fazer varios flushes
#[test]
fn _00_12_flushes_repeatedly_with_a_small_buffer() {
  let mut chunks = Vec::new();
  for i in 0..30i64 {
    chunks.push(data_chunk(
      &block_spec {
        dense: vec![node(
          i + 1,
          38.0 + i as f64 / 100.0,
          -9.0,
          &[("name", "no com um nome razoavelmente longo para ocupar bytes")],
        )],
        ..Default::default()
      },
      blob_compression::zlib,
    ));
  }

  let out = run_scene(
    "od_00_12",
    &chunks,
    data_opts {
      buffer_bytes: 512,
      ..default_opts()
    },
    2,
  );

  assert_eq!(out.counts, (30, 0, 0));
  assert_eq!(out.row_count("osm_nodes"), 30);
  assert!(
    out.last_progress().flushes_done > 1,
    "buffer pequeno deveria gerar mais de um flush"
  );
}

// 00.13: rodar duas vezes nao duplica linhas (id e chave primaria)
#[test]
fn _00_13_second_run_does_not_duplicate_rows() {
  let out = tiny_scene("od_00_13");

  let conn = crate::database::open_write(&out.scene.db_path);
  let file_id = crate::database::osm_pbf_files::ensure_by_file_path(&conn, &out.scene.pbf_path);
  let blob_chunks = crate::database::osm_pbf_blob_chunks::get_data_chunks(&conn, file_id);
  drop(conn);

  let write_conn = crate::database::open_write(&out.scene.db_path);
  run(
    &out.scene.pbf_path,
    blob_chunks,
    write_conn,
    default_opts(),
    &1,
    |_| {},
  );

  assert_eq!(out.row_count("osm_nodes"), 1);
  assert_eq!(out.row_count("osm_ways"), 1);
  assert_eq!(out.row_count("osm_relations"), 1);
}

/////////////////////////////////////////////////////////////////////////////////
// 01 — funcoes de thread chamadas diretamente
//
// os caminhos de condvar sao inalcancaveis de forma deterministica atraves de
// `run`, porque dependem de qual thread ganha a corrida. chamando as funcoes
// diretamente com filas montadas a mao cada bloqueio vira deterministico.
/////////////////////////////////////////////////////////////////////////////////

fn empty_queue(reader_done: bool) -> Arc<raw_queue> {
  Arc::new(raw_queue {
    inner: Mutex::new(raw_queue_state {
      items: VecDeque::new(),
      reader_done,
    }),
    not_empty: Condvar::new(),
    not_full: Condvar::new(),
  })
}

fn dummy_chunk(id: u32) -> crate::database::osm_pbf_blob_chunks::osm_pbf_blob_chunk {
  crate::database::osm_pbf_blob_chunks::osm_pbf_blob_chunk {
    id,
    file_id: 1,
    first_byte: 0,
    chunk_size: 0,
    data_first_byte: 0,
    data_size: 0,
    chunk_type: crate::database::osm_pbf_blob_chunks::chunk_type::data,
  }
}

// blob de dados cru, pronto para ser decodificado sem passar por arquivo
fn raw_data_blob(id: i64) -> raw_blob {
  let payload = pbf_fixtures::data_blob(
    &block_spec {
      dense: vec![node(id, 38.7, -9.1, &[("name", "no")])],
      ..Default::default()
    },
    blob_compression::zlib,
  );
  raw_blob {
    chunk: dummy_chunk(id as u32),
    data: payload,
  }
}

fn test_write_buffer(hard_limit: usize, soft_threshold: usize) -> Arc<write_buffer> {
  Arc::new(write_buffer {
    inner: Mutex::new(write_buffer_state {
      current: buffer_data::default(),
      bytes_current: 0,
      decoders_done: false,
    }),
    has_work: Condvar::new(),
    not_too_full: Condvar::new(),
    soft_threshold,
    hard_limit,
    row_threshold: usize::MAX,
    flush_cap_rows: usize::MAX,
  })
}

// 01.00: fila cheia bloqueia o reader ate alguem consumir
#[test]
fn _01_00_reader_blocks_while_the_queue_is_full() {
  let scene = pbf_fixtures::temp_scene("od_01_00");
  let chunks = vec![
    data_chunk(
      &block_spec {
        dense: vec![node(1, 38.7, -9.1, &[])],
        ..Default::default()
      },
      blob_compression::zlib,
    ),
    data_chunk(
      &block_spec {
        dense: vec![node(2, 38.8, -9.2, &[])],
        ..Default::default()
      },
      blob_compression::zlib,
    ),
  ];
  pbf_fixtures::write_pbf(&scene.pbf_path, &chunks);

  let (offsets, sizes) = chunk_offsets(&chunks);
  let blob_chunks: Vec<_> = offsets
    .iter()
    .zip(sizes.iter())
    .enumerate()
    .map(|(i, (&first, &(data_first, data_size)))| {
      crate::database::osm_pbf_blob_chunks::osm_pbf_blob_chunk {
        id: i as u32 + 1,
        file_id: 1,
        first_byte: first,
        chunk_size: 0,
        data_first_byte: data_first,
        data_size,
        chunk_type: crate::database::osm_pbf_blob_chunks::chunk_type::data,
      }
    })
    .collect();

  // fila ja cheia (cap 1) antes do reader comecar: o primeiro push bloqueia
  let queue = Arc::new(raw_queue {
    inner: Mutex::new(raw_queue_state {
      items: VecDeque::from(vec![raw_data_blob(99)]),
      reader_done: false,
    }),
    not_empty: Condvar::new(),
    not_full: Condvar::new(),
  });

  let handle = reader_thread(scene.pbf_path.clone(), blob_chunks, queue.clone(), 1);

  // drena continuamente ate o reader sinalizar que terminou
  loop {
    {
      let mut state = queue.inner.lock().expect("queue mutex");
      if state.reader_done {
        break;
      }
      state.items.clear();
    }
    queue.not_full.notify_all();
    std::thread::sleep(std::time::Duration::from_millis(1));
  }

  handle.join().expect("reader nao deve entrar em panico");
}

// devolve (first_byte de cada chunk, (data_first_byte, data_size))
fn chunk_offsets(chunks: &[Vec<u8>]) -> (Vec<u64>, Vec<(u64, u64)>) {
  let mut firsts = Vec::new();
  let mut data = Vec::new();
  let mut offset = 0u64;
  for chunk in chunks {
    let header_len = u32::from_be_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]) as u64;
    firsts.push(offset);
    data.push((
      offset + 4 + header_len,
      chunk.len() as u64 - 4 - header_len,
    ));
    offset += chunk.len() as u64;
  }
  (firsts, data)
}

// 01.01: decoder que encontra a fila vazia aguarda ate o reader sinalizar o fim
#[test]
fn _01_01_decoder_waits_while_the_queue_is_empty() {
  let queue = empty_queue(false);
  queue
    .inner
    .lock()
    .expect("queue mutex")
    .items
    .push_back(raw_data_blob(1));

  let buffer = test_write_buffer(usize::MAX, usize::MAX);
  let opts = Arc::new(default_opts());
  let (tx, rx) = std::sync::mpsc::channel::<prog_event>();

  // dois decoders para um unico item: o perdedor obrigatoriamente estaciona no wait
  let a = decode_thread(queue.clone(), buffer.clone(), opts.clone(), 0, tx.clone());
  let b = decode_thread(queue.clone(), buffer.clone(), opts, 1, tx);

  // espera o item ser consumido e da tempo do perdedor estacionar
  while !queue.inner.lock().expect("queue mutex").items.is_empty() {
    std::thread::sleep(std::time::Duration::from_millis(1));
  }
  std::thread::sleep(std::time::Duration::from_millis(50));

  queue.inner.lock().expect("queue mutex").reader_done = true;
  queue.not_empty.notify_all();

  a.join().expect("decoder a");
  b.join().expect("decoder b");

  let events: Vec<_> = rx.into_iter().collect();
  assert_eq!(events.len(), 1, "apenas um decoder processou o item");
}

// 01.02: decoder bloqueia quando o buffer ultrapassa hard_limit e so segue
// depois que alguem drena
#[test]
fn _01_02_decoder_blocks_when_the_buffer_exceeds_hard_limit() {
  let queue = empty_queue(true);
  queue
    .inner
    .lock()
    .expect("queue mutex")
    .items
    .push_back(raw_data_blob(1));

  // hard_limit 1 com o buffer ja "cheio" e sem writer para drenar
  let buffer = test_write_buffer(1, 1);
  buffer.inner.lock().expect("buffer mutex").bytes_current = 10_000;

  let (tx, _rx) = std::sync::mpsc::channel::<prog_event>();
  let handle = decode_thread(queue, buffer.clone(), Arc::new(default_opts()), 0, tx);

  // deixa o decoder chegar no wait antes de liberar
  std::thread::sleep(std::time::Duration::from_millis(50));
  buffer.inner.lock().expect("buffer mutex").bytes_current = 0;
  buffer.not_too_full.notify_all();

  handle.join().expect("decoder nao deve entrar em panico");

  let state = buffer.inner.lock().expect("buffer mutex");
  assert_eq!(
    state.current.nodes.len(),
    1,
    "o decoder deve empurrar a linha depois de destravar"
  );
}

// 01.03: ao drenar um buffer que estava cheio, o writer avisa os decoders parados
#[test]
fn _01_03_writer_notifies_decoders_after_draining_a_full_buffer() {
  let scene = pbf_fixtures::temp_scene("od_01_03");
  let conn = crate::database::open_write(&scene.db_path);

  let buffer = test_write_buffer(1, 1);
  {
    let mut state = buffer.inner.lock().expect("buffer mutex");
    state.current.nodes.push_back(crate::database::osm_nodes::osm_node_row {
      id: 1,
      osm_pbf_chunk_id: 1,
      payload: vec![0x0c],
    });
    state.bytes_current = 10_000;
    state.decoders_done = true;
  }

  let (tx, rx) = std::sync::mpsc::channel::<prog_event>();
  writer_thread(conn, buffer.clone(), tx)
    .join()
    .expect("writer nao deve entrar em panico");

  let state = buffer.inner.lock().expect("buffer mutex");
  assert_eq!(state.bytes_current, 0, "o buffer deve terminar drenado");
  assert!(state.current.nodes.is_empty());

  let flushed = rx
    .into_iter()
    .filter(|e| matches!(e, prog_event::flushed(_)))
    .count();
  assert_eq!(flushed, 1, "deve reportar exatamente um flush");
}

// 01.04: writer sem trabalho aguarda ate os decoders sinalizarem o fim
#[test]
fn _01_04_writer_waits_until_decoders_signal_completion() {
  let scene = pbf_fixtures::temp_scene("od_01_04");
  let conn = crate::database::open_write(&scene.db_path);

  let buffer = test_write_buffer(usize::MAX, usize::MAX);
  let (tx, rx) = std::sync::mpsc::channel::<prog_event>();
  let handle = writer_thread(conn, buffer.clone(), tx);

  // buffer vazio e decoders_done falso: o writer estaciona no has_work
  std::thread::sleep(std::time::Duration::from_millis(50));
  buffer.inner.lock().expect("buffer mutex").decoders_done = true;
  buffer.has_work.notify_all();

  handle.join().expect("writer nao deve entrar em panico");
  assert_eq!(
    rx.into_iter().count(),
    0,
    "sem linhas no buffer nao ha flush a reportar"
  );
}

// 01.05: reader falha ao abrir um pbf inexistente
#[test]
fn _01_05_reader_thread_fails_for_a_missing_pbf() {
  let handle = reader_thread(
    "/caminho/que/nao/existe.osm.pbf".to_string(),
    vec![dummy_chunk(1)],
    empty_queue(false),
    4,
  );

  assert!(handle.join().is_err(), "abrir pbf inexistente deve falhar");
}

// 01.06: chunk que aponta para alem do fim do arquivo nao pode ser lido
#[test]
#[should_panic(expected = "failed to read blob data")]
fn _01_06_read_blob_bytes_fails_when_chunk_runs_past_eof() {
  let scene = pbf_fixtures::temp_scene("od_01_06");
  std::fs::write(&scene.pbf_path, b"curto").expect("failed to write fixture");

  let mut file = fs::File::open(&scene.pbf_path).expect("failed to open fixture");
  let mut chunk = dummy_chunk(1);
  chunk.data_first_byte = 0;
  chunk.data_size = 4096;

  read_blob_bytes(&mut file, &chunk);
}

/////////////////////////////////////////////////////////////////////////////////
// 02 — auxiliares puros
/////////////////////////////////////////////////////////////////////////////////

// 02.00: o tamanho estimado soma a capacidade dos vetores e dos payloads
#[test]
fn _02_00_decoded_blob_bytes_sums_stack_and_heap() {
  let empty = decoded_blob {
    nodes: Vec::new(),
    ways: Vec::new(),
    relations: Vec::new(),
  };
  assert_eq!(decoded_blob_bytes(&empty), 0);

  let filled = decoded_blob {
    nodes: vec![crate::database::osm_nodes::osm_node_row {
      id: 1,
      osm_pbf_chunk_id: 1,
      payload: vec![0u8; 64],
    }],
    ways: Vec::new(),
    relations: Vec::new(),
  };
  assert!(
    decoded_blob_bytes(&filled) >= 64,
    "deve contar ao menos o payload no heap"
  );
}

// 02.01: row_count soma as tres filas do buffer
#[test]
fn _02_01_buffer_row_count_sums_every_queue() {
  let mut data = buffer_data::default();
  assert_eq!(data.row_count(), 0);

  data.nodes.push_back(crate::database::osm_nodes::osm_node_row {
    id: 1,
    osm_pbf_chunk_id: 1,
    payload: Vec::new(),
  });
  data.ways.push_back(crate::database::osm_ways::osm_way_row {
    id: 2,
    osm_pbf_chunk_id: 1,
    payload: Vec::new(),
  });
  data
    .relations
    .push_back(crate::database::osm_relations::osm_relation_row {
      id: 3,
      osm_pbf_chunk_id: 1,
      payload: Vec::new(),
    });

  assert_eq!(data.row_count(), 3);
}

// 02.02: tag_passes aplica include e ignore de forma independente
#[test]
fn _02_02_tag_passes_applies_include_and_ignore_lists() {
  let none = default_opts();
  assert!(tag_passes("name", &none), "sem listas tudo passa");

  let include = data_opts {
    tags_include: Some(vec!["name".to_string()]),
    ..default_opts()
  };
  assert!(tag_passes("name", &include));
  assert!(!tag_passes("amenity", &include));

  let ignore = data_opts {
    tags_ignore: Some(vec!["amenity".to_string()]),
    ..default_opts()
  };
  assert!(tag_passes("name", &ignore));
  assert!(!tag_passes("amenity", &ignore));
}

// 02.03: indices fora da tabela de strings sao descartados em vez de causar panico
#[test]
fn _02_03_filter_tags_drops_out_of_range_string_indices() {
  let strings = ["", "name", "Alfa"];
  let opts = default_opts();

  let ok = filter_tags(&strings, &[1], &[2], &opts);
  assert_eq!(ok, vec![("name", "Alfa")]);

  assert!(
    filter_tags(&strings, &[99], &[2], &opts).is_empty(),
    "chave fora do range deve ser descartada"
  );
  assert!(
    filter_tags(&strings, &[1], &[99], &opts).is_empty(),
    "valor fora do range deve ser descartado"
  );
}

// 02.04: bloco sem stringtable usa uma tabela vazia em vez de falhar
#[test]
fn _02_04_decode_blob_uses_an_empty_string_table_when_absent() {
  use prost::Message;

  let block = pbf_fixtures::primitive_block_wire {
    stringtable: None,
    primitivegroup: vec![pbf_fixtures::primitive_group_wire {
      nodes: Vec::new(),
      dense: Some(pbf_fixtures::dense_nodes_wire {
        id: vec![1],
        denseinfo: None,
        lat: vec![387_000_000],
        lon: vec![-91_000_000],
        keys_vals: vec![0],
      }),
      ways: Vec::new(),
      relations: Vec::new(),
    }],
    granularity: None,
    date_granularity: None,
    lat_offset: None,
    lon_offset: None,
  };
  let blob = pbf_fixtures::make_blob(&block.encode_to_vec(), blob_compression::raw);

  let out = decode_blob(&blob, &default_opts());
  assert_eq!(out.nodes.len(), 1);
  assert!(out.nodes[0].tags.is_empty());
}

// 02.05: strings invalidas em utf-8 viram "" em vez de derrubar o decode
#[test]
fn _02_05_decode_blob_tolerates_invalid_utf8_in_string_table() {
  use prost::Message;

  let block = pbf_fixtures::primitive_block_wire {
    stringtable: Some(pbf_fixtures::string_table_wire {
      // indice 1 e uma sequencia utf-8 invalida
      s: vec![Vec::new(), vec![0xff, 0xfe], b"Alfa".to_vec()],
    }),
    primitivegroup: vec![pbf_fixtures::primitive_group_wire {
      nodes: Vec::new(),
      dense: Some(pbf_fixtures::dense_nodes_wire {
        id: vec![1],
        denseinfo: None,
        lat: vec![387_000_000],
        lon: vec![-91_000_000],
        keys_vals: vec![1, 2, 0],
      }),
      ways: Vec::new(),
      relations: Vec::new(),
    }],
    granularity: None,
    date_granularity: None,
    lat_offset: None,
    lon_offset: None,
  };
  let blob = pbf_fixtures::make_blob(&block.encode_to_vec(), blob_compression::raw);

  let out = decode_blob(&blob, &default_opts());
  assert_eq!(out.nodes.len(), 1);
  assert_eq!(
    out.nodes[0].tags.get(""),
    Some(&"Alfa".to_string()),
    "a chave invalida vira string vazia"
  );
}

// 02.06: tag reprovada pelos filtros e descartada por filter_tags
#[test]
fn _02_06_filter_tags_drops_tags_rejected_by_the_filters() {
  let strings = ["", "name", "Alfa", "amenity", "cafe"];
  let opts = data_opts {
    tags_ignore: Some(vec!["amenity".to_string()]),
    ..default_opts()
  };

  let kept = filter_tags(&strings, &[1, 3], &[2, 4], &opts);
  assert_eq!(
    kept,
    vec![("name", "Alfa")],
    "apenas a tag aprovada deve sobreviver"
  );
}
