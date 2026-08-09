DEFINE TABLE IF NOT EXISTS profile_cache SCHEMAFULL;
DEFINE FIELD IF NOT EXISTS xiaomi_user_id ON TABLE profile_cache TYPE string;
DEFINE FIELD IF NOT EXISTS fetched_at_ms ON TABLE profile_cache TYPE int;
DEFINE FIELD IF NOT EXISTS payload ON TABLE profile_cache TYPE string;

DEFINE TABLE IF NOT EXISTS weight_cache SCHEMAFULL;
DEFINE FIELD IF NOT EXISTS xiaomi_user_id ON TABLE weight_cache TYPE string;
DEFINE FIELD IF NOT EXISTS cache_key ON TABLE weight_cache TYPE string;
DEFINE FIELD IF NOT EXISTS profile_id ON TABLE weight_cache TYPE string;
DEFINE FIELD IF NOT EXISTS fetched_at_ms ON TABLE weight_cache TYPE int;
DEFINE FIELD IF NOT EXISTS payload ON TABLE weight_cache TYPE string;
