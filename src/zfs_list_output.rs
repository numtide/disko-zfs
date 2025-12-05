use serde::Deserialize;
use std::{
    collections::{HashMap, HashSet},
    io::Read,
    process::Command,
};

use crate::{
    property::{Property, PropertySource},
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

pub struct SpecificationFilter<F> {
    pub properties: Option<HashSet<String>>,
    pub property_sources: Option<F>,
}

impl Default for SpecificationFilter<fn(&PropertySource) -> bool> {
    fn default() -> Self {
        Self {
            properties: None,
            property_sources: None,
        }
    }
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
            None => go(
                "zfs",
                ["get", "all", "-t", "filesystem", "--json", "--json-int"].into_iter(),
            ),
        }
    }

    pub fn from_reader<R>(rdr: R) -> Result<ZfsList, serde_json::Error>
    where
        R: Read,
    {
        serde_json::from_reader(rdr)
    }

    pub fn into_specification<F>(self, filter: &SpecificationFilter<F>) -> ZfsSpecification
    where
        F: Fn(&PropertySource) -> bool,
    {
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
                                .filter(|(k, _)| match &filter.properties {
                                    Some(property_filter) => property_filter.contains(k),
                                    None => true,
                                })
                                .filter(|(_, v)| match &filter.property_sources {
                                    Some(source_filter) => source_filter(&v.source),
                                    None => true,
                                })
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
            ignored_datasets: Vec::new(),
            ignored_properties: Vec::new(),
        }
    }
}
