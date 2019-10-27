
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct SnapshotsJson {
	hostname: String,
	id: String,
	parent: String,
	paths: Vec<String>,
	short_id: String,
	time: String,
	tree: String,
	username: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ListJson {
	atime: String,
	ctime: String,
	gid: i64,
	uid: i64,
	mode: i64,
	mtime: String,
	name: String,
	path: String,
	struct_type: String,
}
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "message_type")]
pub enum BackupJson {
	#[serde(rename = "summary")]
	Summary {
		files_new: u64,
		files_changed: u64,
		files_unmodified: u64,
		dirs_new: u64,
		dirs_changed: u64,
		dirs_unmodified: u64,
		data_blobs: u64,
		tree_blobs: u64,
		data_added: u64,
		total_files_processed: u64,
		total_bytes_processed: u64,
		total_duration: f64,
		snapshot_id: String,
	},
	#[serde(rename = "status")]
	Status {
		percent_done: f64,
		total_files: u64,
		total_bytes: u64,
	}
}