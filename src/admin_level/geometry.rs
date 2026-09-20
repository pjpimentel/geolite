use geo::{BoundingRect, Contains, Coord, Geometry, Line, LineString, MultiLineString, Point};
use geozero::{CoordDimensions, ToGeo, ToWkb, wkb::SpatiaLiteWkb};
use rstar::{RTree, primitives::GeomWithData};
use rusqlite::types::{FromSql, FromSqlResult, ToSql, ToSqlOutput, ValueRef};
use std::collections::{BTreeSet, HashMap, HashSet};

pub struct admin_geometry(pub Geometry<f64>);

impl admin_geometry {
  pub fn geometry(&self) -> &Geometry<f64> {
    &self.0
  }

  pub fn into_geometry(self) -> Geometry<f64> {
    self.0
  }
}

impl From<Geometry<f64>> for admin_geometry {
  fn from(geometry: Geometry<f64>) -> Self {
    Self(geometry)
  }
}

impl ToSql for admin_geometry {
  fn to_sql(&self) -> rusqlite::Result<ToSqlOutput<'_>> {
    let bbox = self.0.bounding_rect().ok_or_else(|| {
      rusqlite::Error::ToSqlConversionFailure(Box::<dyn std::error::Error + Send + Sync>::from(
        "admin_geometry has no bounding rect",
      ))
    })?;
    let envelope = vec![bbox.min().x, bbox.min().y, bbox.max().x, bbox.max().y];
    let blob = self
      .0
      .to_spatialite_wkb(CoordDimensions::default(), Some(4326), envelope)
      .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
    Ok(ToSqlOutput::from(blob))
  }
}

impl FromSql for admin_geometry {
  fn column_result(value: ValueRef<'_>) -> FromSqlResult<Self> {
    let blob = value.as_blob()?;
    let empty = || {
      admin_geometry(Geometry::GeometryCollection(geo::GeometryCollection(
        vec![],
      )))
    };
    if blob.len() <= 40 {
      eprintln!(
        "warn: admin_geometry: blob too short ({} bytes); returning empty sentinel",
        blob.len()
      );
      return Ok(empty());
    }
    // geozero's to_spatialite_wkb omits the byte-order byte from the WKB body and uses 0x69 as
    // the sub-geometry separator, so the ISO WKB reader cannot parse it; SpatiaLiteWkb can.
    match SpatiaLiteWkb(blob).to_geo() {
      Ok(geometry) => Ok(admin_geometry(geometry)),
      Err(e) => {
        let preview: Vec<String> = blob.iter().take(16).map(|b| format!("{:02x}", b)).collect();
        eprintln!(
          "warn: admin_geometry: WKB parse failed ({} bytes, head=[{}]): {:?}",
          blob.len(),
          preview.join(" "),
          e
        );
        Ok(empty())
      }
    }
  }
}

// reads the MBR header of a spatialite blob without parsing the geometry: byte 0 is 0x00, byte 1
// the endianness, bytes 2-5 the SRID, bytes 6-37 min_x, min_y, max_x, max_y as four f64
pub fn mbr_of(blob: &[u8]) -> Option<bounding_box> {
  if blob.len() < 38 || blob[0] != 0x00 {
    return None;
  }
  let little_endian = blob[1] == 0x01;
  let read = |offset: usize| -> f64 {
    let mut buf = [0u8; 8];
    buf.copy_from_slice(&blob[offset..offset + 8]);
    if little_endian {
      f64::from_le_bytes(buf)
    } else {
      f64::from_be_bytes(buf)
    }
  };
  Some(bounding_box {
    min_lon: read(6),
    min_lat: read(14),
    max_lon: read(22),
    max_lat: read(30),
  })
}

pub fn mbr_center(blob: &[u8]) -> Option<(f64, f64)> {
  let mbr = mbr_of(blob)?;
  Some((
    (mbr.min_lon + mbr.max_lon) / 2.0,
    (mbr.min_lat + mbr.max_lat) / 2.0,
  ))
}

pub fn approx_eq(a: Coord<f64>, b: Coord<f64>) -> bool {
  (a.x - b.x).abs() < 1e-9 && (a.y - b.y).abs() < 1e-9
}

pub fn assemble_rings(ways: &[LineString<f64>]) -> Vec<LineString<f64>> {
  let mut remaining: Vec<(LineString<f64>, bool)> =
    ways.iter().map(|w| (w.clone(), false)).collect();
  let mut rings: Vec<LineString<f64>> = Vec::new();
  while let Some(start_idx) = remaining.iter().position(|(_, used)| !used) {
    let mut coords: Vec<Coord<f64>> = remaining[start_idx].0.0.clone();
    remaining[start_idx].1 = true;
    while let Some(next) = take_continuation(&mut remaining, *coords.last().unwrap()) {
      coords.extend_from_slice(&next[1..]);
    }
    rings.push(LineString(coords));
  }
  rings
}

