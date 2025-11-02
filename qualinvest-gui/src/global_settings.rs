use serde::{Deserialize, Serialize};
use time::macros::datetime;

/// Global settings per database, such as the inception date for all market data.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GlobalSettings {
    pub inception_date: time::OffsetDateTime,
}

impl Default for GlobalSettings {
    fn default() -> Self {
        Self {
            inception_date: datetime!(2000-01-01 0:00 UTC),
        }
    }
}
