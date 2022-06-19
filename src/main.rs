use clap::Parser;
use lazy_regex::regex;
use strum::{EnumString, EnumVariantNames, VariantNames};
use thiserror::Error;
use std::{fs, path::{PathBuf, Path}, fmt::Display, str::FromStr};
use log::{debug, error};

const DEFAULT_ROOT: &str = ".";
const CONTENT_PATH: [&str; 2] = ["DCIM", "100CANON"];

#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
pub struct Args {
    #[clap(short, long, value_parser, default_value = DEFAULT_ROOT)]
    /// The root directory of the SD card
    root: String,
    /// The output directory to store the generated groups
    #[clap(short, long, value_parser)]
    out: String,
}

#[inline]
pub fn join<'a, S, T>(root: &'a S, iter: T) -> PathBuf
where
    S: 'a,
    PathBuf: From<&'a S>,
    T: IntoIterator,
    <T as IntoIterator>::Item: AsRef<Path>,
{
    let mut buf = PathBuf::from(root);
    buf.extend(iter);
    return buf;
}

pub fn get_media_ids(root: &str) -> Vec<u32> {
    let re = regex!(r#"MVI_(\d{4})\.MOV"#);
    let path = join(&root.to_string(), &CONTENT_PATH);
    let mut file_list = fs::read_dir(path).unwrap().filter_map(|r| {
        let r = r.unwrap();
        if r.file_type().unwrap().is_file() {
            Some(r.file_name())
        } else {
            None
        }
    }).filter_map(|os| {
        let caps = re.captures(os.to_str().unwrap());
        caps.map(|cap| {
            cap.get(1).expect("filename does not follow hardcoded pattern. tough luck.").as_str().parse::<u32>().unwrap()
        })
    }).collect::<Vec<_>>();

    file_list.sort_unstable();
    file_list
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
    op_type: MapOpType,
    name: String,
    start: u32,
    end: u32,
}

pub type MapOps = Vec<MapOp>;

#[derive(Debug, Clone)]
pub enum OpValidationResult {
    Valid,
    OverlappingRange(MapOp, MapOp),
    Empty,
}

pub fn validate_ops(ops: &MapOps) -> OpValidationResult {
    use OpValidationResult::*;
    
    if ops.len() == 0 {
        return Empty;
    }

    for op1 in ops.clone() {
        for op2 in ops.clone() {
            if op1 == op2 {
                continue;
            }

            if (op1.start > op2.start && op2.end > op1.start)
            || (op1.start < op2.start && op1.end > op2.start) {
                return OverlappingRange(op1, op2);
            }
        }
    }

    Valid
}

pub fn input_loop() -> MapOps {
    let mut v = Vec::new();

    loop {
        // i don't care enough to handle these errors, just don't enter bad input
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

        let start = rprompt::prompt_reply_stdout("Enter start range (incl.): ").unwrap().parse::<u32>().unwrap();
        let end = rprompt::prompt_reply_stdout("Enter end range (excl.): ").unwrap().parse::<u32>().unwrap();

        v.push(MapOp {
            end,
            name,
            op_type,
            start
        });
    }

    v
}

#[derive(Error, Debug)]
pub enum Errors {
    InvalidRoot(String, PathBuf),
    OutputDirectoryNotFound,
    OutputDirectoryNotEmpty,
    NoVideos,
    ValidationError(OpValidationResult),
}

impl Display for Errors {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        use Errors::*;
        match self {
            InvalidRoot(root, path) => write!(f, "{:#?} is not a valid root ({:#?} does not exist).", root, path),
            OutputDirectoryNotFound => write!(f, "The output directory does not exist!"),
            OutputDirectoryNotEmpty => write!(f, "The output directory is not empty!"),
            NoVideos => write!(f, "There are no compatible video files in the root folder!"),
            ValidationError(res) => {
                use OpValidationResult::*;
                match res {
                    OverlappingRange(op1, op2) => write!(f, "{} ({}..{}) overlaps {} ({}..{})", op1.name, op1.start, op1.end, op2.name, op2.start, op2.end),
                    Empty => write!(f, "No operations defined! Exiting."),
                    Valid => panic!()
                }
            }
        }
    }
}

fn main() -> Result<(), anyhow::Error> {
    env_logger::builder().format_timestamp(None).init();

    let args = Args::parse();
    let root_path = join(&args.root, &CONTENT_PATH);

    debug!("Checking root directory");
    if root_path.to_string_lossy().is_empty() || !root_path.exists() {
        if args.root == DEFAULT_ROOT {
            error!("The current directory is not a valid SD mount point.");
            error!("Change directories or use the `-r` option to set the mount point root.")
        }
        return Err(Errors::InvalidRoot(args.root, root_path).into());
    }
    debug!("Root is valid!");

    debug!("Checking output directory");
    let out_path = PathBuf::from(&args.out);
    if !out_path.exists() {
        return Err(Errors::OutputDirectoryNotFound.into());
    } else if fs::read_dir(&out_path)?.count() != 0 {
        return Err(Errors::OutputDirectoryNotEmpty.into());
    }
    debug!("Output is valid!");

    debug!("Parsing filenames");
    let ids = get_media_ids(&args.root);
    debug!("Parsed {} video files!", ids.len());
    
    if ids.len() == 0 {
        return Err(Errors::NoVideos.into());
    }

    let start = ids.get(0).unwrap();
    let end = ids.get(ids.len() - 1).unwrap();

    debug!("Filename range: {} -> {}", start, end);

    println!("\u{00BB} {} videos ({}..{})\n\u{00BB} Available ops: {}", ids.len(), start, end, MapOpType::VARIANTS.join(", "));

    let ops = input_loop();

    debug!("Validating map ops");
    let valid = validate_ops(&ops);
    debug!("Result: {:?}", valid);

    match &valid {
        OpValidationResult::Valid => {},
        _ => return Err(Errors::ValidationError(valid).into())
    }

    println!("Processing all operations... (this will take a while)");

    for op in ops {
        debug!("Processing ops: {}", op.name);
        match op.op_type {
            MapOpType::Group => {
                let group_out = join(&out_path, &[op.name]);
                fs::create_dir(&group_out).unwrap();
                for id in ids.clone() {
                    if id >= op.start && id < op.end {
                        let filename = format!("MVI_{}.MOV", id);
                        let from = join(&root_path, &[&filename]);
                        let to = join(&group_out, &[&filename]);
                        fs::copy(from, to).unwrap();
                    }
                }
            }
        }
    }

    println!("Done! {}", out_path.to_string_lossy());
    Ok(())
}