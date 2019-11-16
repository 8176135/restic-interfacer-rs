use serde::{Deserialize, Serialize, Serializer, Deserializer};
use globset::{Glob, GlobSet, GlobSetBuilder};
use std::ops::Deref;
use std::path::{Path, PathBuf};
use serde::de::Visitor;
use std::fmt;

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct MyGlob(Glob);

impl Deref for MyGlob {
	type Target = Glob;

	fn deref(&self) -> &Glob {
		&self.0
	}
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum BackupFileSelectionType {
	Irreverent,
	Contains,
	Included,
	Excluded,
}

impl Serialize for MyGlob {
	fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
		where
			S: Serializer,
	{
		serializer.serialize_str(&self.glob()[3..])
	}
}

struct MyGlobVisitor;

impl<'de> Visitor<'de> for MyGlobVisitor {
	type Value = MyGlob;

	fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
		formatter.write_str("A Unix shell Glob")
	}

	fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
		where
			E: serde::de::Error,
	{
		let modded = if value.starts_with("/") {
			value.trim_end_matches("/").to_owned()
		} else {
			"**/".to_owned() + value.trim_end_matches("/")
		};

		Ok(MyGlob(Glob::new(&modded).map_err(|_| E::custom(format!("String not glob")))?))
	}
}

impl<'de> Deserialize<'de> for MyGlob {
	fn deserialize<D>(deserializer: D) -> Result<MyGlob, D::Error>
		where
			D: Deserializer<'de>,
	{
		deserializer.deserialize_str(MyGlobVisitor)
	}
}

#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq, Default)]
pub struct BackupTarget {
	pub folders: Vec<PathBuf>,
	pub exclusions: Vec<MyGlob>,
	pub tags: Vec<String>,
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
			exclusions: exclusions.into_iter().map(|c| MyGlob(c)).collect(),
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
			builder.add(exclusion.clone().0);
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
			.any(|c| {
				path.as_ref()
					.canonicalize()
					.unwrap()
					.starts_with(c.as_path())
			})
		{
			let ex_set = self.get_exclusions_as_globset();

			if path.as_ref().ancestors().any(|c| {
				ex_set.is_match(c)
			}) {
				BackupFileSelectionType::Excluded
			} else {
				BackupFileSelectionType::Included
			}
		} else if self
			.folders
			.iter()
			.any(|c| {
				c.canonicalize()
					.map(|c| c.starts_with(path.as_ref()))
					.unwrap_or(false)
			}) {
			BackupFileSelectionType::Contains
		} else {
			BackupFileSelectionType::Irreverent
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
			}
		}

		store
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn check_serialize_deserialize() {
		let backup_tar =
			BackupTarget::new_from_string(&vec!["/mnt/d/", "/mnt/c/Windows/"], vec!["system32".to_owned()], vec!["abc".to_owned()]).unwrap();
		let out_tar: BackupTarget = serde_json::from_str(&serde_json::to_string(&backup_tar).unwrap()).unwrap();
		assert_eq!(backup_tar, out_tar);
	}
}
