use serde_json::{Map, Value};

pub(crate) fn ensure_object(value: &mut Value) -> &mut Map<String, Value> {
    if !value.is_object() {
        *value = Value::Object(Map::new());
    }
    match value {
        Value::Object(object) => object,
        _ => unreachable!("ensure_object assigned an object"),
    }
}
