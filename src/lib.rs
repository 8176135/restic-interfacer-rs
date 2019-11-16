#![recursion_limit = "1024"]

mod errors;
mod restic_outputs;
mod backup_target;

use errors::*;

use restic_outputs::*;
pub use backup_target::*;
use serde::{Deserialize, Serialize};

use std::ffi::OsStr;
use std::path::PathBuf;
use std::process::Command;

const RESTIC_COMMAND: &str = "restic";
const RESTIC_PASSWORD_ENV: &str = "RESTIC_PASSWORD";
const RESTIC_REPO_FLAG: &str = "-r";

pub trait CreateRepoPath {
	fn create_path_string(&self) -> Box<dyn AsRef<OsStr>>;
	fn add_env_vars(&self, cmd: &mut Command) {}
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

	fn add_env_vars(&self, cmd: &mut Command) {
        match self {
            ResticStorageConfig::B2(b2_config) => b2_config.add_env_vars(cmd),
            _ => ()
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct B2Config {
	bucket_name: String,
	repo_path: String,
	account_key: String,
	account_id: String,
}

impl CreateRepoPath for B2Config {
	fn create_path_string(&self) -> Box<dyn AsRef<OsStr>> {
		Box::new(format!("b2:{}:{}", self.bucket_name, self.repo_path))
	}

	fn add_env_vars(&self, cmd: &mut Command) {
		cmd.env("B2_ACCOUNT_KEY", &self.account_key)
			.env("B2_ACCOUNT_ID", &self.account_id);
	}
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ForgetRate {
	pub keep_last: u32,
	pub keep_hourly: u32,
	pub keep_daily: u32,
	pub keep_weekly: u32,
	pub keep_monthly: u32,
	pub keep_yearly: u32,
	pub keep_tags: Vec<String>,
	pub keep_within: Option<std::time::Duration>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResticConfig {
	pub repo_password: String,
	pub repo_path: ResticStorageConfig,
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

		self.repo_path.add_env_vars(&mut cmd);
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
			cmd.output(),
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
			cmd.output(),
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
			cmd.output(),
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

	/// Run the forget command, tags format is the inner vec is ANDed and  the outer vec is ORed
	///
	/// tags are not implemented yet
	/// keep within not implemented yet
	pub fn forget(&self, forget_rate: &ForgetRate, _tags: Vec<Vec<String>>) -> Result<()> {
		let mut cmd = self.cmd_setup();
		cmd.arg("forget");
		if forget_rate.keep_hourly != 0 {
			cmd.arg("--keep-hourly").arg(forget_rate.keep_hourly.to_string());
		}

		if forget_rate.keep_daily != 0 {
			cmd.arg("--keep-daily").arg(forget_rate.keep_hourly.to_string());
		}

		if forget_rate.keep_weekly != 0 {
			cmd.arg("--keep-weekly").arg(forget_rate.keep_hourly.to_string());
		}

		if forget_rate.keep_monthly != 0 {
			cmd.arg("--keep-monthly").arg(forget_rate.keep_hourly.to_string());
		}

		if forget_rate.keep_yearly != 0 {
			cmd.arg("--keep-yearly").arg(forget_rate.keep_hourly.to_string());
		}

		if let Some(dur) = forget_rate.keep_within {
			cmd.arg("--keep-within").arg(format!(""));
//            dur.as_secs() * 60 *60
		}

		for keep_tag in &forget_rate.keep_tags {
			cmd.arg("--keep-tag").arg(keep_tag);
		}

		Self::output_parsing(cmd.output(), |_| Ok(()))
	}

//    fn convert_forget_tags_to_cmd(tags: &Vec<Vec<String>>) -> impl IntoIterator {
//        tags.iter().flat_map(|c| {
//            c.iter().map(|c| ).
//        });
//    }

	pub fn prune(&self) -> Result<()> {
		let mut cmd = self.cmd_setup();
		cmd.arg("prune");
		Self::output_parsing(cmd.output(), |_| Ok(()))
	}

	fn output_parsing<T, F: FnOnce(std::borrow::Cow<str>) -> Result<T>>(
		output: std::io::Result<std::process::Output>,
		success_handler: F,
	) -> Result<T> {
		let output = output.chain_err(|| "Failed to start restic")?;
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
