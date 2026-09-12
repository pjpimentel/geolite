use super::value::house_number;
use super::policy::house_number_policy;

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

fn name_contains_token(name: &str, token: &str) -> bool {
  name
    .split(|c: char| !c.is_alphanumeric())
    .any(|part| part.eq_ignore_ascii_case(token))
}

#[cfg(test)]
#[path = "token.test.rs"]
mod tests;
