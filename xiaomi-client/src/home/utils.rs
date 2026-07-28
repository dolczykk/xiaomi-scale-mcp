pub fn get_xiaomi_home_api_url(url: &str, region: &str) -> String {
    if region.eq_ignore_ascii_case("cn") {
        return url.replace("{}.", "");
    }

    url.replace("{}", region)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::home::{XIAOMI_HOME_BASE_API, XIAOMI_HOME_CORE_BASE_API};

    #[test]
    fn omits_cn_region_prefix() {
        assert_eq!(
            get_xiaomi_home_api_url(XIAOMI_HOME_BASE_API, "cn"),
            "https://api.io.mi.com/app"
        );
        assert_eq!(
            get_xiaomi_home_api_url(XIAOMI_HOME_CORE_BASE_API, "cn"),
            "https://core.api.io.mi.com/app/v2"
        );
    }

    #[test]
    fn keeps_non_cn_region_prefix() {
        assert_eq!(
            get_xiaomi_home_api_url(XIAOMI_HOME_BASE_API, "de"),
            "https://de.api.io.mi.com/app"
        );
        assert_eq!(
            get_xiaomi_home_api_url(XIAOMI_HOME_CORE_BASE_API, "de"),
            "https://de.core.api.io.mi.com/app/v2"
        );
    }
}
