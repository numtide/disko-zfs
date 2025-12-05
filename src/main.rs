use glob::Pattern;
use log::{Level, LevelFilter};
use std::{
    collections::{HashMap, HashSet},
    fs::File,
    io::{Read, Write},
    iter::Filter,
    path::PathBuf,
    process::Command,
    str::{FromStr, MatchIndices},
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
    #[error("Couldn't write to stdout")]
    WriteStdoutFailed(#[source] std::io::Error),
    #[error("Couldn't serialize current ZFS specification to JSON")]
    SeriliazationJSONCurrentSpecFailed(#[source] serde_json::Error),
    #[error("Couldn't serialize current ZFS specification to Nix")]
    SeriliazationNixCurrentSpecFailed(#[source] ser_nix::Error),
}

use clap::Parser as _;

use crate::{
    prefix_paths::PrefixPaths,
    property::{PropertySource, PropertyValue},
    zfs_list_output::{SpecificationFilter, ZfsList},
    zfs_specification::{ZfsSpecification, ZfsSpecificationDataset},
};
mod prefix_paths;
mod property;
mod zfs_list_output;
mod zfs_specification;

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
    InheritProperties {
        dataset: String,
        properties: Vec<String>,
    },
}

#[derive(Debug)]
enum DestructiveAction {
    DestroyDataset { name: String },
}

#[derive(Debug)]
struct ActionSet {
    additive: Vec<ZfsAction>,
    destrictive: Vec<DestructiveAction>,
}

impl ActionSet {
    pub fn to_destructive_commands(&self) -> Vec<Vec<String>> {
        self.destrictive
            .iter()
            .map(|action| match action {
                DestructiveAction::DestroyDataset { name } => {
                    let mut output = Vec::with_capacity(3);
                    output.extend_from_slice(&["zfs", "destroy"].map(ToOwned::to_owned));
                    output.push(name.clone());
                    output
                }
            })
            .collect()
    }

    pub fn to_additive_commands(&self) -> Vec<Vec<String>> {
        self.additive
            .iter()
            .map(|action| match action {
                ZfsAction::CreateDataset { name, properties } => {
                    let mut output = Vec::with_capacity(3 + properties.len());
                    output.extend_from_slice(&["zfs", "create"].map(ToOwned::to_owned));
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
                    let mut vec = Vec::from(["zfs", "set"].map(ToOwned::to_owned));
                    vec.extend(
                        properties
                            .into_iter()
                            .map(|(name, value)| format!("{}={}", name, value.to_string())),
                    );
                    vec.push(dataset.to_owned());
                    vec
                }
                ZfsAction::InheritProperties {
                    dataset,
                    properties,
                } => {
                    let mut vec = Vec::from(["zfs", "inherit"].map(ToOwned::to_owned));
                    vec.extend(properties.into_iter().map(|s| s.clone()));
                    vec.push(dataset.to_owned());
                    vec
                }
            })
            .collect()
    }
}

#[derive(Debug)]
struct VecActionProducer {
    actions: Vec<ZfsAction>,
    destructive_actions: Vec<DestructiveAction>,
    errors: Vec<String>,
}

impl VecActionProducer {
    fn new() -> VecActionProducer {
        VecActionProducer {
            actions: Vec::new(),
            destructive_actions: Vec::new(),
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
                ZfsAction::SetProperties { .. } | ZfsAction::InheritProperties { .. } => {
                    [action].into_iter().collect()
                }
            })
            .collect::<Vec<_>>()
    }

    fn cleanup(&mut self) {
        self.actions = Self::cleanup_multiple_creates(std::mem::take(&mut self.actions));
    }

    fn finalize(mut self) -> (ActionSet, Vec<String>) {
        self.cleanup();
        (
            ActionSet {
                additive: self.actions,
                destrictive: self.destructive_actions,
            },
            self.errors,
        )
    }
}

impl ActionProducer for VecActionProducer {
    fn produce_action(&mut self, action: ZfsAction) {
        self.actions.push(action)
    }

    fn produce_destructive_action(&mut self, action: DestructiveAction) {
        self.destructive_actions.push(action)
    }

    fn produce_error(&mut self, error: String) {
        self.errors.push(error)
    }
}

trait ActionProducer {
    fn produce_action(&mut self, action: ZfsAction);
    fn produce_destructive_action(&mut self, action: DestructiveAction);
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

macro_rules! filter_by_pats {
    ( $iterator:expr, $pats:expr ) => {
        $iterator.filter(|(key, _)| $pats.iter().all(|pat| !pat.matches(key)))
    };
}

fn eval_spec<AP>(action_producer: &mut AP, actual: ZfsSpecification, desired: ZfsSpecification)
where
    AP: ActionProducer,
{
    fn filter_spec(
        spec: ZfsSpecification,
        ignored_datasets: Option<&Vec<Pattern>>,
        ignored_properties: Option<&Vec<Pattern>>,
    ) -> ZfsSpecification {
        ZfsSpecification {
            datasets: filter_by_pats!(
                spec.datasets.into_iter(),
                ignored_datasets.unwrap_or(&spec.ignored_datasets)
            )
            .map(|(k, v)| {
                (
                    k,
                    ZfsSpecificationDataset {
                        properties: filter_by_pats!(
                            v.properties.into_iter(),
                            ignored_properties.unwrap_or(&spec.ignored_properties)
                        )
                        .collect::<HashMap<_, _>>(),
                    },
                )
            })
            .collect::<HashMap<_, _>>(),
            ignored_datasets: spec.ignored_datasets,
            ignored_properties: spec.ignored_properties,
        }
    }

    let actual = filter_spec(
        actual,
        Some(&desired.ignored_datasets),
        Some(&desired.ignored_properties),
    );
    let desired = filter_spec(desired, None, None);

    let mut desired_datasets = desired.datasets.iter().collect::<Vec<_>>();
    desired_datasets.sort_by_key(|(key, _)| key.len());

    for (dataset_name, desired_dataset) in desired_datasets {
        if let Some(actual_dataset) = actual.get_dataset(dataset_name) {
            log::trace!("dataset {} already exists", dataset_name);

            let mut properties = HashMap::new();

            for (desired_property_name, desired_property) in &desired_dataset.properties {
                if let Some(actual_property) = actual_dataset.get_property(desired_property_name) {
                    if actual_property
                        .source
                        .as_ref()
                        .map(|p| p.user_managed())
                        .unwrap_or(false)
                    {
                        if actual_property.value != desired_property.value {
                            match (&actual_property.value, &desired_property.value) {
                                (PropertyValue::String(str), PropertyValue::Integer(int))
                                    if is_k_syntax(str, &int) =>
                                {
                                    log::trace!(
                                        "dataset {} property {} set to {}, guessing to be equal to {}, skip",
                                        dataset_name,
                                        desired_property_name,
                                        actual_property.value.to_string(),
                                        desired_property.value.to_string()
                                    );
                                }
                                (PropertyValue::Integer(int), PropertyValue::String(str))
                                    if is_k_syntax(str, &int) =>
                                {
                                    log::trace!(
                                        "dataset {} property {} set to {}, guessing to be equal to {}, skip",
                                        dataset_name,
                                        desired_property_name,
                                        actual_property.value.to_string(),
                                        desired_property.value.to_string()
                                    );
                                }
                                _ => {
                                    log::trace!(
                                        "dataset {} property {} set to {}, modify to {}",
                                        dataset_name,
                                        desired_property_name,
                                        actual_property.value.to_string(),
                                        desired_property.value.to_string()
                                    );
                                    properties.insert(
                                        desired_property_name.to_owned(),
                                        desired_property.to_owned(),
                                    );
                                }
                            }
                        } else {
                            log::trace!(
                                "dataset {} property {} already set to {}, skip",
                                dataset_name,
                                desired_property_name,
                                desired_property.value.to_string()
                            );
                        }
                    } else {
                        log::trace!(
                            "dataset {} property {} not normal, error",
                            dataset_name,
                            desired_property_name,
                        );
                        action_producer.produce_error(format!(
                            "cannot set property {} of dataset {} because source is {:?}",
                            desired_property_name, dataset_name, actual_property.source
                        ))
                    }
                } else {
                    log::trace!(
                        "dataset {} property {} not set, set to {}",
                        dataset_name,
                        desired_property_name,
                        desired_property.value.to_string(),
                    );
                    properties.insert(
                        desired_property_name.to_owned(),
                        desired_property.to_owned(),
                    );
                }
            }

            if !properties.is_empty() {
                action_producer.produce_action(ZfsAction::SetProperties {
                    dataset: dataset_name.to_owned(),
                    properties: properties
                        .into_iter()
                        .map(|(k, v)| (k, v.value))
                        .collect::<HashMap<_, _>>(),
                })
            }
        } else {
            log::trace!("prepare dataset {}", dataset_name);

            for dataset_part in PrefixPaths::new(&dataset_name) {
                if actual.get_dataset(dataset_part).is_none() {
                    log::trace!("create parent dataset {}", dataset_part);
                    action_producer.produce_action(ZfsAction::CreateDataset {
                        name: dataset_part.to_owned(),
                        properties: HashMap::new(),
                    })
                }
            }
            log::trace!(
                "create dataset {} with properties {}",
                dataset_name,
                desired_dataset
                    .properties
                    .iter()
                    .map(|(name, value)| format!("{}={}", name, value.value.to_string()))
                    .collect::<Vec<_>>()
                    .join(" ")
            );
            action_producer.produce_action(ZfsAction::CreateDataset {
                name: dataset_name.to_owned(),
                properties: desired_dataset
                    .properties
                    .iter()
                    .map(|(k, v)| (k.clone(), v.value.clone()))
                    .collect::<HashMap<_, _>>(),
            })
        }
    }

    for (dataset_name, actual_dataset) in &actual.datasets {
        match desired.datasets.get(dataset_name) {
            Some(desired_dataset) => {
                let mut inherited_properties: Vec<String> = Vec::new();

                for (property_name, actual_property) in &actual_dataset.properties {
                    if actual_property
                        .source
                        .as_ref()
                        .map_or(false, |source| source.is_local())
                        && desired_dataset.properties.get(property_name).is_none()
                    {
                        log::trace!(
                            "dataset {} inherit property {}",
                            dataset_name,
                            property_name
                        );
                        inherited_properties.push(property_name.clone())
                    }
                }

                if !inherited_properties.is_empty() {
                    action_producer.produce_action(ZfsAction::InheritProperties {
                        dataset: dataset_name.clone(),
                        properties: inherited_properties,
                    })
                }
            }
            None => {
                log::trace!("destroy dataset {}", dataset_name);
                action_producer.produce_destructive_action(DestructiveAction::DestroyDataset {
                    name: dataset_name.clone(),
                })
            }
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
    #[clap(flatten)]
    source: Source,
    #[arg(long = "log-level")]
    log_level: Option<Level>,
    #[command(subcommand)]
    command: Commands,
}

#[derive(Clone, clap::ValueEnum)]
enum CommandShowFormat {
    Json,
    Nix,
}

#[derive(Clone, clap::Subcommand)]
enum Commands {
    Plan {
        #[arg(short, long)]
        output: Option<PathBuf>,
        #[arg(short, long)]
        spec: PathBuf,
    },
    Apply {
        #[arg(short, long)]
        spec: PathBuf,
    },
    Show {
        #[arg(value_enum, short, long, default_value = "json")]
        format: CommandShowFormat,
        #[arg(short = 'p', long = "property")]
        properties: Vec<String>,
        #[arg(short = 'l', long = "local")]
        local: bool,
    },
}

fn get_actions(
    specification_file: &PathBuf,
    zfs_list_output: ZfsList,
) -> Result<ActionSet, ZfsDiskoError> {
    let zfs_specification = {
        let file = File::open(specification_file).map_err(ZfsDiskoError::SpecNotFound)?;
        ZfsSpecification::from_reader(file).map_err(ZfsDiskoError::InvalidSpec)?
    };

    let mut ap = VecActionProducer::new();

    eval_spec(
        &mut ap,
        zfs_list_output.into_specification(&Default::default()),
        zfs_specification,
    );

    let (actions, errors) = ap.finalize();

    for error in errors {
        log::error!("{}", error)
    }

    Ok(actions)
}

fn main() -> Result<(), ZfsDiskoError> {
    let mut stdout = std::io::stdout();

    let cli = Cli::parse();

    simple_logger::init_with_level(
        cli.log_level
            .or_else(|| {
                std::env::var("RUST_LOG")
                    .ok()
                    .and_then(|l| Level::from_str(&l).ok())
            })
            .unwrap_or(Level::Info),
    )
    .unwrap();

    let zfs_list_output: ZfsList = if let Some(file) = cli.source.file {
        let file = File::open(file).map_err(ZfsDiskoError::ZFSOutputNotFound)?;
        ZfsList::from_reader(file).map_err(ZfsDiskoError::InvalidZFSOutput)?
    } else {
        ZfsList::from_command::<Vec<_>, String>(None).map_err(ZfsDiskoError::ZFSCommandFailed)?
    };

    fn write_command<W, S>(
        output: &mut W,
        prefix: S,
        command: Vec<String>,
    ) -> Result<(), ZfsDiskoError>
    where
        W: Write,
        S: AsRef<str>,
    {
        write!(output, "{}{}\n", prefix.as_ref(), command.join(" "))
            .map_err(ZfsDiskoError::WriteStdoutFailed)?;
        Ok(())
    }

    match cli.command {
        Commands::Plan { spec, output } => {
            let actions = get_actions(&spec, zfs_list_output)?;

            let (mut output, prefix): (Box<dyn Write>, String) = if let Some(output) = output {
                (
                    Box::new(File::create(output).map_err(ZfsDiskoError::WriteStdoutFailed)?),
                    "".to_string(),
                )
            } else {
                (Box::new(stdout), "> ".to_string())
            };

            writeln!(&mut output, "# Additive Commands")
                .map_err(ZfsDiskoError::WriteStdoutFailed)?;

            for command in actions.to_additive_commands() {
                write_command(&mut output, &prefix, command)?;
            }

            writeln!(&mut output, "# !! Destructive Commands !!")
                .map_err(ZfsDiskoError::WriteStdoutFailed)?;

            for command in actions.to_destructive_commands() {
                write_command(&mut output, &prefix, command)?;
            }

            Ok(())
        }
        Commands::Apply { spec } => {
            let actions = get_actions(&spec, zfs_list_output)?;

            writeln!(stdout, "# !! Destructive Commands !!")
                .map_err(ZfsDiskoError::WriteStdoutFailed)?;

            for command in actions.to_destructive_commands() {
                write_command(&mut stdout, "> ", command)?;
            }

            for command in actions.to_additive_commands() {
                println!("+ {}", &command.join(" "));
                Command::new(&command[0])
                    .args(&command[1..])
                    .status()
                    .map_err(ZfsDiskoError::ZFSCommandFailed)?;
            }

            Ok(())
        }
        Commands::Show {
            format,
            properties,
            local,
        } => {
            let maybe_properties = if properties.is_empty() {
                None
            } else {
                Some(properties.into_iter().collect())
            };
            let current_spec = zfs_list_output.into_specification(&SpecificationFilter::<
                fn(&PropertySource) -> bool,
            > {
                properties: maybe_properties,
                property_sources: Some(if local {
                    |p| p.is_local()
                } else {
                    |p| p.user_managed()
                }),
            });

            match format {
                CommandShowFormat::Json => {
                    serde_json::to_writer_pretty(&mut stdout, &current_spec)
                        .map_err(ZfsDiskoError::SeriliazationJSONCurrentSpecFailed)?;
                    write!(stdout, "\n").map_err(ZfsDiskoError::WriteStdoutFailed)?;
                }
                CommandShowFormat::Nix => {
                    let nix_data = ser_nix::to_string(&current_spec)
                        .map_err(ZfsDiskoError::SeriliazationNixCurrentSpecFailed)?;

                    write!(stdout, "{}\n", nix_data).map_err(ZfsDiskoError::WriteStdoutFailed)?;
                }
            }

            Ok(())
        }
    }
}
