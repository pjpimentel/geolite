// #[cfg(test)]
// mod tests {
//     use super::*;

//     // primitiveblock com 1 densenode: id=1, lat=0, lon=0, sem tags
//     //
//     //   stringtable {}                       → 0x0a 0x00
//     //   primitivegroup {                     → 0x12 0x0b
//     //     densenodes {                       → 0x12 0x09
//     //       id   = [1] (sint64 packed)       → 0x0a 0x01 0x02  (zigzag(1)=2)
//     //       lat  = [0] (sint64 packed)       → 0x42 0x01 0x00
//     //       lon  = [0] (sint64 packed)       → 0x4a 0x01 0x00
//     //     }
//     //   }
//     //
//     // com granularity=100 (default): lat=0.0, lon=0.0
//     const PRIMITIVE_BLOCK_1_NODE: &[u8] = &[
//         0x0a, 0x00,
//         0x12, 0x0b,
//           0x12, 0x09,
//             0x0a, 0x01, 0x02,
//             0x42, 0x01, 0x00,
//             0x4a, 0x01, 0x00,
//     ];

//     // primitiveblock com 1 way: id=42, refs=[10,20] (delta-encoded), sem tags
//     //
//     //   stringtable {}                       → 0x0a 0x00
//     //   primitivegroup {                     → 0x12 0x08
//     //     way {                              → 0x1a 0x06
//     //       id   = 42                        → 0x08 0x2a
//     //       refs = [10,10] sint64 packed     → 0x42 0x02 0x14 0x14  (zigzag(10)=20=0x14)
//     //     }
//     //   }
//     //
//     // o decode acumula deltas: 0+10=10, 10+10=20 → refs=[10,20]
//     const PRIMITIVE_BLOCK_1_WAY: &[u8] = &[
//         0x0a, 0x00,
//         0x12, 0x08,
//           0x1a, 0x06,
//             0x08, 0x2a,
//             0x42, 0x02, 0x14, 0x14,
//     ];

//     // primitiveblock com 1 relation: id=7, 1 member (way, memid=5, role="outer"), sem tags
//     //
//     //   stringtable {                        → 0x0a 0x09
//     //     s[0] = ""                          → 0x0a 0x00
//     //     s[1] = "outer"                     → 0x0a 0x05 0x6f 0x75 0x74 0x65 0x72
//     //   }
//     //   primitivegroup {                     → 0x12 0x0d
//     //     relation {                         → 0x22 0x0b
//     //       id        = 7                    → 0x08 0x07
//     //       roles_sid = [1] int32 packed     → 0x42 0x01 0x01
//     //       memids    = [5] sint64 packed    → 0x4a 0x01 0x0a  (zigzag(5)=10)
//     //       types     = [1=way] packed       → 0x52 0x01 0x01
//     //     }
//     //   }
//     //
//     // decode: memid=0+5=5, role=stringtable[1]="outer", type=way(1)
//     const PRIMITIVE_BLOCK_1_RELATION: &[u8] = &[
//         0x0a, 0x09,
//           0x0a, 0x00,
//           0x0a, 0x05, 0x6f, 0x75, 0x74, 0x65, 0x72,
//         0x12, 0x0d,
//           0x22, 0x0b,
//             0x08, 0x07,
//             0x42, 0x01, 0x01,
//             0x4a, 0x01, 0x0a,
//             0x52, 0x01, 0x01,
//     ];

//     // primitiveblock com 1 densenode: id=1, lat=0, lon=0, tags={name:"Test", amenity:"cafe"}
//     //
//     //   stringtable {                        → 0x0a 0x1d
//     //     s[0] = ""                          → 0x0a 0x00
//     //     s[1] = "name"                      → 0x0a 0x04 "name"
//     //     s[2] = "Test"                      → 0x0a 0x04 "Test"
//     //     s[3] = "amenity"                   → 0x0a 0x07 "amenity"
//     //     s[4] = "cafe"                      → 0x0a 0x04 "cafe"
//     //   }
//     //   primitivegroup {                     → 0x12 0x12
//     //     densenodes {                       → 0x12 0x10
//     //       id        = [1]                  → 0x0a 0x01 0x02
//     //       lat       = [0]                  → 0x42 0x01 0x00
//     //       lon       = [0]                  → 0x4a 0x01 0x00
//     //       keys_vals = [1,2,3,4,0]          → 0x52 0x05 01 02 03 04 00
//     //     }
//     //   }
//     const PRIMITIVE_BLOCK_1_NODE_WITH_TAGS: &[u8] = &[
//         0x0a, 0x1d,
//           0x0a, 0x00,
//           0x0a, 0x04, 0x6e, 0x61, 0x6d, 0x65,
//           0x0a, 0x04, 0x54, 0x65, 0x73, 0x74,
//           0x0a, 0x07, 0x61, 0x6d, 0x65, 0x6e, 0x69, 0x74, 0x79,
//           0x0a, 0x04, 0x63, 0x61, 0x66, 0x65,
//         0x12, 0x12,
//           0x12, 0x10,
//             0x0a, 0x01, 0x02,
//             0x42, 0x01, 0x00,
//             0x4a, 0x01, 0x00,
//             0x52, 0x05, 0x01, 0x02, 0x03, 0x04, 0x00,
//     ];

