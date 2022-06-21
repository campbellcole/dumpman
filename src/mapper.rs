use lazy_regex::regex;
use std::{ffi::OsString, fs, path::PathBuf, time::SystemTime};
use strum::{EnumString, EnumVariantNames};

use crate::{
    error::{Errors, Result},
    util,
};

const CONTENT_PATH: [&str; 2] = ["DCIM", "100CANON"];

#[derive(Debug, Clone)]
pub struct Mapper {
    root_path: PathBuf,
    out_path: PathBuf,
    media: Vec<Media>,
    ops: Vec<MapOp>,
}

#[derive(Debug, Clone, PartialEq, Eq, EnumString, EnumVariantNames)]
#[strum(serialize_all = "kebab_case")]
pub enum MapOpType {
    Group,
}

impl Default for MapOpType {
    fn default() -> Self {
        Self::Group
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MapOp {
    pub(crate) op_type: MapOpType,
    pub(crate) name: String,
    pub(crate) start: u32,
    pub(crate) end: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Media {
    id: u32,
    filename: OsString,
    created_at: SystemTime,
}

impl PartialOrd for Media {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        self.id.partial_cmp(&other.id)
    }
}

impl Ord for Media {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.id.cmp(&other.id)
    }
}

impl Mapper {
    pub fn try_new(root: String, out: String) -> Result<Self> {
        let root_path = util::join(&root, &["DCIM", "100CANON"]);
        if !root_path.exists() {
            return Err(Errors::InvalidRoot(root, root_path));
        }

        let out_path = PathBuf::from(&out);
        if !out_path.exists() {
            return Err(Errors::OutputDirectoryNotFound);
        } else {
            match fs::read_dir(&out_path) {
                Ok(it) => {
                    if it.count() != 0 {
                        return Err(Errors::OutputDirectoryNotEmpty);
                    }
                }
                Err(e) => {
                    return Err(Errors::IOError(e.kind()));
                }
            }
        }

        Ok(Self {
            root_path,
            out_path,
            media: Vec::new(),
            ops: Vec::new(),
        })
    }

    pub fn load_media(&mut self) -> Result {
        let re = regex!(r#"MVI_(\d{4})\.MOV"#);
        let dir = match fs::read_dir(&self.root_path) {
            Ok(dir) => dir,
            Err(e) => return Err(Errors::IOError(e.kind())),
        };
        let mut media = dir
            .filter_map(|entry| {
                let entry = entry.unwrap();
                if entry.file_type().unwrap().is_file() {
                    let filename = entry.file_name().clone();
                    let caps = re.captures(filename.to_str().unwrap());
                    caps.map(|cap| Media {
                        id: cap.get(1).unwrap().as_str().parse().unwrap(),
                        filename: filename.clone(),
                        created_at: entry.metadata().unwrap().created().unwrap(),
                    })
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();

        if media.len() == 0 {
            Err(Errors::NoVideos)
        } else {
            media.sort_unstable();
            self.media.extend(media);
            Ok(())
        }
    }

    pub fn prompt_for_ops() -> Result {
        
    }
}
