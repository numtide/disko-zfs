use std::{
    collections::HashMap,
    fs::File,
    io::{Read, Write},
    path::PathBuf,
    process::Command,
    str::FromStr,
    str::MatchIndices,
};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ZfsDiskoError {
    #[error("ZFS specification not found")]
    SpecNotFound(#[source] std::io::Error),
    #[error("Invalid ZFS output")]
    InvalidZFSOutput(#[source] serde_json::Error),
    #[error("ZFS output file not found")]
    ZFSOutputNotFound(#[source] std::io::Error),
    #[error("ZFS command failed")]
    ZFSCommandFailed(#[source] std::io::Error),
    #[error("ZFS specification invalid")]
    InvalidSpec(#[source] serde_json::Error),
    #[error("Couldn't write plan output")]
    PlanOutputWriteFailed(#[source] std::io::Error),
}

use clap::Parser as _;
use serde::{Deserialize, Serialize, de::Visitor};
use serde_derive::{Deserialize, Serialize};

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

#[derive(Debug, Clone, PartialEq, Eq)]
enum PropertyValue {
    Integer(i64),
    String(String),
}

impl PropertyValue {
    pub fn to_string(&self) -> String {
        match self {
            PropertyValue::Integer(int) => int.to_string(),
            PropertyValue::String(string) => string.clone(),
        }
    }

    pub fn new_string<S>(str: S) -> PropertyValue
    where
        S: AsRef<str>,
    {
        PropertyValue::String(str.as_ref().to_owned())
    }

    pub fn new_integer(integer: i64) -> PropertyValue {
        PropertyValue::Integer(integer)
    }
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

impl Serialize for PropertyValue {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        match self {
            PropertyValue::Integer(int) => int.serialize(serializer),
            PropertyValue::String(str) => str.serialize(serializer),
        }
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
struct ZfsListOutput {
    output_version: ZfsListOutputVersion,
    datasets: HashMap<String, Dataset>,
}

impl ZfsListOutput {
    pub fn from_command<I, S>(command: Option<I>) -> Result<ZfsListOutput, std::io::Error>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        fn go<'a, I>(head: &str, tail: I) -> Result<ZfsListOutput, std::io::Error>
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

    pub fn from_reader<R>(rdr: R) -> Result<ZfsListOutput, serde_json::Error>
    where
        R: Read,
    {
        serde_json::from_reader(rdr)
    }

    pub fn get_dataset<'a, S>(&'a self, name: S) -> Option<&'a Dataset>
    where
        S: AsRef<str>,
    {
        self.datasets.get(name.as_ref())
    }

    pub fn get_dataset_mut<'a, S>(&'a mut self, name: S) -> Option<&'a mut Dataset>
    where
        S: AsRef<str>,
    {
        self.datasets.get_mut(name.as_ref())
    }
}

#[derive(Debug)]
enum ZfsAction {
    CreateDataset {
        name: String,
        properties: HashMap<String, PropertyValue>,
    },
    SetProperties {
        dataset: String,
        properties: HashMap<String, PropertyValue>,
    },
}

#[derive(Deserialize, Serialize)]
struct ZfsSpecDataset {
    properties: HashMap<String, PropertyValue>,
}

impl ZfsSpecDataset {
    pub fn new<S>(properties: HashMap<S, PropertyValue>) -> ZfsSpecDataset
    where
        S: AsRef<str>,
    {
        ZfsSpecDataset {
            properties: properties
                .into_iter()
                .map(|(k, v)| (k.as_ref().to_owned(), v))
                .collect(),
        }
    }
}

#[derive(Deserialize, Serialize)]
struct ZfsSpec {
    datasets: HashMap<String, ZfsSpecDataset>,
}

impl ZfsSpec {
    pub fn from_reader<R>(rdr: R) -> Result<ZfsSpec, serde_json::Error>
    where
        R: Read,
    {
        serde_json::from_reader(rdr)
    }
}

#[derive(Debug)]
struct ActionSet(Vec<ZfsAction>);

impl ActionSet {
    pub fn to_commands(&self) -> Vec<Vec<String>> {
        self.0
            .iter()
            .map(|action| match action {
                ZfsAction::CreateDataset { name, properties } => {
                    let mut output = Vec::with_capacity(3 + properties.len());
                    output.extend_from_slice(&["zfs", "create"].map(|x| x.to_owned()));
                    output.extend(
                        properties
                            .iter()
                            .map(|(name, value)| format!("-o{}={}", name, value.to_string())),
                    );
                    output.push(name.to_owned());
                    output
                }
                ZfsAction::SetProperties {
                    dataset,
                    properties,
                } => {
                    vec![
                        "zfs".to_owned(),
                        "set".to_owned(),
                        properties
                            .into_iter()
                            .map(|(name, value)| format!("{}={}", name, value.to_string()))
                            .collect::<Vec<_>>()
                            .join(" "),
                        dataset.to_owned(),
                    ]
                }
            })
            .collect()
    }
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

    fn cleanup_multiple_creates(actions: Vec<ZfsAction>) -> Vec<ZfsAction> {
        let mut known_datasets: HashMap<String, HashMap<String, PropertyValue>> = HashMap::new();

        actions
            .into_iter()
            .flat_map::<Box<[ZfsAction]>, _>(|action| match &action {
                ZfsAction::CreateDataset { name, properties } => {
                    log::trace!("optimizing {:?}", action);
                    let mut edited_properties = HashMap::new();

                    if let Some(existing_properties) = known_datasets.get(name) {
                        log::trace!("known dateset {}", name);
                        for (name, value) in properties {
                            if let Some(existing_value) = existing_properties.get(name) {
                                if existing_value != value {
                                    log::trace!(
                                        "existing property {} {} != {}",
                                        name,
                                        existing_value.to_string(),
                                        value.to_string()
                                    );
                                    edited_properties.insert(name.clone(), value.clone());
                                } else {
                                    log::trace!(
                                        "existing property {} {} == {}",
                                        name,
                                        existing_value.to_string(),
                                        value.to_string()
                                    );
                                }
                            } else {
                                log::trace!("adding property {}={}", name, value.to_string());
                                edited_properties.insert(name.clone(), value.clone());
                            }
                        }
                        for (property_name, property_value) in &edited_properties {
                            log::trace!(
                                "dataset {} setting {}={}",
                                name,
                                property_name,
                                property_value.to_string()
                            )
                        }

                        if !edited_properties.is_empty() {
                            [ZfsAction::SetProperties {
                                dataset: name.clone(),
                                properties: edited_properties,
                            }]
                            .into_iter()
                            .collect()
                        } else {
                            [].into_iter().collect()
                        }
                    } else {
                        log::trace!("new dateset {}, keeping", name);
                        known_datasets.insert(name.clone(), properties.clone());
                        [action].into_iter().collect()
                    }
                }
                ZfsAction::SetProperties { .. } => [action].into_iter().collect(),
            })
            .collect::<Vec<_>>()
    }

    fn cleanup(&mut self) {
        self.actions = Self::cleanup_multiple_creates(std::mem::take(&mut self.actions));
    }

    fn finalize(mut self) -> (ActionSet, Vec<String>) {
        self.cleanup();
        (ActionSet(self.actions), self.errors)
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

fn is_k_syntax<S>(str: S, int: &i64) -> bool
where
    S: AsRef<str>,
{
    let str = str.as_ref();
    let beginning = (str.ends_with("K") || str.ends_with("k"));
    let end = i64::from_str(&str[..str.len() - 1])
        .map(|parsed| parsed * 1024)
        .unwrap_or(0);

    log::trace!("beginning {} end {}", beginning, end);

    (str.ends_with("K") || str.ends_with("k"))
        && i64::from_str(&str[..str.len() - 1])
            .map(|parsed| parsed * 1024 == *int)
            .unwrap_or(false)
}

fn eval_spec<AP>(action_producer: &mut AP, state: &ZfsListOutput, spec: &ZfsSpec)
where
    AP: ActionProducer,
{
    let mut datasets = spec.datasets.iter().collect::<Vec<_>>();
    datasets.sort_by_key(|(key, _)| key.len());

    for (name, dataset) in datasets {
        if let Some(dataset_state) = state.get_dataset(name) {
            log::trace!("dataset {} already exists", name);

            let mut properties = HashMap::new();

            for (property, value) in &dataset.properties {
                if let Some(property_state) = dataset_state.get_property(property) {
                    match property_state.source {
                        PropertySource::Local { .. }
                        | PropertySource::Inherited { .. }
                        | PropertySource::Default { .. } => {
                            if property_state.value != *value {
                                match (&property_state.value, value) {
                                    (PropertyValue::String(str), PropertyValue::Integer(int))
                                        if is_k_syntax(str, int) =>
                                    {
                                        log::trace!(
                                            "dataset {} property {} set to {}, guessing to be equal to {}, skip",
                                            name,
                                            property,
                                            property_state.value.to_string(),
                                            value.to_string()
                                        );
                                    }
                                    (PropertyValue::Integer(int), PropertyValue::String(str))
                                        if is_k_syntax(str, int) =>
                                    {
                                        log::trace!(
                                            "dataset {} property {} set to {}, guessing to be equal to {}, skip",
                                            name,
                                            property,
                                            property_state.value.to_string(),
                                            value.to_string()
                                        );
                                    }
                                    _ => {
                                        log::trace!(
                                            "dataset {} property {} set to {}, modify to {}",
                                            name,
                                            property,
                                            property_state.value.to_string(),
                                            value.to_string()
                                        );
                                        properties.insert(property.to_owned(), value.to_owned());
                                    }
                                }
                            } else {
                                log::trace!(
                                    "dataset {} property {} already set to {}, skip",
                                    name,
                                    property,
                                    value.to_string()
                                );
                            }
                        }
                        _ => {
                            log::trace!("dataset {} property {} not normal, error", name, property,);
                            action_producer.produce_error(format!(
                                "cannot set property {} of dataset {} because source is {:?}",
                                property, name, property_state.source
                            ))
                        }
                    }
                } else {
                    log::trace!(
                        "dataset {} property {} not set, set to {}",
                        name,
                        property,
                        value.to_string(),
                    );
                    properties.insert(property.to_owned(), value.to_owned());
                }
            }

            if !properties.is_empty() {
                action_producer.produce_action(ZfsAction::SetProperties {
                    dataset: name.to_owned(),
                    properties,
                })
            }
        } else {
            log::trace!("prepare dataset {}", name);

            struct PrefixPaths<'a>(&'a str, MatchIndices<'a, char>);

            impl<'a> PrefixPaths<'a> {
                fn new(string: &'a str) -> PrefixPaths<'a> {
                    PrefixPaths(string, string.match_indices('/'))
                }
            }

            impl<'a> Iterator for PrefixPaths<'a> {
                type Item = &'a str;

                fn next(&mut self) -> Option<Self::Item> {
                    self.1.next().map(|(index, _)| &self.0[0..index])
                }
            }

            for dataset_part in PrefixPaths::new(&name) {
                if state.get_dataset(dataset_part).is_none() {
                    log::trace!("create parent dataset {}", dataset_part);
                    action_producer.produce_action(ZfsAction::CreateDataset {
                        name: dataset_part.to_owned(),
                        properties: HashMap::new(),
                    })
                }
            }
            log::trace!(
                "create dataset {} with properties {}",
                name,
                dataset
                    .properties
                    .iter()
                    .map(|(name, value)| format!("{}={}", name, value.to_string()))
                    .collect::<Vec<_>>()
                    .join(" ")
            );
            action_producer.produce_action(ZfsAction::CreateDataset {
                name: name.to_owned(),
                properties: dataset.properties.to_owned(),
            })
        }
    }
}

#[derive(clap::Args, Clone)]
struct Source {
    #[clap(short, long)]
    file: Option<PathBuf>,
}

#[derive(clap::Parser)]
#[command(name = "disko-zfs")]
struct Cli {
    #[arg(short, long)]
    spec: PathBuf,
    #[clap(flatten)]
    source: Source,
    #[command(subcommand)]
    command: Commands,
}

#[derive(Clone, clap::Subcommand)]
enum Commands {
    Plan {
        #[arg(short, long)]
        output: Option<PathBuf>,
    },
    Apply,
}

fn main() -> Result<(), ZfsDiskoError> {
    simple_logger::SimpleLogger::new().env().init().unwrap();

    let cli = Cli::parse();

    // let zfs_spec = ZfsSpec {
    //     datasets: vec![
    //         ZfsSpecDataset::new(
    //             "zroot/test",
    //             HashMap::from(
    //                 [
    //                     (":test", PropertyValue::new_string("test")),
    //                     ("recordsize", PropertyValue::new_integer(8192)),
    //                 ]
    //                 .map(|(k, v)| (k.to_owned(), v)),
    //             ),
    //         ),
    //         ZfsSpecDataset::new(
    //             "zroot/test",
    //             HashMap::from(
    //                 [
    //                     (":test", PropertyValue::new_string("test")),
    //                     ("recordsize", PropertyValue::new_integer(16384)),
    //                 ]
    //                 .map(|(k, v)| (k.to_owned(), v)),
    //             ),
    //         ),
    //         ZfsSpecDataset::new(
    //             "zroot/ds1/persist/var/lib/postgresql",
    //             HashMap::from(
    //                 [
    //                     (":test", PropertyValue::new_string("test")),
    //                     ("recordsize", PropertyValue::new_integer(16384)),
    //                     ("compressratio", PropertyValue::new_string("2.0")),
    //                 ]
    //                 .map(|(k, v)| (k.to_owned(), v)),
    //             ),
    //         ),
    //         ZfsSpecDataset::new("zroot/test/test", HashMap::<String, _>::new()),
    //     ],
    // };
    // serde_json::to_writer_pretty(std::io::stdout(), &zfs_spec)?;

    let zfs_list_output: ZfsListOutput = if let Some(file) = cli.source.file {
        let file = File::open(file).map_err(ZfsDiskoError::ZFSOutputNotFound)?;
        ZfsListOutput::from_reader(file).map_err(ZfsDiskoError::InvalidZFSOutput)?
    } else {
        ZfsListOutput::from_command::<Vec<_>, String>(None)
            .map_err(ZfsDiskoError::ZFSCommandFailed)?
    };

    let zfs_spec = {
        let file = File::open(cli.spec).map_err(ZfsDiskoError::SpecNotFound)?;
        ZfsSpec::from_reader(file).map_err(ZfsDiskoError::InvalidSpec)?
    };

    let mut ap = VecActionProducer::new();

    eval_spec(&mut ap, &zfs_list_output, &zfs_spec);

    let (actions, errors) = ap.finalize();
    let commands = actions.to_commands();

    for error in errors {
        log::error!("{}", error)
    }

    match cli.command {
        Commands::Plan { output } => {
            if let Some(output) = output {
                let mut output =
                    File::create(output).map_err(ZfsDiskoError::PlanOutputWriteFailed)?;

                for command in commands {
                    write!(&mut output, "{}\n", command.join(" "))
                        .map_err(ZfsDiskoError::PlanOutputWriteFailed)?;
                }
            } else {
                for command in commands {
                    println!("> {}", command.join(" "))
                }
            }

            Ok(())
        }
        Commands::Apply => {
            for command in commands {
                println!("+ {}", &command.join(" "));
                Command::new(&command[0])
                    .args(&command[1..])
                    .status()
                    .map_err(ZfsDiskoError::ZFSCommandFailed)?;
            }

            Ok(())
        }
    }
}
