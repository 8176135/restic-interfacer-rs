#![recursion_limit = "1024"]

mod errors;

use errors::*;
use std::process::Command;
use std::path::PathBuf;
use std::ffi::OsStr;

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
		let status = self.cmd_setup().arg("check")
			.status().chain_err(|| "Unable to spawn restic")?;

		println!("Status: {}", status);
		return Ok(status.success());
	}

	pub fn create_restic_repo(&self) -> Result<()> {
		let status = self.cmd_setup().arg("init")
			.status().chain_err(|| "Unable to spawn restic")?;

		println!("{}", status);
		if status.success() {
			Ok(())
		} else {
			Err(ErrorKind::ResticRepoNotFound.into())
		}
	}

	pub fn get_restic_snapshots(&self) -> Result<Vec<String>> {
		let mut cmd = self.cmd_setup();
		cmd.arg("--json");
		cmd.arg("snapshots");

		let out: std::process::Output = cmd.output().chain_err(|| "Failed to list snapshots")?;
		println!("Snapshot Out: {:?}", out);

		Ok(Vec::new())
	}

	pub fn restic_backup(&self, backup_targets: &BackupTarget) -> Result<()>{
		let mut cmd = self.cmd_setup();
		cmd.arg("--json");
		cmd.arg("backup");

		for folder in &backup_targets.folders {
			cmd.arg(folder);
		}

		for exclusion in &backup_targets.exclusions {
			cmd.arg("--exclude");
			cmd.arg(exclusion);
		}

		let out: std::process::Output = cmd.output().chain_err(|| "Failed to backup")?;

		println!("Backup Out: {:?}", out);

		Ok(())
	}
}

#[derive(Debug, Clone)]
pub struct BackupTarget {
	pub folders: Vec<PathBuf>,
	pub exclusions: Vec<String>,
}

#[cfg(test)]
mod tests {
	#[test]
	fn it_works() {
		assert_eq!(2 + 2, 4);
	}
}
