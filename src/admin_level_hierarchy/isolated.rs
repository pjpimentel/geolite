use rusqlite::Connection;

use super::repository;
use super::street_merge::IDS_PER_READ;
use crate::admin_level::repository as admin_level_repository;
use crate::house_number::repository as house_number_repository;

pub struct isolated_areas {
  countries: Vec<i64>,
  streets: Vec<i64>,
}

impl isolated_areas {
  pub fn is_empty(&self) -> bool {
    self.countries.is_empty() && self.streets.is_empty()
  }
}

pub struct removal_report {
  pub countries: u64,
  pub streets: u64,
  pub house_numbers: u64,
}

pub fn find(conn: &Connection) -> isolated_areas {
  isolated_areas {
    countries: repository::childless_country_ids(conn),
    streets: repository::parentless_street_ids(conn),
  }
}

fn delete(conn: &Connection, ids: &[i64]) -> u64 {
  ids
    .chunks(IDS_PER_READ)
    .map(|chunk| admin_level_repository::delete_by_ids(conn, chunk) as u64)
    .sum()
}

pub fn remove(conn: &Connection, found: &isolated_areas) -> removal_report {
  let tx = conn
    .unchecked_transaction()
    .expect("failed to begin transaction");
  let numbers_before = house_number_repository::count(&tx);
  let countries = delete(&tx, &found.countries);
  let streets = delete(&tx, &found.streets);
  let house_numbers = (numbers_before - house_number_repository::count(&tx)) as u64;
  tx.commit().expect("failed to commit");
  removal_report {
    countries,
    streets,
    house_numbers,
  }
}
