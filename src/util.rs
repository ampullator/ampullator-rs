//------------------------------------------------------------------------------
pub type Sample = f32;


pub(crate) fn split_name(s: &str) -> (&str, &str) {
    s.rsplit_once('.')
        .unwrap_or_else(|| panic!("Expected 'name.port', got: '{}'", s))
}