//     fn encode_varint(buf: &mut Vec<u8>, mut v: u64) {
//         loop {
//             let mut b = (v & 0x7f) as u8;
//             v >>= 7;
//             if v != 0 {
//                 b |= 0x80;
//             }
//             buf.push(b);
//             if v == 0 {
//                 break;
//             }
//         }
//     }

//     // monta um blob osm válido com field 3 (zlib_data) contendo o primitiveblock comprimido.
//     // estrutura: [0x1a, varint(len_comprimido), ...zlib_data]
//     fn make_osm_data_blob(pb_bytes: &[u8]) -> Vec<u8> {
//         use flate2::{write::ZlibEncoder, Compression};
//         use std::io::Write;
//         let mut enc = ZlibEncoder::new(Vec::new(), Compression::default());
//         enc.write_all(pb_bytes).unwrap();
//         let compressed = enc.finish().unwrap();
//         let mut blob = vec![0x1a]; // field 3, wire type 2
//         encode_varint(&mut blob, compressed.len() as u64);
//         blob.extend_from_slice(&compressed);
//         blob
//     }

//     // monta um pbf válido com um único blob osm.
//     // estrutura: [be_u32(header_len), blobheader_bytes, blob_bytes]
//     // blobheader: field 1 (type="OSMData") + field 3 (datasize=blob_len)
//     fn make_pbf_file(pb_bytes: &[u8]) -> Vec<u8> {
//         let blob = make_osm_data_blob(pb_bytes);
//         let t = b"OSMData";
//         let mut hdr = vec![0x0a, t.len() as u8];
//         hdr.extend_from_slice(t);
//         hdr.push(0x18); // field 3, wire type 0
//         encode_varint(&mut hdr, blob.len() as u64);
//         let mut out = (hdr.len() as u32).to_be_bytes().to_vec();
//         out.extend_from_slice(&hdr);
//         out.extend_from_slice(&blob);
//         out
//     }

//     // retorna caminhos únicos para o pbf e db de cada teste, removendo arquivos anteriores
//     fn tmp_paths(id: &str) -> (String, String) {
//         let dir = std::env::temp_dir();
//         let pbf = dir.join(format!("geolite_test_{id}.osm.pbf")).to_string_lossy().into_owned();
//         let db  = dir.join(format!("geolite_test_{id}.db")).to_string_lossy().into_owned();
//         let _ = std::fs::remove_file(&pbf);
//         let _ = std::fs::remove_file(&db);
//         (pbf, db)
//     }

//     fn row_count(db: &str, table: &str) -> i64 {
//         let conn = rusqlite::Connection::open(db).unwrap();
//         conn.query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |r| r.get(0)).unwrap()
//     }

//     fn first_payload(db: &str, table: &str) -> serde_json::Value {
//         let conn = rusqlite::Connection::open(db).unwrap();
//         let s: String = conn
//             .query_row(&format!("SELECT json(payload) FROM {table} LIMIT 1"), [], |r| r.get(0))
//             .unwrap();
//         serde_json::from_str(&s).unwrap()
//     }

//     fn default_opts() -> data_opts {
//         data_opts {
//             include_nodes: true,
//             include_ways: true,
//             include_relations: true,
//             ignore_info: true,
//             tags_include: None,
//             tags_ignore: None,
//         }
//     }

//     // 01.00: run() deve reportar contagem correta no output ao processar arquivo com
//     // 1 node, 1 way e 1 relation em chunks separados
//     #[test]
//     fn test_01_00() {
//         let (pbf, db) = tmp_paths("01_00");
//         let content: Vec<u8> = [
//             make_pbf_file(PRIMITIVE_BLOCK_1_NODE),
//             make_pbf_file(PRIMITIVE_BLOCK_1_WAY),
//             make_pbf_file(PRIMITIVE_BLOCK_1_RELATION),
//         ]
//         .concat();
//         std::fs::write(&pbf, &content).unwrap();

//         let mut out = Vec::new();
//         run(&pbf, &db, default_opts(), 1, &mut out);
//         let output = String::from_utf8(out).unwrap();

