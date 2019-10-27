extern crate restic_interfacer;

use restic_interfacer::{ResticStorageConfig, BackupTarget};

fn main() {
	let config = restic_interfacer::ResticConfig::new("1234".into(), ResticStorageConfig::Local("./sample_repo".into()));
//	config.create_restic_repo().unwrap();
	let backup_tar = BackupTarget {
		exclusions: Vec::new(),
		folders: vec!["./src".into()],
		tags: Vec::new()
	};
//	config.restic_backup(&backup_tar).unwrap();
	config.backup_dry_run_simulator(&backup_tar).unwrap();
//	let stuff  = config.restic_ls("0d9613ea").unwrap();
//	dbg!(stuff);
}