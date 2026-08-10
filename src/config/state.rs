use serde::{Deserialize, Serialize};
use std::collections::HashSet;

#[derive(Deserialize, Serialize)]
pub struct StateConfig {
    pub active: HashSet<String>,
}
