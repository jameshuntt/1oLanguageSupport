// qweres/infra/language-server-1o/src/schema_definitions.rs

// Example: define allowed keys for each top-level modifier

use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct Schema {
    pub allowed_keys: HashMap<String, Vec<String>>,
}

impl Schema {
    pub fn default() -> Self {
        let mut allowed_keys = HashMap::new();

        allowed_keys.insert(
            "container".into(),
            vec!["name".into(), "memory".into(), "cpu".into(), "env".into()]
        );

        allowed_keys.insert(
            "model".into(),
            vec!["fields".into(), "primary_key".into(), "indexes".into()]
        );

        allowed_keys.insert(
            "slice".into(),
            vec!["query".into(), "limit".into(), "offset".into()]
        );

        allowed_keys.insert(
            "view".into(),
            vec!["template".into(), "data".into()]
        );

        Schema { allowed_keys }
    }
}
