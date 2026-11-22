//! Fixture files beneath a crate's `tests/fixtures` directory.

use std::path::{Path, PathBuf};

use crate::Reply;

/// Construct with the calling crate's `env!("CARGO_MANIFEST_DIR")`.
pub struct Fixtures(pub &'static str);

impl Fixtures {
    pub fn path(&self, name: &str) -> PathBuf {
        Path::new(self.0).join("tests/fixtures").join(name)
    }

    pub fn json(&self, name: &str) -> Reply {
        Reply::ok()
            .content_type("application/json")
            .fixture(self.path(name))
    }

    pub fn html(&self, name: &str) -> Reply {
        Reply::ok()
            .content_type("text/html; charset=utf-8")
            .fixture(self.path(name))
    }

    pub fn read_json(&self, name: &str) -> serde_json::Value {
        serde_json::from_slice(&std::fs::read(self.path(name)).expect("read fixture"))
            .expect("fixture is JSON")
    }
}
