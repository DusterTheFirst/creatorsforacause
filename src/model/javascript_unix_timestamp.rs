use serde::{Deserialize, Deserializer, Serializer};
use time::OffsetDateTime;

pub fn js_timestamp_to_date_time(timestamp: i64) -> OffsetDateTime {
    let timestamp = i128::from(timestamp);

    OffsetDateTime::from_unix_timestamp_nanos(timestamp * 1_000_000)
        .expect("provided value should be a valid unix timestamp in milliseconds")
}

pub fn date_time_to_js_timestamp(date_time: &OffsetDateTime) -> i64 {
    let timestamp_millis = date_time.unix_timestamp_nanos() / 1_000_000;

    i64::try_from(timestamp_millis)
        .expect("unix timestamp in milliseconds should not overflow an i64")
}

pub fn deserialize<'de, D>(deserializer: D) -> Result<OffsetDateTime, D::Error>
where
    D: Deserializer<'de>,
{
    i64::deserialize(deserializer).map(js_timestamp_to_date_time)
}

pub fn serialize<S>(date_time: &OffsetDateTime, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    serializer.serialize_i64(date_time_to_js_timestamp(date_time))
}

pub mod option {
    use serde::{Deserialize, Deserializer, Serializer};
    use time::OffsetDateTime;

    use super::{date_time_to_js_timestamp, js_timestamp_to_date_time};

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Option<OffsetDateTime>, D::Error>
    where
        D: Deserializer<'de>,
    {
        Ok(Option::<i64>::deserialize(deserializer)?.map(js_timestamp_to_date_time))
    }

    pub fn serialize<S>(
        date_time: &Option<OffsetDateTime>,
        serializer: S,
    ) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match date_time {
            Some(date_time) => serializer.serialize_some(&date_time_to_js_timestamp(date_time)),
            None => serializer.serialize_none(),
        }
    }
}
