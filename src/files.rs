//! Bounded, uncompressed file selections. Native clipboard access belongs to desktop.
mod filesystem;
use eyre::{Result, ensure};
pub use filesystem::{Materialized, Rename, capture, materialize, publish_directory};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

pub const MAX_ENTRIES: usize = 4096;
pub const MAX_DEPTH: usize = 64;
pub const MAX_MANIFEST: usize = 1024 * 1024;
const MAGIC: &[u8; 8] = b"SCBFILES";
const HEADER: usize = 16;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    File,
    Directory,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Entry {
    pub parent: Option<usize>,
    pub name: String,
    pub kind: Kind,
    pub length: u32,
}

pub struct Bundle<'a> {
    pub entries: Vec<Entry>,
    pub data: &'a [u8],
}

impl Bundle<'_> {
    pub fn label(&self) -> String {
        let files = self
            .entries
            .iter()
            .filter(|entry| entry.kind == Kind::File)
            .count();
        let folders = self.entries.len() - files;
        format!(
            "{files} {}, {folders} {}",
            if files == 1 { "file" } else { "files" },
            if folders == 1 { "folder" } else { "folders" }
        )
    }
    pub fn preview(&self) -> String {
        let names = self
            .entries
            .iter()
            .filter(|entry| entry.parent.is_none())
            .take(20)
            .map(|entry| {
                entry
                    .name
                    .chars()
                    .filter(|c| !c.is_control())
                    .take(80)
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n");
        format!("{}\n{} bytes\n{names}", self.label(), self.data.len())
    }
}

pub fn validate(bytes: &[u8]) -> Result<Bundle<'_>> {
    ensure!(
        bytes.len() >= HEADER && &bytes[..8] == MAGIC,
        "Invalid file bundle header."
    );
    ensure!(
        u32::from_le_bytes(bytes[8..12].try_into()?) == 1,
        "Unsupported file bundle version."
    );
    let manifest_len = u32::from_le_bytes(bytes[12..16].try_into()?) as usize;
    ensure!(
        manifest_len <= MAX_MANIFEST && manifest_len <= bytes.len() - HEADER,
        "Invalid file manifest length."
    );
    // Bound entry count before deserialization can allocate an unbounded vector.
    struct Entries;
    impl<'de> serde::de::Visitor<'de> for Entries {
        type Value = Vec<Entry>;
        fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
            f.write_str("a bounded entry array")
        }
        fn visit_seq<A: serde::de::SeqAccess<'de>>(
            self,
            mut seq: A,
        ) -> std::result::Result<Self::Value, A::Error> {
            let mut entries = Vec::new();
            while let Some(entry) = seq.next_element()? {
                if entries.len() == MAX_ENTRIES {
                    return Err(serde::de::Error::custom("too many entries"));
                }
                entries.push(entry);
            }
            Ok(entries)
        }
    }
    use serde::de::Deserializer;
    let mut de = serde_json::Deserializer::from_slice(&bytes[HEADER..HEADER + manifest_len]);
    let entries = (&mut de).deserialize_seq(Entries)?;
    de.end()?;
    ensure!(!entries.is_empty(), "A file selection cannot be empty.");
    let mut siblings = HashSet::new();
    let mut depths = Vec::new();
    let mut total = 0usize;
    for (index, entry) in entries.iter().enumerate() {
        ensure!(
            !entry.name.is_empty()
                && entry.name != "."
                && entry.name != ".."
                && !entry.name.contains(['/', '\0'])
                && entry.name.len() <= 4096,
            "Invalid file entry name."
        );
        let depth = if let Some(parent) = entry.parent {
            ensure!(
                parent < index && entries[parent].kind == Kind::Directory,
                "Invalid file entry parent."
            );
            ensure!(
                siblings.insert((parent, entry.name.as_str())),
                "Duplicate sibling name."
            );
            depths[parent] + 1
        } else {
            1
        };
        ensure!(depth <= MAX_DEPTH, "File tree exceeds 64 levels.");
        depths.push(depth);
        ensure!(
            entry.kind == Kind::File || entry.length == 0,
            "Directory has file bytes."
        );
        total = total
            .checked_add(entry.length as usize)
            .ok_or_else(|| eyre::eyre!("File lengths overflow."))?;
    }
    let data = &bytes[HEADER + manifest_len..];
    ensure!(
        total == data.len(),
        "File bundle lengths do not match its bytes."
    );
    Ok(Bundle { entries, data })
}

