// shared kernel: concepts that more than one context needs with the very same meaning.
// nothing enters here that a single context could own by itself.

// a module and a type share rust's type namespace, so the type cannot be re-exported here under
// the module's own name. call sites import the type itself:
//   use crate::domain::kernel::admin_area_id::admin_area_id;
pub mod admin_area_id;
