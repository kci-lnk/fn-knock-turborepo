use super::*;

pub(in crate::storage::redis_store) fn js_string(value: &Value) -> String {
    match value {
        Value::Null => String::new(),
        Value::String(value) => value.clone(),
        Value::Bool(value) => value.to_string(),
        Value::Number(value) => value.to_string(),
        Value::Array(items) => items.iter().map(js_string).collect::<Vec<_>>().join(","),
        Value::Object(_) => "[object Object]".to_string(),
    }
}

pub(in crate::storage::redis_store) fn js_finite_number(value: Option<&Value>) -> Option<f64> {
    let number = match value? {
        Value::Number(number) => number.as_f64()?,
        Value::String(value) => js_number_from_string(value)?,
        Value::Bool(value) => {
            if *value {
                1.0
            } else {
                0.0
            }
        }
        Value::Null => 0.0,
        Value::Array(items) => {
            js_number_from_string(&items.iter().map(js_string).collect::<Vec<_>>().join(","))?
        }
        Value::Object(_) => return None,
    };
    number.is_finite().then_some(number)
}

pub(in crate::storage::redis_store) fn js_number_from_string(value: &str) -> Option<f64> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Some(0.0);
    }

    let radix_value = if let Some(rest) = trimmed
        .strip_prefix("0x")
        .or_else(|| trimmed.strip_prefix("0X"))
    {
        Some(u128::from_str_radix(rest, 16).ok()? as f64)
    } else if let Some(rest) = trimmed
        .strip_prefix("0b")
        .or_else(|| trimmed.strip_prefix("0B"))
    {
        Some(u128::from_str_radix(rest, 2).ok()? as f64)
    } else if let Some(rest) = trimmed
        .strip_prefix("0o")
        .or_else(|| trimmed.strip_prefix("0O"))
    {
        Some(u128::from_str_radix(rest, 8).ok()? as f64)
    } else {
        None
    };

    match radix_value {
        Some(value) => Some(value),
        None => trimmed.parse::<f64>().ok(),
    }
}

pub(crate) fn node_locale_compare_ordering(left: &str, right: &str) -> Ordering {
    if left == right {
        return Ordering::Equal;
    }

    let mut left_chars = left.chars();
    let mut right_chars = right.chars();

    loop {
        match (left_chars.next(), right_chars.next()) {
            (Some(left), Some(right)) if left == right => {}
            (Some(left), Some(right)) => {
                let left_key = node_locale_char_key(left);
                let right_key = node_locale_char_key(right);
                let ordering = left_key.cmp(&right_key);
                return if ordering == Ordering::Equal {
                    left.cmp(&right)
                } else {
                    ordering
                };
            }
            (None, Some(_)) => return Ordering::Less,
            (Some(_), None) => return Ordering::Greater,
            (None, None) => return Ordering::Equal,
        }
    }
}

pub(in crate::storage::redis_store) fn node_locale_char_key(value: char) -> (u16, u16, u16, u32) {
    if let Some(rank) = node_ascii_punctuation_rank(value) {
        return (rank, 0, 0, value as u32);
    }

    if value.is_ascii_digit() {
        return (100 + value as u16 - b'0' as u16, 0, 0, value as u32);
    }

    if value.is_ascii_alphabetic() {
        let lower = value.to_ascii_lowercase();
        let letter = lower as u16 - b'a' as u16;
        let case = if value.is_ascii_lowercase() { 0 } else { 1 };
        return (200 + letter, 0, case, value as u32);
    }

    if let Some((letter, accent, case)) = node_latin_accent_key(value) {
        return (200 + letter, accent, case, value as u32);
    }

    if matches!(
        value as u32,
        0x3400..=0x4dbf | 0x4e00..=0x9fff | 0xf900..=0xfaff
    ) {
        return (400, 0, 0, value as u32);
    }

    (50, 0, 0, value as u32)
}

pub(in crate::storage::redis_store) fn node_ascii_punctuation_rank(value: char) -> Option<u16> {
    match value {
        ' ' => Some(0),
        '_' => Some(1),
        '-' => Some(2),
        ',' => Some(3),
        ';' => Some(4),
        ':' => Some(5),
        '!' => Some(6),
        '?' => Some(7),
        '.' => Some(8),
        '\'' => Some(9),
        '"' => Some(10),
        '(' => Some(11),
        ')' => Some(12),
        '[' => Some(13),
        ']' => Some(14),
        '{' => Some(15),
        '}' => Some(16),
        '@' => Some(17),
        '*' => Some(18),
        '/' => Some(19),
        '\\' => Some(20),
        '&' => Some(21),
        '#' => Some(22),
        '%' => Some(23),
        '`' => Some(24),
        '^' => Some(25),
        '+' => Some(26),
        '<' => Some(27),
        '=' => Some(28),
        '>' => Some(29),
        '|' => Some(30),
        '~' => Some(31),
        '$' => Some(32),
        _ => None,
    }
}

pub(in crate::storage::redis_store) fn node_latin_accent_key(
    value: char,
) -> Option<(u16, u16, u16)> {
    let case = if value.is_lowercase() { 0 } else { 1 };
    let (letter, accent) = match value {
        'á' | 'Á' => (0, 1),
        'å' | 'Å' => (0, 2),
        'ä' | 'Ä' => (0, 3),
        'à' | 'À' => (0, 4),
        'â' | 'Â' => (0, 5),
        'ã' | 'Ã' => (0, 6),
        'ā' | 'Ā' => (0, 7),
        'ă' | 'Ă' => (0, 8),
        'ą' | 'Ą' => (0, 9),
        'ç' | 'Ç' => (2, 1),
        'é' | 'É' => (4, 1),
        'è' | 'È' => (4, 2),
        'ê' | 'Ê' => (4, 3),
        'ë' | 'Ë' => (4, 4),
        'í' | 'Í' => (8, 1),
        'ì' | 'Ì' => (8, 2),
        'î' | 'Î' => (8, 3),
        'ï' | 'Ï' => (8, 4),
        'ñ' | 'Ñ' => (13, 1),
        'ó' | 'Ó' => (14, 1),
        'ò' | 'Ò' => (14, 2),
        'ô' | 'Ô' => (14, 3),
        'ö' | 'Ö' => (14, 4),
        'õ' | 'Õ' => (14, 5),
        'ú' | 'Ú' => (20, 1),
        'ù' | 'Ù' => (20, 2),
        'û' | 'Û' => (20, 3),
        'ü' | 'Ü' => (20, 4),
        'ý' | 'Ý' => (24, 1),
        'ÿ' | 'Ÿ' => (24, 2),
        _ => return None,
    };
    Some((letter, accent, case))
}
