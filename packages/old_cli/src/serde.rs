pub mod serialize {
    use serde::Serializer;
    use std::fmt::Display;
    pub fn display<T, S>(value: &T, serializer: S) -> Result<S::Ok, S::Error>
    where
        T: Display,
        S: Serializer,
    {
        serializer.serialize_str(&value.to_string())
    }
}

pub mod deserialize {
    use serde::{Deserialize, Deserializer};
    use std::fmt::Display;
    use std::str::FromStr;
    pub fn fromstr<'de, T, D>(deserializer: D) -> Result<T, D::Error>
    where
        T: FromStr,
        T::Err: Display,
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        T::from_str(&s).map_err(serde::de::Error::custom)
    }
}
