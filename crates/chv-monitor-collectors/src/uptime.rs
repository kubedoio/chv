//! `/proc/uptime` (seconds).

pub(crate) const UPTIME_SECONDS: &str = "vm.guest.uptime_seconds";

/// Parse the first field of `/proc/uptime` (`12345.67 23456.78`).
pub(crate) fn parse_seconds(uptime: &str) -> Option<f64> {
    let first = uptime.split_whitespace().next()?;
    let v: f64 = first.parse().ok()?;
    if !v.is_finite() || v < 0.0 {
        return None;
    }
    Some(v)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_first_field() {
        assert_eq!(parse_seconds("12345.67 23456.78\n"), Some(12345.67));
    }

    #[test]
    fn garbage_is_absence() {
        assert_eq!(parse_seconds(""), None);
        assert_eq!(parse_seconds("up 3 days\n"), None);
        assert_eq!(parse_seconds("-1 2\n"), None);
    }
}
