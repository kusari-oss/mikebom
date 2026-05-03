// Member crate `a` — exists so Cargo treats this as a real package.
pub fn hello() -> &'static str { "a" }
