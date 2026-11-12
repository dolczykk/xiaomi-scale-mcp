use chrono::{DateTime, NaiveDate};

pub(crate) fn current_unix_millis() -> anyhow::Result<i64> {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|error| anyhow::anyhow!("system clock is before the Unix epoch: {error}"))
        .map(|duration| i64::try_from(duration.as_millis()).unwrap_or(i64::MAX))
}

pub(crate) fn parse_before_millis(value: &str) -> Result<i64, String> {
    let timestamp = DateTime::parse_from_rfc3339(value)
        .map(|date| date.timestamp_millis())
        .ok()
        .or_else(|| {
            let date = NaiveDate::parse_from_str(value, "%Y-%m-%d").ok()?;
            if date.format("%Y-%m-%d").to_string() != value {
                return None;
            }
            Some(date.and_hms_opt(0, 0, 0)?.and_utc().timestamp_millis())
        })
        .ok_or_else(|| {
            "before must be a YYYY-MM-DD date (e.g. 2026-11-12, midnight UTC) or an RFC 3339 datetime with timezone (e.g. 2026-11-12T15:30:00+01:00)".to_string()
        })?;

    if timestamp <= 0 {
        return Err("before must be after the Unix epoch (1970-01-01T00:00:00Z)".to_string());
    }

    Ok(timestamp)
}

#[cfg(test)]
mod tests {
    use super::parse_before_millis;

    #[test]
    fn parses_dates_at_midnight_utc_and_equivalent_timezone_offsets() {
        let expected = parse_before_millis("2026-11-12T00:00:00Z").unwrap();
        assert_eq!(parse_before_millis("2026-11-12"), Ok(expected));
        assert_eq!(
            parse_before_millis("2026-11-12T01:00:00+01:00"),
            Ok(expected)
        );
        assert_eq!(
            parse_before_millis("2026-11-11T19:00:00-05:00"),
            Ok(expected)
        );
        assert_eq!(
            parse_before_millis("2026-11-12T00:00:00.123Z"),
            Ok(expected + 123)
        );
    }

    #[test]
    fn accepts_valid_leap_days_and_positive_milliseconds() {
        assert!(parse_before_millis("2024-02-29").is_ok());
        assert_eq!(parse_before_millis("1970-01-01T00:00:00.001Z"), Ok(1));
        assert_eq!(parse_before_millis("1970-01-02"), Ok(86_400_000));
    }

    #[test]
    fn rejects_invalid_dates_and_unsupported_formats() {
        for value in [
            "",
            "1794441600000",
            "2026-02-29",
            "2026-13-01",
            "2026-11-12T15:30:00",
            "2026-1-2",
            "12/11/2026",
        ] {
            let error = parse_before_millis(value).unwrap_err();
            assert!(error.contains("YYYY-MM-DD"), "{value}: {error}");
            assert!(error.contains("RFC 3339"), "{value}: {error}");
        }
    }

    #[test]
    fn rejects_epoch_and_earlier_dates() {
        for value in [
            "1970-01-01",
            "1970-01-01T00:00:00Z",
            "1969-12-31",
            "1970-01-01T01:00:00+01:00",
        ] {
            assert!(
                parse_before_millis(value)
                    .unwrap_err()
                    .contains("Unix epoch")
            );
        }
    }
}
