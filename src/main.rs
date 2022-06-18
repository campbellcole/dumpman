use clap::Parser;
use lazy_regex::regex;
use thiserror::Error;
use std::{fs, path::{PathBuf, Path}, fmt::Display};
use log::{debug, error, info};

const DEFAULT_ROOT: &str = ".";
const CONTENT_PATH: [&str; 2] = ["DCIM", "100CANON"];

#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
pub struct Args {
    #[clap(short, long, value_parser, default_value = DEFAULT_ROOT)]
    root: String,
}

#[inline]
pub fn join<T>(root: &str, iter: T) -> PathBuf
where
    T: IntoIterator,
    <T as IntoIterator>::Item: AsRef<Path>,
{
    let mut buf = PathBuf::from(root);
    buf.extend(iter);
    return buf;
}

pub fn is_root_valid(root: &str) -> Result<bool, anyhow::Error> {
    if root.is_empty() {
        return Ok(false);
    }

    let mut found_dcim = false;

    for walk in fs::read_dir(&root)? {
        let walk = walk?;
        if walk.file_name().eq_ignore_ascii_case("dcim") {
            found_dcim = true;
        }
    }

    if !found_dcim {
        let full_path = join(root, &CONTENT_PATH);
        if root == DEFAULT_ROOT {
            error!("The current directory is not a valid SD mount point.");
            error!("Change directories or use the `-r` option to set the mount point root.")
        }
        debug!("Unable to locate {:#?}", full_path);
        return Ok(false);
    }

    Ok(true)
}

pub fn get_media_ids(root: &str) -> Vec<u32> {
    let re = regex!(r#"MVI_(\d{4})\.MOV"#);
    let path = join(root, &CONTENT_PATH);
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

#[derive(Error, Debug)]
pub enum Errors {
    InvalidRoot,
    NoVideos,
}

impl Display for Errors {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}

fn main() -> Result<(), anyhow::Error> {
    env_logger::builder().format_timestamp(None).init();

    let args = Args::parse();

    info!("Checking validity of '{}'", args.root);
    if !is_root_valid(&args.root)? {
        return Err(Errors::InvalidRoot.into());
    }
    info!("Found DCIM!");

    info!("Parsing filenames");
    let ids = get_media_ids(&args.root);
    info!("Parsed {} video files!", ids.len());
    
    if ids.len() == 0 {
        error!("Parsing completed without error, but there are no videos found.");
        error!("Cannot continue with no videos.");
        return Err(Errors::NoVideos.into());
    }

    let start = ids.get(0).unwrap();
    let end = ids.get(ids.len() - 1).unwrap();

    info!("Filename range: {} -> {}", start, end);

    Ok(())
}