//         assert!(output.contains("1 nodes"), "output: {output}");
//         assert!(output.contains("1 ways"), "output: {output}");
//         assert!(output.contains("1 relations"), "output: {output}");
//     }

//     // 01.01: arquivo pbf vazio → db permanece vazio,
//     // output indica 0 nodes, 0 ways, 0 relations
//     #[test]
//     fn test_01_01() {
//         let (pbf, db) = tmp_paths("01_01");
//         std::fs::write(&pbf, &[] as &[u8]).unwrap();

//         let mut out = Vec::new();
//         run(&pbf, &db, default_opts(), 1, &mut out);
//         let output = String::from_utf8(out).unwrap();

//         assert!(output.contains("0 nodes"), "output: {output}");
//         assert!(output.contains("0 ways"), "output: {output}");
//         assert!(output.contains("0 relations"), "output: {output}");
//         assert_eq!(row_count(&db, "osm_nodes"), 0);
//         assert_eq!(row_count(&db, "osm_ways"), 0);
//         assert_eq!(row_count(&db, "osm_relations"), 0);
//     }

//     // 01.02: deve extrair e persistir dense nodes de um chunk osm válido;
//     // verificar: osm_nodes com id=1, lat=0.0, lon=0.0, tags={}
//     #[test]
//     fn test_01_02() {
//         let (pbf, db) = tmp_paths("01_02");
//         std::fs::write(&pbf, make_pbf_file(PRIMITIVE_BLOCK_1_NODE)).unwrap();

//         run(
//             &pbf,
//             &db,
//             data_opts {
//                 include_nodes: true,
//                 include_ways: false,
//                 include_relations: false,
//                 ignore_info: true,
//                 tags_include: None,
//                 tags_ignore: None,
//             },
//             1,
//             &mut Vec::new(),
//         );

//         assert_eq!(row_count(&db, "osm_nodes"), 1);
//         assert_eq!(row_count(&db, "osm_ways"), 0);
//         assert_eq!(row_count(&db, "osm_relations"), 0);

//         let p = first_payload(&db, "osm_nodes");
//         assert_eq!(p["id"], 1);
//         assert_eq!(p["lat"], 0.0);
//         assert_eq!(p["lon"], 0.0);
//         assert_eq!(p["tags"], serde_json::json!({}));
//     }

//     // 01.03: deve extrair e persistir ways de um chunk osm válido;
//     // verificar: osm_ways com id=42, refs=[10,20], tags={}
//     #[test]
//     fn test_01_03() {
//         let (pbf, db) = tmp_paths("01_03");
//         std::fs::write(&pbf, make_pbf_file(PRIMITIVE_BLOCK_1_WAY)).unwrap();

//         run(
//             &pbf,
//             &db,
//             data_opts {
//                 include_nodes: false,
//                 include_ways: true,
//                 include_relations: false,
//                 ignore_info: true,
//                 tags_include: None,
//                 tags_ignore: None,
//             },
//             1,
//             &mut Vec::new(),
//         );

//         assert_eq!(row_count(&db, "osm_nodes"), 0);
//         assert_eq!(row_count(&db, "osm_ways"), 1);
//         assert_eq!(row_count(&db, "osm_relations"), 0);

//         let p = first_payload(&db, "osm_ways");
//         assert_eq!(p["id"], 42);
//         assert_eq!(p["refs"], serde_json::json!([10, 20]));
//         assert_eq!(p["tags"], serde_json::json!({}));
//     }

//     // 01.04: deve extrair e persistir relations de um chunk osm válido;
//     // verificar: osm_relations com id=7, members=[{type:"w",id:5,role:"outer"}], tags={}
//     #[test]
//     fn test_01_04() {
//         let (pbf, db) = tmp_paths("01_04");
//         std::fs::write(&pbf, make_pbf_file(PRIMITIVE_BLOCK_1_RELATION)).unwrap();

//         run(
//             &pbf,
//             &db,
//             data_opts {
//                 include_nodes: false,
//                 include_ways: false,
//                 include_relations: true,
//                 ignore_info: true,
//                 tags_include: None,
//                 tags_ignore: None,
//             },
//             1,
//             &mut Vec::new(),
//         );

//         assert_eq!(row_count(&db, "osm_nodes"), 0);
//         assert_eq!(row_count(&db, "osm_ways"), 0);
//         assert_eq!(row_count(&db, "osm_relations"), 1);

//         let p = first_payload(&db, "osm_relations");
//         assert_eq!(p["id"], 7);
//         assert_eq!(p["tags"], serde_json::json!({}));
//         assert_eq!(p["members"][0]["type"], "w");
//         assert_eq!(p["members"][0]["id"], 5);
//         assert_eq!(p["members"][0]["role"], "outer");
//     }

