pub(crate) fn current_unix_millis() -> anyhow::Result<i64> {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|error| anyhow::anyhow!("system clock is before the Unix epoch: {error}"))
        .map(|duration| i64::try_from(duration.as_millis()).unwrap_or(i64::MAX))
}
