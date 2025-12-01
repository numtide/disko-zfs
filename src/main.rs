use std::{
    collections::HashMap,
    error::Error,
    io::{self, Read},
};

use serde::{de::Visitor, Deserialize};
use serde_derive::Deserialize;

#[derive(Deserialize, Debug)]
struct ZfsListOutputVersion {
    command: String,
    vers_major: i32,
    vers_minor: i32,
}

#[derive(Deserialize, Debug)]
enum DatasetType {
    #[serde(rename(deserialize = "FILESYSTEM"))]
    FileSystem,
    #[serde(rename(deserialize = "SNAPSHOT"))]
    Snapshot,
    #[serde(rename(deserialize = "ZVOL"))]
    Zvol,
}

#[derive(Debug, Clone)]
enum PropertyValue {
    Integer(i64),
    String(String),
}

struct PropertyValueVisitor;

impl<'de> Visitor<'de> for PropertyValueVisitor {
    type Value = PropertyValue;

    fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
        formatter.write_str("either a integer or string")
    }

    fn visit_i64<E>(self, v: i64) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        Ok(Self::Value::Integer(v))
    }

    fn visit_u64<E>(self, v: u64) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        Ok(Self::Value::Integer(v as i64))
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

#[derive(Deserialize, Debug)]
#[serde(tag = "type")]
enum PropertySource {
    #[serde(rename(deserialize = "LOCAL"))]
    Local { data: String },
    #[serde(rename(deserialize = "NONE"))]
    None { data: String },
    #[serde(rename(deserialize = "INHERITED"))]
    Inherited { data: String },
    #[serde(rename(deserialize = "DEFAULT"))]
    Default { data: String },
    #[serde(rename(deserialize = "TEMPORARY"))]
    TEMPORARY { data: String },
}

#[derive(Deserialize, Debug)]
struct Property {
    value: PropertyValue,
    source: PropertySource,
}

#[derive(Deserialize, Debug)]
struct Dataset {
    name: String,
    r#type: DatasetType,
    pool: String,
    createtxg: i32,
    properties: HashMap<String, Property>,
}

#[derive(Deserialize, Debug)]
struct ZfsListOutput {
    output_version: ZfsListOutputVersion,
    datasets: HashMap<String, Dataset>,
}

#[derive(Debug)]
enum ZfsAction {
    CreateDataset {
        name: String,
        properties: HashMap<String, PropertyValue>,
    },
    SetProperty {
        dataset: String,
        name: String,
        value: PropertyValue,
    },
}

struct ZfsSpecDataset {
    name: String,
    properties: HashMap<String, PropertyValue>,
}

struct ZfsSpec {
    datasets: Vec<ZfsSpecDataset>,
}

#[derive(Debug)]
struct VecActionProducer {
    actions: Vec<ZfsAction>,
    errors: Vec<String>,
}

impl VecActionProducer {
    fn new() -> VecActionProducer {
        VecActionProducer {
            actions: Vec::new(),
            errors: Vec::new(),
        }
    }
}

impl ActionProducer for VecActionProducer {
    fn produce_action(&mut self, action: ZfsAction) {
        self.actions.push(action)
    }

    fn produce_error(&mut self, error: String) {
        self.errors.push(error)
    }
}

trait ActionProducer {
    fn produce_action(&mut self, action: ZfsAction);
    fn produce_error(&mut self, error: String);
}

fn eval_spec<AP>(action_producer: &mut AP, state: &ZfsListOutput, spec: &ZfsSpec)
where
    AP: ActionProducer,
{
    for dataset in spec.datasets.iter() {
        if let Some(dataset_state) = state.datasets.get(&dataset.name) {
            for (property, value) in &dataset.properties {
                if let Some(property_state) = dataset_state.properties.get(property) {
                    match property_state.source {
                        PropertySource::Local { .. }
                        | PropertySource::Inherited { .. }
                        | PropertySource::Default { .. } => {
                            action_producer.produce_action(ZfsAction::SetProperty {
                                dataset: dataset.name.to_owned(),
                                name: property.to_owned(),
                                value: value.to_owned(),
                            })
                        }
                        _ => action_producer.produce_error(format!(
                            "cannot set property {} of dataset {} because source is {:?}",
                            property, dataset.name, property_state.source
                        )),
                    }
                } else {
                    action_producer.produce_action(ZfsAction::SetProperty {
                        dataset: dataset.name.to_owned(),
                        name: property.to_owned(),
                        value: value.to_owned(),
                    })
                }
            }
        } else {
            fn prefix_paths<I>(input: String) -> I
            where
                I: Iterator<Item = String>,
            {
                let input = input.rsplit("/").enumerate().map(|(_, i)| );
            }

            for dataset_part in dataset.name.rsplit("/").skip(1) {
                action_producer.produce_action(ZfsAction::CreateDataset {
                    name: dataset.name.to_owned(),
                    properties: HashMap::new(),
                })
            }

            action_producer.produce_action(ZfsAction::CreateDataset {
                name: dataset.name.to_owned(),
                properties: dataset.properties.to_owned(),
            })
        }
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    let zfs_spec = ZfsSpec {
        datasets: vec![
            ZfsSpecDataset {
                name: "zroot/test".to_string(),
                properties: HashMap::from(
                    [(":test", "test"), ("recordsize", "8k")]
                        .map(|(k, v)| (k.to_owned(), PropertyValue::String(v.to_owned()))),
                ),
            },
            ZfsSpecDataset {
                name: "zroot/ds1/persist/var/lib/postgresql".to_string(),
                properties: HashMap::from(
                    [
                        (":test", "test"),
                        ("recordsize", "16k"),
                        ("compressratio", "2.0"),
                    ]
                    .map(|(k, v)| (k.to_owned(), PropertyValue::String(v.to_owned()))),
                ),
            },
        ],
    };

    let mut buf = Vec::new();
    io::stdin().read_to_end(&mut buf)?;

    let zfs_list_output: ZfsListOutput = serde_json::from_slice(buf.as_slice())?;

    let mut ap = VecActionProducer::new();

    eval_spec(&mut ap, &zfs_list_output, &zfs_spec);

    println!("{:#?}", ap);

    Ok(())
}
