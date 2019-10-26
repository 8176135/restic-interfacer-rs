#![recursion_limit = "1024"]

mod errors;
mod restic_outputs;

use errors::*;
use restic_outputs::*;
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

	pub fn get_restic_snapshots(&self) -> Result<Vec<SnapshotsJson>> {
		let mut cmd = self.cmd_setup();
		cmd.arg("--json");
		cmd.arg("snapshots");

		Self::output_parsing(cmd.output().chain_err(|| "Failed to start restic")?, |stdout_data| {
			println!("\n{}\n", stdout_data);
			let val: Vec<SnapshotsJson> = serde_json::from_str(&stdout_data)
				.chain_err(|| "Failed to parse snapshots JSON, version not compatible?")?;
			Ok(val)
		})
	}

	pub fn restic_ls(&self, id: &str) -> Result<Vec<ListJson>> {
		let mut cmd = self.cmd_setup();
		cmd.arg("--json");
		cmd.arg("ls").arg(id);

		if !check_string_is_hex(id.trim()) {
			return Err(ErrorKind::InvalidId.into());
		}

		Self::output_parsing(cmd.output().chain_err(|| "Failed to start restic")?, |stdout_data| {
			let mut lines = stdout_data.lines().into_iter();
			let description_line = lines.next().ok_or(ErrorKind::Msg("No output from restic for ls".to_owned()))?;
			let val: SnapshotsJson = serde_json::from_str(description_line)
				.chain_err(|| "Failed to parse ls JSON, version not compatible?")?;

			lines.map(|line|
				serde_json::from_str(line)
					.chain_err(|| "Failed to parse ls JSON, version not compatible?")
			).collect()
		})
	}

	pub fn restic_backup(&self, backup_targets: &BackupTarget) -> Result<()> {
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
			cmd.arg("--exclude").arg(exclusion);
		}

		let out: std::process::Output = cmd.output().chain_err(|| "Failed to backup")?;

		println!("Backup Out: {:?}", out);

		Ok(())
	}

	fn output_parsing<T, F: FnOnce(std::borrow::Cow<str>) -> Result<T>>(output: std::process::Output, success_handler: F) -> Result<T> {
		if output.status.success() {
			success_handler(String::from_utf8_lossy(&output.stdout))
		} else {
			let error_msg = String::from_utf8_lossy(&output.stderr);
			if error_msg.contains("wrong password") {
				Err(ErrorKind::ResticRepoInvalidPassword.into())
			} else {
				Err(ErrorKind::Msg(format!("Output failed failed for unknown reasons: {}", error_msg)).into())
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

#[derive(Debug, Clone)]
pub struct BackupTarget {
	pub folders: Vec<PathBuf>,
	pub exclusions: Vec<String>,
	pub tags: Vec<String>,
}

#[cfg(test)]
mod tests {
	#[test]
	fn it_works() {
		assert_eq!(2 + 2, 4);
	}
}
