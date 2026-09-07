use geo::{BoundingRect, Geometry};
use geozero::{CoordDimensions, ToGeo, ToWkb, wkb::SpatiaLiteWkb};
use rusqlite::types::{FromSql, FromSqlResult, ToSql, ToSqlOutput, ValueRef};

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

// reads the centre of the MBR header of a spatialite blob without parsing the geometry: byte 0
// is 0x00, byte 1 the endianness, bytes 2-5 the SRID, bytes 6-37 min_x, min_y, max_x, max_y as
// four f64. returns (lon, lat).
pub fn mbr_center(blob: &[u8]) -> Option<(f64, f64)> {
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
  let min_x = read(6);
  let min_y = read(14);
  let max_x = read(22);
  let max_y = read(30);
  Some(((min_x + max_x) / 2.0, (min_y + max_y) / 2.0))
}

#[derive(Clone, Copy)]
pub struct bounding_box {
  pub min_lat: f64,
  pub max_lat: f64,
  pub min_lon: f64,
  pub max_lon: f64,
}

#[cfg(test)]
#[path = "geometry.test.rs"]
mod tests;
