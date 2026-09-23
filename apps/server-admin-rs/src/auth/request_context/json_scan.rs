//! Stream one credential at a time, then validate the entire remaining input.
//! IgnoredAny is insufficient here: serde_json skips its number range and depth
//! checks. DeserializeAny retains the same validation as serde_json::Value.
use serde::{
    Deserialize, Deserializer,
    de::{MapAccess, SeqAccess, Visitor},
};
use serde_json::Value;
use std::fmt;

pub(super) fn array(raw: &str, visit: impl FnMut(Value) -> bool) -> serde_json::Result<()> {
    let mut deserializer = serde_json::Deserializer::from_str(raw);
    if raw.trim_start().starts_with('[') {
        deserializer.deserialize_seq(ArrayVisitor(visit))?;
    } else {
        Discard::deserialize(&mut deserializer)?;
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
        loop {
            if selecting {
                let Some(value) = sequence.next_element::<Value>()? else {
                    break;
                };
                selecting = (self.0)(value);
            } else if sequence.next_element::<Discard>()?.is_none() {
                break;
            }
        }
        Ok(())
    }
}

struct Discard;
impl<'de> Deserialize<'de> for Discard {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_any(Discard)
    }
}
impl<'de> Visitor<'de> for Discard {
    type Value = Self;
    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a JSON value")
    }
    fn visit_bool<E>(self, _: bool) -> Result<Self, E> {
        Ok(self)
    }
    fn visit_i64<E>(self, _: i64) -> Result<Self, E> {
        Ok(self)
    }
    fn visit_u64<E>(self, _: u64) -> Result<Self, E> {
        Ok(self)
    }
    fn visit_f64<E>(self, _: f64) -> Result<Self, E> {
        Ok(self)
    }
    fn visit_str<E: serde::de::Error>(self, _: &str) -> Result<Self, E> {
        Ok(self)
    }
    fn visit_unit<E>(self) -> Result<Self, E> {
        Ok(self)
    }
    fn visit_seq<A: SeqAccess<'de>>(self, mut sequence: A) -> Result<Self, A::Error> {
        while sequence.next_element::<Discard>()?.is_some() {}
        Ok(self)
    }
    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Self, A::Error> {
        while map.next_entry::<Discard, Discard>()?.is_some() {}
        Ok(self)
    }
}
