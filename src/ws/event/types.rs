use serde::{Deserialize, Serialize};

/// Common user object
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct User {}

/// Common quoted message
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Quote {}
