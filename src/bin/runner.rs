extern crate restic_interfacer;

use restic_interfacer::{BackupTarget, ResticStorageConfig};
use std::path::PathBuf;

fn main() {
    let config = restic_interfacer::ResticConfig::new(
        "1234".into(),
        ResticStorageConfig::Local("./sample_repo".into()),
    );
    //	config.create_restic_repo().unwrap();
    //vec!["target/**/deps".to_owned(), "target/**/build".to_owned(), "target/**/incremental".to_owned(), ".git".to_owned()]
    let backup_tar =
        BackupTarget::new_from_string(&vec!["/mnt/d/"], Vec::new(), Vec::new()).unwrap();
    let gened = backup_tar.generate_files();
    dbg!(gened.size());
    //	let hi = gened.walk();
    //	dbg!(hi.len());

    loop {}
    //	config.restic_backup(&backup_tar).unwrap();
    //	config.backup_dry_run_simulator(&backup_tar).unwrap();
    //	let stuff  = config.restic_ls("0d9613ea").unwrap();
    //	dbg!(stuff);
}
