//! Stream one credential at a time, then validate the entire remaining input.
//! Even unselected elements use Value's deserializer so dependency features
//! (including raw_value's private map representation) preserve legacy semantics.
use serde::{
    Deserialize, Deserializer,
    de::{SeqAccess, Visitor},
};
use serde_json::Value;
use std::fmt;

pub(super) fn array(raw: &str, mut visit: impl FnMut(Value) -> bool) -> serde_json::Result<()> {
    let mut deserializer = serde_json::Deserializer::from_str(raw);
    if raw.trim_start().starts_with('[') {
        deserializer.deserialize_seq(ArrayVisitor(visit))?;
    } else if let Value::Array(values) = Value::deserialize(&mut deserializer)? {
        // A RawValue private map can deserialize into an array. Preserve this
        // uncommon compatibility form even though it cannot use streaming.
        for value in values {
            if !visit(value) {
                break;
            }
        }
    }
    deserializer.end()
}

struct ArrayVisitor<F>(F);
impl<'de, F: FnMut(Value) -> bool> Visitor<'de> for ArrayVisitor<F> {
    type Value = ();
    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a credential array")
    }
    fn visit_seq<A: SeqAccess<'de>>(mut self, mut sequence: A) -> Result<(), A::Error> {
        let mut selecting = true;
        while let Some(value) = sequence.next_element::<Value>()? {
            if selecting {
                selecting = (self.0)(value);
            }
        }
        Ok(())
    }
}
