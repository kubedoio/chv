//! `/proc/loadavg` one-minute load average.

pub(crate) const LOAD1: &str = "vm.guest.load1";

/// Parse the first field of `/proc/loadavg`
/// (`0.42 0.50 0.55 1/382 12345`).
pub(crate) fn parse(loadavg: &str) -> Option<f64> {
    let first = loadavg.split_whitespace().next()?;
    let v: f64 = first.parse().ok()?;
    // A load average is finite and non-negative; NaN/inf from a
    // malformed read is an absence.
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
        assert_eq!(parse("0.42 0.50 0.55 1/382 12345\n"), Some(0.42));
        assert_eq!(parse("0.00 0.00 0.00 1/100 1\n"), Some(0.0));
    }

    #[test]
    fn garbage_is_absence() {
        assert_eq!(parse(""), None);
        assert_eq!(parse("not-a-number 0.5 0.5 1/1 1\n"), None);
        assert_eq!(parse("NaN 0.5 0.5 1/1 1\n"), None);
        assert_eq!(parse("-1 0.5 0.5 1/1 1\n"), None);
    }
}
