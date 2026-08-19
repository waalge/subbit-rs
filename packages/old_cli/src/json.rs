use serde_json::{Map, Value};

/// Merges a vector of JSON values into a single JSON Object.
/// Non-object values in the vector are safely ignored.
pub fn merge(objects: Vec<Value>) -> Value {
    let mut merged_map = Map::new();

    for value in objects {
        if let Value::Object(obj) = value {
            merged_map.extend(obj);
        }
    }

    Value::Object(merged_map)
}