//     // 01.05: com include_nodes=false, include_ways=false, include_relations=false,
//     // nenhum elemento deve ser persistido mesmo com arquivo contendo os 3 tipos
//     #[test]
//     fn test_01_05() {
//         let (pbf, db) = tmp_paths("01_05");
//         let content: Vec<u8> = [
//             make_pbf_file(PRIMITIVE_BLOCK_1_NODE),
//             make_pbf_file(PRIMITIVE_BLOCK_1_WAY),
//             make_pbf_file(PRIMITIVE_BLOCK_1_RELATION),
//         ]
//         .concat();
//         std::fs::write(&pbf, &content).unwrap();

//         run(
//             &pbf,
//             &db,
//             data_opts {
//                 include_nodes: false,
//                 include_ways: false,
//                 include_relations: false,
//                 ignore_info: true,
//                 tags_include: None,
//                 tags_ignore: None,
//             },
//             1,
//             &mut Vec::new(),
//         );

//         assert_eq!(row_count(&db, "osm_nodes"), 0);
//         assert_eq!(row_count(&db, "osm_ways"), 0);
//         assert_eq!(row_count(&db, "osm_relations"), 0);
//     }

//     // 01.06: tags_include=["name"] num node com tags {name:"Test",amenity:"cafe"}
//     // → payload deve conter apenas { name: "Test" }
//     #[test]
//     fn test_01_06() {
//         let (pbf, db) = tmp_paths("01_06");
//         std::fs::write(&pbf, make_pbf_file(PRIMITIVE_BLOCK_1_NODE_WITH_TAGS)).unwrap();

//         run(
//             &pbf,
//             &db,
//             data_opts {
//                 include_nodes: true,
//                 include_ways: false,
//                 include_relations: false,
//                 ignore_info: true,
//                 tags_include: Some(vec!["name".to_string()]),
//                 tags_ignore: None,
//             },
//             1,
//             &mut Vec::new(),
//         );

//         let p = first_payload(&db, "osm_nodes");
//         assert_eq!(p["tags"], serde_json::json!({"name": "Test"}));
//     }

//     // 01.07: tags_include=["name","amenity"], tags_ignore=["amenity"] num node com
//     // tags {name:"Test",amenity:"cafe"} → payload deve conter apenas { name: "Test" }
//     #[test]
//     fn test_01_07() {
//         let (pbf, db) = tmp_paths("01_07");
//         std::fs::write(&pbf, make_pbf_file(PRIMITIVE_BLOCK_1_NODE_WITH_TAGS)).unwrap();

//         run(
//             &pbf,
//             &db,
//             data_opts {
//                 include_nodes: true,
//                 include_ways: false,
//                 include_relations: false,
//                 ignore_info: true,
//                 tags_include: Some(vec!["name".to_string(), "amenity".to_string()]),
//                 tags_ignore: Some(vec!["amenity".to_string()]),
//             },
//             1,
//             &mut Vec::new(),
//         );

//         let p = first_payload(&db, "osm_nodes");
//         assert_eq!(p["tags"], serde_json::json!({"name": "Test"}));
//     }

//     // 01.08: processar o mesmo arquivo duas vezes não deve duplicar registros
//     // (on conflict / insert or ignore) → counts iguais após segunda execução
//     #[test]
//     fn test_01_08() {
//         let (pbf, db) = tmp_paths("01_08");
//         let content: Vec<u8> = [
//             make_pbf_file(PRIMITIVE_BLOCK_1_NODE),
//             make_pbf_file(PRIMITIVE_BLOCK_1_WAY),
//             make_pbf_file(PRIMITIVE_BLOCK_1_RELATION),
//         ]
//         .concat();
//         std::fs::write(&pbf, &content).unwrap();

//         run(&pbf, &db, default_opts(), 1, &mut Vec::new());
//         let after_first = (
//             row_count(&db, "osm_nodes"),
//             row_count(&db, "osm_ways"),
//             row_count(&db, "osm_relations"),
//         );

//         run(&pbf, &db, default_opts(), 1, &mut Vec::new());
//         let after_second = (
//             row_count(&db, "osm_nodes"),
//             row_count(&db, "osm_ways"),
//             row_count(&db, "osm_relations"),
//         );

//         assert_eq!(after_first, (1, 1, 1));
//         assert_eq!(after_second, after_first);
//     }

//     // 01.09: arquivo pbf inexistente → run() deve entrar em panic
//     #[test]
//     #[should_panic]
//     fn test_01_09() {
//         let db = std::env::temp_dir()
//             .join("geolite_test_01_09.db")
//             .to_string_lossy()
//             .into_owned();
//         run("/nonexistent/path/file.osm.pbf", &db, default_opts(), 1, &mut Vec::new());
//     }
// }
