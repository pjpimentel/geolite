use geo::{Coord, Geometry, LineString};
use rusqlite::types::{FromSql, ToSql, ToSqlOutput, Value, ValueRef};

use super::{admin_geometry, approx_eq, assemble_rings, mbr_center};

fn line(from: (f64, f64), to: (f64, f64)) -> admin_geometry {
  Geometry::LineString(LineString(vec![
    Coord { x: from.0, y: from.1 },
    Coord { x: to.0, y: to.1 },
  ]))
  .into()
}

fn blob_of(geometry: &admin_geometry) -> Vec<u8> {
  match geometry.to_sql().expect("to_sql") {
    ToSqlOutput::Owned(Value::Blob(b)) => b,
    ToSqlOutput::Borrowed(ValueRef::Blob(b)) => b.to_vec(),
    _ => panic!("expected a blob"),
  }
}

#[test]
fn _00_mbr_center_reads_the_spatialite_bbox_center() {
  let blob = blob_of(&line((10.0, 20.0), (30.0, 40.0)));
  let (cx, cy) = mbr_center(&blob).expect("mbr center");
  assert!((cx - 20.0).abs() < 1e-9, "cx={cx}");
  assert!((cy - 30.0).abs() < 1e-9, "cy={cy}");
}

#[test]
fn _01_mbr_center_rejects_short_or_invalid_blobs() {
  assert_eq!(mbr_center(&[]), None);
  assert_eq!(mbr_center(&[0u8; 10]), None);
  assert_eq!(mbr_center(&[0xFFu8; 40]), None);
}

#[test]
fn _02_geometry_round_trips_through_the_column_format() {
  let blob = blob_of(&line((-46.3198, -23.9724), (-46.3197, -23.9723)));
  let read = admin_geometry::column_result(ValueRef::Blob(&blob)).expect("column_result");
  match read.geometry() {
    Geometry::LineString(ls) => {
      assert_eq!(ls.0.len(), 2);
      assert!((ls.0[0].x - -46.3198).abs() < 1e-9);
      assert!((ls.0[1].y - -23.9723).abs() < 1e-9);
    }
    other => panic!("expected a LineString, got {other:?}"),
  }
}

#[test]
fn _03_unreadable_blob_degrades_to_an_empty_geometry() {
  let read = admin_geometry::column_result(ValueRef::Blob(&[0u8; 8])).expect("column_result");
  assert!(matches!(
    read.geometry(),
    Geometry::GeometryCollection(collection) if collection.0.is_empty()
  ));
}

fn ls(points: &[(f64, f64)]) -> LineString<f64> {
  LineString(points.iter().map(|&(x, y)| Coord { x, y }).collect())
}

// 04: coordenadas identicas sao aproximadamente iguais
#[test]
fn _04_identical_coords_are_equal() {
  assert!(approx_eq(
    Coord { x: 1.5, y: -2.5 },
    Coord { x: 1.5, y: -2.5 }
  ));
}

// 05: diferenca abaixo da tolerancia de 1e-9 ainda conta como igual
#[test]
fn _05_difference_below_tolerance_is_equal() {
  assert!(approx_eq(
    Coord { x: 1.0, y: 1.0 },
    Coord {
      x: 1.0 + 1e-12,
      y: 1.0 - 1e-12
    }
  ));
}

// 06: divergencia acima da tolerancia em x ou em y quebra a igualdade
#[test]
fn _06_difference_above_tolerance_is_not_equal() {
  assert!(!approx_eq(Coord { x: 1.0, y: 1.0 }, Coord { x: 1.1, y: 1.0 }));
  assert!(!approx_eq(Coord { x: 1.0, y: 1.0 }, Coord { x: 1.0, y: 1.1 }));
}

// 07: lista vazia de ways nao produz anel nenhum
#[test]
fn _07_no_ways_produces_no_rings() {
  assert!(assemble_rings(&[]).is_empty());
}

// 08: way unico vira um anel com as mesmas coordenadas
#[test]
fn _08_single_way_becomes_a_single_ring() {
  let rings = assemble_rings(&[ls(&[(0.0, 0.0), (1.0, 0.0), (1.0, 1.0)])]);
  assert_eq!(rings.len(), 1);
  assert_eq!(rings[0].0.len(), 3);
}

// 09: fim do way A encosta no inicio do way B → juncao direta, sem inverter B
#[test]
fn _09_joins_ways_head_to_tail() {
  let rings = assemble_rings(&[
    ls(&[(0.0, 0.0), (1.0, 0.0)]),
    ls(&[(1.0, 0.0), (1.0, 1.0)]),
  ]);
  assert_eq!(rings.len(), 1);
  assert_eq!(rings[0].0.len(), 3);
  assert!(approx_eq(rings[0].0[2], Coord { x: 1.0, y: 1.0 }));
}

// 10: fim do way A encosta no FIM do way B → B precisa ser invertido antes de juntar
#[test]
fn _10_reverses_way_when_joining_tail_to_tail() {
  let rings = assemble_rings(&[
    ls(&[(0.0, 0.0), (1.0, 0.0)]),
    ls(&[(1.0, 1.0), (1.0, 0.0)]),
  ]);
  assert_eq!(rings.len(), 1);
  assert_eq!(rings[0].0.len(), 3);
  assert!(
    approx_eq(rings[0].0[2], Coord { x: 1.0, y: 1.0 }),
    "o way invertido deve terminar no seu ponto inicial original"
  );
}

// 11: quatro segmentos encadeados fecham um quadrado (primeiro ponto == ultimo)
#[test]
fn _11_chained_segments_close_into_a_ring() {
  let rings = assemble_rings(&[
    ls(&[(0.0, 0.0), (1.0, 0.0)]),
    ls(&[(1.0, 0.0), (1.0, 1.0)]),
    ls(&[(1.0, 1.0), (0.0, 1.0)]),
    ls(&[(0.0, 1.0), (0.0, 0.0)]),
  ]);
  assert_eq!(rings.len(), 1);
  assert_eq!(rings[0].0.len(), 5);
  assert!(approx_eq(rings[0].0[0], *rings[0].0.last().unwrap()));
}

// 12: segmentos que nao se tocam viram aneis separados
#[test]
fn _12_disjoint_ways_become_separate_rings() {
  let rings = assemble_rings(&[
    ls(&[(0.0, 0.0), (1.0, 0.0)]),
    ls(&[(10.0, 10.0), (11.0, 10.0)]),
  ]);
  assert_eq!(rings.len(), 2);
}
