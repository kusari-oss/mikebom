// Member crate `b` — depends on `a` via path-dep, exercising FR-011.
pub fn hello() -> String { format!("b uses {}", a::hello()) }
