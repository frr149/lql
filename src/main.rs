//! Thin binary shell. All logic lives in the library crate (`lql::run`) so the
//! unit tests compile once instead of being duplicated into a second test
//! binary. See `docs/adr/0001-fast-iteration-builds.md`.

fn main() {
    std::process::exit(lql::run());
}
