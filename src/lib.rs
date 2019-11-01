#![recursion_limit = "1024"]

mod errors;
mod restic_outputs;

use errors::*;
use globset::{Glob, GlobSet, GlobSetBuilder};
use restic_outputs::*;
use std::collections::HashSet;
use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::process::Command;

const RESTIC_COMMAND: &str = "restic";
const RESTIC_PASSWORD_ENV: &str = "RESTIC_PASSWORD";
const RESTIC_REPO_FLAG: &str = "-r";

pub trait CreateRepoPath {
    fn create_path_string(&self) -> Box<dyn AsRef<OsStr>>;
}

#[derive(Debug, Clone)]
pub enum ResticStorageConfig {
    Local(PathBuf),
    B2(B2Config),
}

impl CreateRepoPath for ResticStorageConfig {
    fn create_path_string(&self) -> Box<dyn AsRef<OsStr>> {
        match self {
            ResticStorageConfig::Local(path) => Box::new(path.clone()),
            ResticStorageConfig::B2(b2_config) => b2_config.create_path_string(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct B2Config {
    bucket_name: String,
    repo_path: String,
}

impl CreateRepoPath for B2Config {
    fn create_path_string(&self) -> Box<dyn AsRef<OsStr>> {
        Box::new(format!("b2:{}:{}", self.bucket_name, self.repo_path))
    }
}

#[derive(Debug, Clone)]
pub struct ResticConfig {
    repo_password: String,
    repo_path: ResticStorageConfig,
}

impl ResticConfig {
    pub fn new(repo_password: String, repo_path: ResticStorageConfig) -> ResticConfig {
        ResticConfig {
            repo_password,
            repo_path,
        }
    }

    fn cmd_setup(&self) -> Command {
        let mut cmd = Command::new(RESTIC_COMMAND);

        cmd.env(RESTIC_PASSWORD_ENV, &self.repo_password)
            .arg(RESTIC_REPO_FLAG);

        cmd.arg(&*self.repo_path.create_path_string());

        cmd
    }

    pub fn check_restic_repo(&self) -> Result<bool> {
        let status = self
            .cmd_setup()
            .arg("check")
            .status()
            .chain_err(|| "Unable to spawn restic")?;

        println!("Status: {}", status);
        return Ok(status.success());
    }

    pub fn create_restic_repo(&self) -> Result<()> {
        let status = self
            .cmd_setup()
            .arg("init")
            .status()
            .chain_err(|| "Unable to spawn restic")?;

        println!("{}", status);
        if status.success() {
            Ok(())
        } else {
            Err(ErrorKind::ResticRepoNotFound.into())
        }
    }

    pub fn get_restic_snapshots(&self) -> Result<Vec<SnapshotsJson>> {
        let mut cmd = self.cmd_setup();
        cmd.arg("--json");
        cmd.arg("snapshots");

        Self::output_parsing(
            cmd.output().chain_err(|| "Failed to start restic")?,
            |stdout_data| {
                println!("\n{}\n", stdout_data);
                let val: Vec<SnapshotsJson> = serde_json::from_str(&stdout_data)
                    .chain_err(|| "Failed to parse snapshots JSON, version not compatible?")?;
                Ok(val)
            },
        )
    }

    pub fn restic_ls(&self, id: &str) -> Result<Vec<ListJson>> {
        let mut cmd = self.cmd_setup();
        cmd.arg("--json");
        cmd.arg("ls").arg(id);

        if !check_string_is_hex(id.trim()) {
            return Err(ErrorKind::InvalidId.into());
        }

        Self::output_parsing(
            cmd.output().chain_err(|| "Failed to start restic")?,
            |stdout_data| {
                let mut lines = stdout_data.lines().into_iter();
                let description_line = lines
                    .next()
                    .ok_or::<Error>(ErrorKind::NoOutputFromRestic.into())?;
                let val: SnapshotsJson = serde_json::from_str(description_line)
                    .chain_err(|| "Failed to parse ls JSON, version not compatible?")?;

                lines
                    .map(|line| {
                        serde_json::from_str(line)
                            .chain_err(|| "Failed to parse ls JSON, version not compatible?")
                    })
                    .collect()
            },
        )
    }

    pub fn restic_backup(&self, backup_targets: &BackupTarget) -> Result<BackupJson> {
        let mut cmd = self.cmd_setup();
        cmd.arg("--json");
        cmd.arg("backup");

        for tag in &backup_targets.tags {
            cmd.arg("--tag").arg(tag);
        }

        for folder in &backup_targets.folders {
            cmd.arg(folder);
        }

        for exclusion in &backup_targets.exclusions {
            cmd.arg("--exclude").arg(exclusion.glob());
        }

        Self::output_parsing(
            cmd.output()
                .chain_err(|| "Failed to launch restic for backup")?,
            |stdout_data| {
                let mut lines = stdout_data.lines();
                let mut val: BackupJson;
                while {
                    let result_line = lines
                        .next_back()
                        .ok_or::<Error>(ErrorKind::NoOutputFromRestic.into())?;
                    val = serde_json::from_str(result_line).chain_err(|| {
                        format!(
                            "Failed to parse backup JSON, version not compatible? Out: {}",
                            result_line
                        )
                    })?;
                    match val {
                        BackupJson::Status { .. } => true,
                        BackupJson::Summary { .. } => false,
                    }
                } {}

                Ok(val)
            },
        )
    }
    //
    //	pub fn generate_files_for_backup_target<P: AsRef<Path>>(&self, backup_targets: &BackupTarget) -> Result<filepath_tree::PathStore> {
    //
    //	}

    fn output_parsing<T, F: FnOnce(std::borrow::Cow<str>) -> Result<T>>(
        output: std::process::Output,
        success_handler: F,
    ) -> Result<T> {
        if output.status.success() {
            success_handler(String::from_utf8_lossy(&output.stdout))
        } else {
            let error_msg = String::from_utf8_lossy(&output.stderr);
            if error_msg.contains("wrong password") {
                Err(ErrorKind::ResticRepoInvalidPassword.into())
            } else {
                Err(ErrorKind::Msg(format!(
                    "Output failed failed for unknown reasons: {}",
                    error_msg
                ))
                .into())
            }
        }
    }
}

fn check_string_is_hex(input: &str) -> bool {
    for c in input.chars() {
        match c {
            '0'..='9' | 'a'..='f' => (),
            _ => return false,
        }
    }

    true
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum BackupFileSelectionType {
    NotATarget,
    Included,
    Excluded,
}

#[derive(Debug, Clone)]
pub struct BackupTarget {
    folders: Vec<PathBuf>,
    exclusions: Vec<Glob>,
    tags: Vec<String>,
}

impl BackupTarget {
    pub fn new<P: AsRef<Path>>(folders: &[P], exclusions: Vec<Glob>, tags: Vec<String>) -> Self {
        Self {
            folders: folders
                .iter()
                .map(|c| {
                    c.as_ref()
                        .canonicalize()
                        .expect("Failed to canonicalize path, path does not exist most likely")
                })
                .collect(),
            tags,
            exclusions,
        }
    }

    pub fn new_from_string<P: AsRef<Path>>(
        folders: &[P],
        exclusions: Vec<String>,
        tags: Vec<String>,
    ) -> std::result::Result<Self, globset::Error> {
        Ok(Self::new(
            folders,
            exclusions
                .iter()
                .map(|c| Glob::new(&format!("**/{}", c)))
                .collect::<std::result::Result<Vec<Glob>, globset::Error>>()?,
            tags,
        ))
    }

    pub fn get_exclusions_as_globset(&self) -> GlobSet {
        let mut builder = GlobSetBuilder::new();
        for exclusion in &self.exclusions {
            builder.add(exclusion.clone());
        }
        builder.build().unwrap()
    }

    pub fn add_folder<P: AsRef<Path>>(&mut self, folder_path: P) {
        self.folders.push(
            folder_path
                .as_ref()
                .canonicalize()
                .expect("Failed to canonicalize path, not sure when this happens"),
        )
    }

    pub fn check_path_is_in_backup<P: AsRef<Path>>(&self, path: P) -> BackupFileSelectionType {
        if self
            .folders
            .iter()
            .find(|c| {
                path.as_ref()
                    .canonicalize()
                    .unwrap()
                    .starts_with(c.as_path())
            })
            .is_some()
        {
            let ex_set = self.get_exclusions_as_globset();

            if path.as_ref().ancestors().any(|c| ex_set.is_match(c)) {
                BackupFileSelectionType::Excluded
            } else {
                BackupFileSelectionType::Included
            }
        } else {
            BackupFileSelectionType::NotATarget
        }
    }
    pub fn generate_files(&self) -> filepath_tree::PathStore<()> {
        let mut store = filepath_tree::PathStore::new(None);
        let ex_set = self.get_exclusions_as_globset();

        for folder in &self.folders {
            let mut walk = walkdir::WalkDir::new(&folder)
                .follow_links(false)
                .into_iter();

            while let Some(entry) = walk.next() {
                let entry = match entry {
                    Ok(c) => c,
                    Err(err) => {
                        eprintln!("Error while walking: {}", err);
                        continue;
                    }
                };

                if ex_set.is_match(entry.path()) {
                    println!("Excluded path found: {}", entry.path().display());
                    walk.skip_current_dir();
                    continue;
                }
                store
                    .add_path(entry.path(), None)
                    .expect("Failed to add to store");
                //				dbg!(&entry);
            }
        }

        store
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}
