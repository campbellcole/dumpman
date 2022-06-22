use chrono::{Date, TimeZone, Utc};
use lazy_regex::regex;
use std::{
    collections::HashMap,
    ffi::OsString,
    fs,
    path::PathBuf,
    str::FromStr,
    time::{SystemTime, UNIX_EPOCH},
};
use strum::{EnumString, EnumVariantNames, VariantNames};

use crate::{
    error::{Errors, OpValidationResult, Result},
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
    Copy,
}

impl Default for MapOpType {
    fn default() -> Self {
        Self::Copy
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
    pub fn try_new(root: String, out: String, mkdir: bool) -> Result<Self> {
        let root_path = util::join(&root, CONTENT_PATH);
        if !root_path.exists() {
            return Err(Errors::InvalidRoot(root, root_path));
        }

        let out_path = PathBuf::from(&out);
        if !out_path.exists() {
            if mkdir {
                fs::create_dir_all(&out_path).map_err(|e| Errors::IOError(e.kind()))?;
            } else {
                return Err(Errors::OutputDirectoryNotFound);
            }
        } else {
            match fs::read_dir(&out_path) {
                Ok(it) => {
                    for f in it {
                        if f.unwrap().file_name().eq_ignore_ascii_case(".ds_store") {
                            continue;
                        }

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

    pub fn get_range(&self) -> Result<(u32, u32)> {
        if self.media.len() == 0 {
            return Ok((0, 0));
        }

        let start = self.media.get(0).unwrap().id;
        let end = self.media.get(self.media.len() - 1).unwrap().id;

        Ok((start, end))
    }

    pub fn len(&self) -> usize {
        self.media.len()
    }

    fn validate_ops(&self) -> Result<OpValidationResult> {
        use OpValidationResult::*;

        if self.ops.len() == 0 {
            return Ok(Empty);
        }

        for op1 in self.ops.clone() {
            for op2 in self.ops.clone() {
                if op1 == op2 {
                    continue;
                }

                if (op1.start > op2.start && op2.end > op1.start)
                    || (op1.start < op2.start && op1.end > op2.start)
                {
                    return Ok(OverlappingRange(op1, op2));
                }
            }
        }

        Ok(Valid)
    }

    pub fn group_by_day(&mut self) -> Result {
        let mut v = Vec::<MapOp>::new();
        let mut map = HashMap::<Date<Utc>, (u32, u32)>::new();

        for m in &self.media {
            let epoch = m.created_at.duration_since(UNIX_EPOCH).unwrap().as_secs();
            let date = Utc.timestamp(epoch.try_into().unwrap(), 0).date();
            match map.get_mut(&date) {
                Some(bounds) => {
                    if m.id < bounds.0 {
                        bounds.0 = m.id;
                    }
                    if m.id >= bounds.1 {
                        bounds.1 = m.id + 1;
                    }
                }
                None => {
                    map.insert(date, (m.id, m.id + 1));
                }
            }
        }

        println!("Enter a name for the following days.");
        for (date, bounds) in map {
            let prompt = format!("{date}: ");
            let name = rprompt::prompt_reply_stdout(&prompt).unwrap();
            let name = format!("{name}_{date}");
            let op_type = {
                if MapOpType::VARIANTS.len() > 1 {
                    let type_name = rprompt::prompt_reply_stdout("Enter map operation: ").unwrap();
                    MapOpType::from_str(&type_name).unwrap()
                } else {
                    Default::default()
                }
            };
            v.push(MapOp {
                end: bounds.1,
                name,
                op_type,
                start: bounds.0,
            });
        }

        self.ops.extend(v);

        let valid = self.validate_ops()?;

        match &valid {
            OpValidationResult::Valid => {}
            _ => return Err(Errors::ValidationError(valid)),
        }

        Ok(())
    }

    pub fn prompt_for_ops(&mut self) -> Result {
        let mut v = Vec::<MapOp>::new();
        loop {
            let name = rprompt::prompt_reply_stdout("Enter group name (empty = done): ").unwrap();
            if name.is_empty() {
                break;
            }

            let op_type = {
                if MapOpType::VARIANTS.len() > 1 {
                    let type_name = rprompt::prompt_reply_stdout("Enter map operation: ").unwrap();
                    MapOpType::from_str(&type_name).unwrap()
                } else {
                    Default::default()
                }
            };

            let start = rprompt::prompt_reply_stdout("Enter start range (incl.): ")
                .unwrap()
                .parse::<u32>()
                .unwrap();
            let end = rprompt::prompt_reply_stdout("Enter end range (excl.): ")
                .unwrap()
                .parse::<u32>()
                .unwrap();

            v.push(MapOp {
                end,
                name,
                op_type,
                start,
            });
        }

        self.ops.extend(v);

        let valid = self.validate_ops()?;

        match &valid {
            OpValidationResult::Valid => {}
            _ => return Err(Errors::ValidationError(valid)),
        }

        Ok(())
    }

    pub fn execute(&mut self) -> Result {
        for op in &self.ops {
            match op.op_type {
                MapOpType::Copy => {
                    let group_out = util::join(&self.out_path, [&op.name]);
                    fs::create_dir(&group_out).unwrap();
                    for m in self.media.clone() {
                        if m.id >= op.start && m.id < op.end {
                            let from = util::join(&self.root_path, [&m.filename]);
                            let to = util::join(&group_out, [&m.filename]);
                            fs::copy(from, to).unwrap();
                        }
                    }
                }
            }
        }
        Ok(())
    }
}
