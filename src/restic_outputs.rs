
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