pub fn encode(entries: &[Entry], data: &[u8], maximum: usize) -> Result<Vec<u8>> {
    ensure!(
        !entries.is_empty() && entries.len() <= MAX_ENTRIES && data.len() <= maximum,
        "File selection exceeds its bounds."
    );
    let manifest = serde_json::to_vec(entries)?;
    ensure!(
        manifest.len() <= MAX_MANIFEST,
        "File manifest exceeds 1 MiB."
    );
    let length = HEADER
        .checked_add(manifest.len())
        .and_then(|n| n.checked_add(data.len()))
        .ok_or_else(|| eyre::eyre!("File selection length overflow."))?;
    ensure!(
        length <= maximum,
        "File selection exceeds the item limit (including manifest)."
    );
    let mut bytes = Vec::new();
    bytes.try_reserve_exact(length)?;
    bytes.extend_from_slice(MAGIC);
    bytes.extend_from_slice(&1u32.to_le_bytes());
    bytes.extend_from_slice(&(manifest.len() as u32).to_le_bytes());
    bytes.extend_from_slice(&manifest);
    bytes.extend_from_slice(data);
    validate(&bytes)?;
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn file(parent: Option<usize>, name: &str, length: u32) -> Entry {
        Entry {
            parent,
            name: name.into(),
            kind: Kind::File,
            length,
        }
    }
    fn unchecked(entries: &[Entry], data: &[u8]) -> Vec<u8> {
        let manifest = serde_json::to_vec(entries).unwrap();
        let mut bytes = MAGIC.to_vec();
        bytes.extend(1u32.to_le_bytes());
        bytes.extend((manifest.len() as u32).to_le_bytes());
        bytes.extend(manifest);
        bytes.extend(data);
        bytes
    }
    #[test]
    fn binary_bytes_duplicate_roots_and_empty_files_round_trip() {
        let entries = [file(None, "same", 3), file(None, "same", 0)];
        let bytes = encode(&entries, &[0, 255, 1], 4096).unwrap();
        assert_eq!(validate(&bytes).unwrap().data, &[0, 255, 1]);
        assert!(validate(&bytes[..bytes.len() - 1]).is_err());
        let mut trailing = bytes;
        trailing.push(0);
        assert!(validate(&trailing).is_err());
    }
    #[test]
    fn malformed_trees_are_refused() {
        for entries in [
            vec![file(None, "..", 0)],
            vec![file(None, "/abs", 0)],
            vec![file(None, "a/b", 0)],
            vec![file(Some(0), "cycle", 0)],
            vec![file(Some(1), "forward", 0), file(None, "parent", 0)],
            vec![file(None, "", 0)],
        ] {
            assert!(validate(&unchecked(&entries, &[])).is_err());
        }
        let dir = Entry {
            parent: None,
            name: "dir".into(),
            kind: Kind::Directory,
            length: 0,
        };
        assert!(
            validate(&unchecked(
                &[dir.clone(), file(Some(0), "a", 0), file(Some(0), "a", 0)],
                &[]
            ))
            .is_err()
        );
        let mut deep = Vec::new();
        for i in 0usize..65 {
            deep.push(Entry {
                parent: i.checked_sub(1),
                ..dir.clone()
            });
        }
        assert!(validate(&unchecked(&deep, &[])).is_err());
        assert!(validate(&unchecked(&vec![file(None, "a", 0); MAX_ENTRIES + 1], &[])).is_err());
        assert!(encode(&[file(None, "a", 1)], &[0], 16).is_err());
        let mut wrong_version = unchecked(&[file(None, "a", 0)], &[]);
        wrong_version[8] = 2;
        assert!(validate(&wrong_version).is_err());
        let mut oversized = unchecked(&[file(None, "a", 0)], &[]);
        oversized[12..16].copy_from_slice(&((MAX_MANIFEST + 1) as u32).to_le_bytes());
        assert!(validate(&oversized).is_err());
    }
}
