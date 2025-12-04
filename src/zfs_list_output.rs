use serde::{Deserialize, Serialize, de::Visitor};
use serde_derive::Deserialize;
use std::{collections::HashMap, io::Read, process::Command};

use crate::{
    property::Property,
    zfs_specification::{self, ZfsSpecification, ZfsSpecificationDataset},
};

#[derive(Deserialize, Debug)]
struct ZfsListVersion {
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

#[derive(Deserialize, Debug)]
struct Dataset {
    name: String,
    r#type: DatasetType,
    pool: String,
    createtxg: i32,
    properties: HashMap<String, Property>,
}

impl Dataset {
    pub fn get_property<'a, S>(&'a self, name: S) -> Option<&'a Property>
    where
        S: AsRef<str>,
    {
        self.properties.get(name.as_ref())
    }

    pub fn get_property_mut<'a, S>(&'a mut self, name: S) -> Option<&'a mut Property>
    where
        S: AsRef<str>,
    {
        self.properties.get_mut(name.as_ref())
    }
}

#[derive(Deserialize, Debug)]
pub struct ZfsList {
    pub output_version: ZfsListVersion,
    pub datasets: HashMap<String, Dataset>,
}

impl ZfsList {
    pub fn from_command<I, S>(command: Option<I>) -> Result<ZfsList, std::io::Error>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        fn go<'a, I>(head: &str, tail: I) -> Result<ZfsList, std::io::Error>
        where
            I: Iterator<Item = &'a str>,
        {
            Command::new(head)
                .args(tail)
                .output()
                .and_then(|output| serde_json::from_slice(&output.stdout).map_err(|err| err.into()))
        }

        match command {
            Some(command) => {
                let iter1 = command.into_iter().collect::<Vec<S>>();
                let mut iter = iter1.iter().map(|s| s.as_ref());
                go(iter.next().unwrap(), iter)
            }
            None => go("zfs", ["get", "all", "--json", "--json-int"].into_iter()),
        }
    }

    pub fn from_reader<R>(rdr: R) -> Result<ZfsList, serde_json::Error>
    where
        R: Read,
    {
        serde_json::from_reader(rdr)
    }

    pub fn into_specification(self) -> ZfsSpecification {
        ZfsSpecification {
            datasets: self
                .datasets
                .into_iter()
                .map(|(k, v)| {
                    (
                        k,
                        ZfsSpecificationDataset {
                            properties: v
                                .properties
                                .into_iter()
                                .map(|(k, v)| {
                                    (
                                        k,
                                        zfs_specification::Property {
                                            value: v.value,
                                            source: Some(v.source),
                                        },
                                    )
                                })
                                .collect::<HashMap<_, _>>(),
                        },
                    )
                })
                .collect::<HashMap<_, _>>(),
        }
    }
}
