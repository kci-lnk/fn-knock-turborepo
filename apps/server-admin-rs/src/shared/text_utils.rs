pub(crate) trait EmptyStringExt {
    fn if_empty(self, fallback: String) -> String;
}

impl EmptyStringExt for String {
    fn if_empty(self, fallback: String) -> String {
        if self.is_empty() { fallback } else { self }
    }
}

pub(crate) fn default_string(value: String, fallback: &str) -> String {
    if value.trim().is_empty() {
        fallback.to_string()
    } else {
        value
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn string_fallback_helpers_preserve_empty_rules() {
        assert_eq!("".to_string().if_empty("fallback".to_string()), "fallback");
        assert_eq!(
            "value".to_string().if_empty("fallback".to_string()),
            "value"
        );
        assert_eq!(default_string("   ".to_string(), "fallback"), "fallback");
        assert_eq!(default_string(" value ".to_string(), "fallback"), " value ");
    }
}