fn take_continuation(
  remaining: &mut [(LineString<f64>, bool)],
  tail: Coord<f64>,
) -> Option<Vec<Coord<f64>>> {
  for (ls, used) in remaining.iter_mut() {
    if *used {
      continue;
    }
    let pts = &ls.0;
    if approx_eq(pts[0], tail) {
      *used = true;
      return Some(pts.clone());
    }
    if approx_eq(pts[pts.len() - 1], tail) {
      *used = true;
      let mut rev = pts.clone();
      rev.reverse();
      return Some(rev);
    }
  }
  None
}

pub fn lines_of(geometry: Geometry<f64>) -> Vec<LineString<f64>> {
  let lines = match geometry {
    Geometry::LineString(line) => vec![line],
    Geometry::MultiLineString(lines) => lines.0,
    _ => Vec::new(),
  };
  lines.into_iter().filter(|line| !line.0.is_empty()).collect()
}

const METERS_PER_DEGREE: f64 = 111_320.0;

fn projected(coord: &Coord<f64>) -> Point<f64> {
  Point::new(
    coord.x * coord.y.to_radians().cos() * METERS_PER_DEGREE,
    coord.y * METERS_PER_DEGREE,
  )
}

pub fn nearby_pairs(members: &[Vec<LineString<f64>>], reach_in_meters: f64) -> Vec<(usize, usize)> {
  let mut pairs: BTreeSet<(usize, usize)> = BTreeSet::new();
  let mut pair = |a: usize, b: usize| {
    if a != b {
      pairs.insert((a.min(b), a.max(b)));
    }
  };

  let mut owners: HashMap<(u64, u64), Vec<usize>> = HashMap::new();
  for (member, lines) in members.iter().enumerate() {
    for coord in lines.iter().flat_map(|line| line.0.iter()) {
      let here = owners
        .entry((coord.x.to_bits(), coord.y.to_bits()))
        .or_default();
      here.iter().for_each(|&other| pair(other, member));
      if here.last() != Some(&member) {
        here.push(member);
      }
    }
  }

  let tree = RTree::bulk_load(
    members
      .iter()
      .enumerate()
      .flat_map(|(member, lines)| {
        lines.iter().flat_map(move |line| {
          line.0.windows(2).map(move |pair| {
            GeomWithData::new(Line::new(projected(&pair[0]), projected(&pair[1])), member)
          })
        })
      })
      .collect(),
  );
  for (member, lines) in members.iter().enumerate() {
    for end in lines
      .iter()
      .flat_map(|line| [line.0.first(), line.0.last()])
      .flatten()
    {
      for near in tree.locate_within_distance(projected(end), reach_in_meters * reach_in_meters) {
        pair(near.data, member);
      }
    }
  }
  pairs.into_iter().collect()
}

pub fn fold_lines(lines: Vec<LineString<f64>>) -> Option<Geometry<f64>> {
  let mut seen: HashSet<Vec<(u64, u64)>> = HashSet::new();
  let mut kept: Vec<LineString<f64>> = lines
    .into_iter()
    .filter(|line| {
      seen.insert(
        line
          .0
          .iter()
          .map(|coord| (coord.x.to_bits(), coord.y.to_bits()))
          .collect(),
      )
    })
    .collect();
  match kept.len() {
    0 => None,
    1 => kept.pop().map(Geometry::LineString),
    _ => Some(Geometry::MultiLineString(MultiLineString(kept))),
  }
}

#[derive(Clone, Copy)]
pub struct bounding_box {
  pub min_lat: f64,
  pub max_lat: f64,
  pub min_lon: f64,
  pub max_lon: f64,
}

impl bounding_box {
  pub fn covers(&self, point: &Point<f64>) -> bool {
    (self.min_lon..=self.max_lon).contains(&point.x())
      && (self.min_lat..=self.max_lat).contains(&point.y())
  }

  pub fn center(&self) -> Point<f64> {
    Point::new(
      (self.min_lon + self.max_lon) / 2.0,
      (self.min_lat + self.max_lat) / 2.0,
    )
  }
}

#[derive(Clone)]
pub struct bounding_geometry {
  pub geometry: Geometry<f64>,
  pub envelope: bounding_box,
}

impl bounding_geometry {
  pub fn contains(&self, lat: f64, lon: f64) -> bool {
    self.geometry.contains(&Point::new(lon, lat))
  }
}

// wkt orders "x y" = "lon lat". only an area (polygon/multipolygon) is accepted: the envelope only
// feeds the rtree pre-filter, and `contains` is the exact test
pub fn parse_bounding_wkt(s: &str) -> Result<bounding_geometry, String> {
  let geometry = geozero::wkt::Wkt(s)
    .to_geo()
    .map_err(|e| format!("bounding_wkt: invalid wkt: {e}"))?;
  if !matches!(geometry, Geometry::Polygon(_) | Geometry::MultiPolygon(_)) {
    return Err("bounding_wkt: must be a POLYGON or MULTIPOLYGON".to_string());
  }
  let rect = geometry
    .bounding_rect()
    .ok_or("bounding_wkt: empty geometry")?;
  let envelope = bounding_box {
    min_lat: rect.min().y,
    max_lat: rect.max().y,
    min_lon: rect.min().x,
    max_lon: rect.max().x,
  };
  Ok(bounding_geometry { geometry, envelope })
}

#[cfg(test)]
#[path = "geometry.test.rs"]
mod tests;
