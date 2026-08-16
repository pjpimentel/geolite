// finding the door number inside a free-text query.
//
// the query is not stripped of numbers before the text search runs, because a number can belong
// to the street's own name ("25" in "rua 25 de marco"). the number is picked out here instead,
// per street match: the street's own name tokens are removed first, and the FIRST remaining token
// that reads as a number is the one the user meant.

use super::value::house_number;
use super::policy::house_number_policy;

// a cheap pre-check: is there any number-shaped token at all? avoids loading a street's numbers
// when the query clearly has none.
pub fn has_house_number(query: &str, policy: &house_number_policy) -> bool {
  query
    .split_whitespace()
    .any(|token| house_number::recognize(token, policy).is_some())
}

pub fn first_house_number(
  query: &str,
  street_name: &str,
  policy: &house_number_policy,
) -> Option<house_number> {
  let tokens: Vec<&str> = query.split_whitespace().collect();
  let mut index = 0;
  while index < tokens.len() {
    let token = tokens[index];
    // a lone `#` introduces the number that follows it ("calle 82 # 52-48"), so the candidate is
    // the next token and both are consumed together.
    let (candidate, consumed) = match tokens.get(index + 1) {
      Some(next) if policy.allow_hash_prefix && token == "#" => (*next, 2),
      _ => (token, 1),
    };
    if !name_contains_token(street_name, candidate)
      && let Some(number) = house_number::recognize(candidate, policy)
    {
      return Some(number);
    }
    index += consumed;
  }
  None
}

// whether the token appears as a whole word inside the street's name, so that every number the
// name itself holds ("25" and "2024" in "rua 25 de marco de 2024") is passed over.
fn name_contains_token(name: &str, token: &str) -> bool {
  name
    .split(|c: char| !c.is_alphanumeric())
    .any(|part| part.eq_ignore_ascii_case(token))
}

#[cfg(test)]
#[path = "token.test.rs"]
mod tests;
