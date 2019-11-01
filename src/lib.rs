#![recursion_limit = "1024"]

mod errors;
mod restic_outputs;
mod backup_target;

use errors::*;
use globset::{Glob, GlobSet, GlobSetBuilder};
use restic_outputs::*;
pub use backup_target::*;
use serde::{Deserialize, Serialize, Serializer, Deserializer};

use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::process::Command;

const RESTIC_COMMAND: &str = "restic";
const RESTIC_PASSWORD_ENV: &str = "RESTIC_PASSWORD";
const RESTIC_REPO_FLAG: &str = "-r";

pub trait CreateRepoPath {
    fn create_path_string(&self) -> Box<dyn AsRef<OsStr>>;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct B2Config {
    bucket_name: String,
    repo_path: String,
}

impl CreateRepoPath for B2Config {
    fn create_path_string(&self) -> Box<dyn AsRef<OsStr>> {
        Box::new(format!("b2:{}:{}", self.bucket_name, self.repo_path))
    }
}

#[derive(Debug, Clone, Serialize)]
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
                let _val: SnapshotsJson = serde_json::from_str(description_line)
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



#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}
