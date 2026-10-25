/// Strips everything but ASCII digits, matching v1's `phone.replace(/\D/g, '')`.
/// Returns `None` when nothing is left.
pub fn normalize(phone: &str) -> Option<String> {
    let digits: String = phone.chars().filter(char::is_ascii_digit).collect();
    (!digits.is_empty()).then_some(digits)
}

#[cfg(test)]
mod tests {
    use super::normalize;

    #[test]
    fn strips_formatting() {
        assert_eq!(
            normalize("+62 812-3456-7890").as_deref(),
            Some("6281234567890")
        );
        assert_eq!(normalize("(0812) 345.678").as_deref(), Some("0812345678"));
    }

    #[test]
    fn rejects_empty() {
        assert_eq!(normalize(""), None);
        assert_eq!(normalize("+ -"), None);
    }
}
