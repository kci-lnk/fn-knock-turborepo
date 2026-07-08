pub(crate) fn compare_version(left: &str, right: &str) -> i32 {
    let left_parts = version_parts(left);
    let right_parts = version_parts(right);
    let max_len = left_parts.len().max(right_parts.len()).max(3);
    for index in 0..max_len {
        let left = *left_parts.get(index).unwrap_or(&0);
        let right = *right_parts.get(index).unwrap_or(&0);
        if left > right {
            return 1;
        }
        if left < right {
            return -1;
        }
    }
    0
}

pub(crate) fn version_parts(value: &str) -> Vec<i64> {
    value
        .trim()
        .split('.')
        .map(|part| {
            let digits = part
                .chars()
                .skip_while(|ch| !ch.is_ascii_digit())
                .take_while(|ch| ch.is_ascii_digit())
                .collect::<String>();
            digits.parse::<i64>().unwrap_or(0)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compares_node_compatible_application_versions() {
        assert_eq!(compare_version("1.8.7", "1.8.6"), 1);
        assert_eq!(compare_version("1.8.6", "1.8.6"), 0);
        assert_eq!(compare_version("1.8.6-beta", "1.8.7"), -1);
        assert_eq!(compare_version("v1.8.8", "1.8.8"), 0);
        assert_eq!(compare_version("1.8", "1.8.0"), 0);
    }
}
