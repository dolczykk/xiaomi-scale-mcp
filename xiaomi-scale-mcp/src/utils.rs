use std::str::FromStr;
use std::time::{SystemTime, UNIX_EPOCH};

pub(crate) fn profile_id(device_id: &str, account_id: &str) -> String {
    format!("{device_id}:{account_id}")
}

pub(crate) fn current_unix_millis() -> Result<i64, String> {
    let duration = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| error.to_string())?;

    i64::try_from(duration.as_millis()).map_err(|_| "current timestamp exceeds i64".to_string())
}

pub(crate) fn parse_required<T>(value: &str, field: &str) -> Result<T, String>
where
    T: FromStr,
{
    value
        .parse()
        .map_err(|_| format!("invalid {field} returned by Xiaomi"))
}

pub(crate) fn parse_optional<T>(value: Option<&str>, field: &str) -> Result<Option<T>, String>
where
    T: FromStr,
{
    value
        .filter(|value| !value.trim().is_empty())
        .map(|value| parse_required(value, field))
        .transpose()
}

#[cfg(test)]
mod tests {
    use super::{current_unix_millis, parse_optional, profile_id};

    #[test]
    fn profile_id_distinguishes_the_same_account_on_different_scales() {
        assert_ne!(
            profile_id("scale-one", "account"),
            profile_id("scale-two", "account")
        );
    }

    #[test]
    fn profile_id_is_stable_for_the_same_scale_and_account() {
        assert_eq!(profile_id("blt.4.scale", "123"), "blt.4.scale:123");
    }

    #[test]
    fn current_time_is_a_positive_unix_timestamp() {
        assert!(current_unix_millis().unwrap() > 0);
    }

    #[test]
    fn empty_optional_value_is_treated_as_missing() {
        assert_eq!(parse_optional::<u32>(Some(""), "value").unwrap(), None);
    }
}
