use serde::{Deserialize, Serialize, de::Visitor};

#[derive(Deserialize, Debug, Serialize, Clone, PartialEq, Eq, Copy)]
pub enum DatasetType {
    #[serde(alias = "FILESYSTEM")]
    FileSystem,
    #[serde(alias = "VOLUME")]
    Volume,
}

#[derive(Eq, Hash, PartialEq, Deserialize, Serialize, Debug, Clone)]
#[serde(tag = "type")]
pub enum PropertySource {
    #[serde(rename(deserialize = "LOCAL"))]
    Local { data: String },
    #[serde(rename(deserialize = "NONE"))]
    None { data: String },
    #[serde(rename(deserialize = "INHERITED"))]
    Inherited { data: String },
    #[serde(rename(deserialize = "DEFAULT"))]
    Default { data: String },
    #[serde(rename(deserialize = "TEMPORARY"))]
    Temporary { data: String },
    #[serde(rename(deserialize = "RECEIVED"))]
    Received { data: String },
}

impl PropertySource {
    pub fn is_local(&self) -> bool {
        match self {
            PropertySource::Local { .. } => true,
            PropertySource::Received { .. }
            | PropertySource::Temporary { .. }
            | PropertySource::None { .. }
            | PropertySource::Inherited { .. }
            | PropertySource::Default { .. } => false,
        }
    }
    pub fn user_managed(&self) -> bool {
        match self {
            PropertySource::Local { .. }
            | PropertySource::Inherited { .. }
            | PropertySource::Default { .. }
            | PropertySource::Received { .. } => true,
            PropertySource::Temporary { .. } | PropertySource::None { .. } => false,
        }
    }
}

#[derive(Debug, Clone, Eq)]
pub enum PropertyValue {
    Number(u64),
    String(String),
}

impl PropertyValue {
    pub fn to_string(&self) -> String {
        match self {
            PropertyValue::Number(num) => num.to_string(),
            PropertyValue::String(string) => string.clone(),
        }
    }
}

// According to zfs_nicestrtonum in zfs/lib/libzfs/libzfs_util.c
fn parse_number_with_suffix<S>(str: S) -> Option<u64>
where
    S: AsRef<str>,
{
    let str = str.as_ref();

    let (number_part, suffix_part) = match str.find(|c| !matches!(c, '0'..='9' | '.')) {
        Some(i) => str.split_at(i),
        None => (str, ""),
    };

    let shift = match suffix_part.to_ascii_uppercase().as_str() {
        "" | "B" => 0,
        "K" | "KB" | "KIB" => 10,
        "M" | "MB" | "MIB" => 20,
        "G" | "GB" | "GIB" => 30,
        "T" | "TB" | "TIB" => 40,
        "P" | "PB" | "PIB" => 50,
        "E" | "EB" | "EIB" => 60,
        "Z" | "ZB" | "ZIB" => 70,
        _ => {
            log::trace!("numeric value '{}' ends with an invalid suffix", str);
            return None;
        }
    };

    if number_part.contains('.') {
        let Ok(value): Result<f64, _> = number_part.parse() else {
            log::trace!("numeric value '{}' is bad", str);
            return None;
        };

        let scaled = value * ((1u64 << shift) as f64);

        if !scaled.is_finite() || scaled < 0.0 || scaled > u64::MAX as f64 {
            log::trace!("numeric value '{}' is too large", str);
            return None;
        }

        Some(scaled as u64)
    } else {
        let Ok(value): Result<u64, _> = number_part.parse() else {
            log::trace!("numeric value '{}' is bad", str);
            return None;
        };

        let Some(shifted) = value.checked_shl(shift as u32) else {
            log::trace!("numeric value '{}' is too large", str);
            return None;
        };

        Some(shifted)
    }
}

impl PartialEq for PropertyValue {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Number(s), Self::Number(o)) => s == o,
            (Self::String(s), Self::String(o)) => s == o,
            (Self::Number(s), Self::String(o)) => Some(s) == parse_number_with_suffix(o).as_ref(),
            (Self::String(s), Self::Number(o)) => parse_number_with_suffix(s).as_ref() == Some(o),
        }
    }
}

struct PropertyValueVisitor;

impl<'de> Visitor<'de> for PropertyValueVisitor {
    type Value = PropertyValue;

    fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
        formatter.write_str("either a numeric value or a string")
    }

    fn visit_u64<E>(self, v: u64) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        Ok(Self::Value::Number(v))
    }

    fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        Ok(Self::Value::String(v.to_owned()))
    }
}

impl<'de> Deserialize<'de> for PropertyValue {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        deserializer.deserialize_any(PropertyValueVisitor)
    }
}

impl Serialize for PropertyValue {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        match self {
            PropertyValue::Number(num) => num.serialize(serializer),
            PropertyValue::String(str) => str.serialize(serializer),
        }
    }
}

#[derive(Deserialize, Debug)]
pub struct Property {
    pub value: PropertyValue,
    pub source: PropertySource,
}
