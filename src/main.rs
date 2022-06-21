use chrono::prelude::*;
use clap::Parser;
use lazy_regex::regex;
use log::{debug, error};
use std::{
    collections::HashMap,
    ffi::OsString,
    fmt::Display,
    fs,
    path::{Path, PathBuf},
    str::FromStr,
    time::{SystemTime, UNIX_EPOCH},
};
use strum::{EnumString, EnumVariantNames, VariantNames};
use thiserror::Error;

use crate::mapper::Mapper;

const DEFAULT_ROOT: &str = ".";

pub mod error;
pub mod mapper;
pub mod util;

#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
pub struct Args {
    /// The root directory of the SD card
    #[clap(short, long, value_parser, default_value = DEFAULT_ROOT)]
    root: String,

    /// The output directory to store the generated groups
    #[clap(short, long, value_parser)]
    out: String,

    /// Enable autogrouping
    #[clap(short, long, action, default_value_t = false)]
    auto: bool,
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
                || (op1.start < op2.start && op1.end > op2.start)
            {
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

    v
}

pub fn group_by_day(media: &Vec<Media>) -> MapOps {
    let v = Vec::new();

    let mut map = HashMap::<Date<Utc>, (u32, u32)>::new();

    for m in media {
        let epoch = m.created_at.duration_since(UNIX_EPOCH).unwrap().as_secs();
        let date: Date<Utc> = Utc.timestamp(epoch.try_into().unwrap(), 0).date();
        match map.get_mut(&date) {
            Some(mm) => {
                if m.id < mm.0 {
                    mm.0 = m.id;
                }
                if m.id >= mm.1 {
                    mm.1 = m.id + 1;
                }
            }
            None => {
                map.insert(date, (m.id, m.id + 1));
            }
        }
    }

    println!("{map:#?}");

    v
}

fn main() -> Result<(), anyhow::Error> {
    env_logger::builder().format_timestamp(None).init();

    let args = Args::parse();

    let mapper = Mapper::try_new(args.root, args.out)?;
    mapper.load_media()?;

    let root_path = util::join(&args.root, CONTENT_PATH);

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
    let media = get_media_ids(&args.root);
    debug!("Parsed {} video files!", media.len());

    if media.len() == 0 {
        return Err(Errors::NoVideos.into());
    }

    let start = media.get(0).unwrap().id;
    let end = media.get(media.len() - 1).unwrap().id;

    debug!("Filename range: {} -> {}", start, end);

    println!(
        "\u{00BB} {} videos ({}..{})\n\u{00BB} Available ops: {}",
        media.len(),
        start,
        end,
        MapOpType::VARIANTS.join(", ")
    );

    let ops = {
        if args.auto {
            group_by_day(&media)
        } else {
            input_loop()
        }
    };

    debug!("Validating map ops");
    let valid = validate_ops(&ops);
    debug!("Result: {:?}", valid);

    match &valid {
        OpValidationResult::Valid => {}
        _ => return Err(Errors::ValidationError(valid).into()),
    }

    println!("Processing all operations... (this will take a while)");

    for op in ops {
        debug!("Processing ops: {}", op.name);
        match op.op_type {
            MapOpType::Group => {
                let group_out = util::join(&out_path, [op.name]);
                fs::create_dir(&group_out).unwrap();
                for m in media.clone() {
                    if m.id >= op.start && m.id < op.end {
                        let from = util::join(&root_path, [&m.filename]);
                        let to = util::join(&group_out, [&m.filename]);
                        fs::copy(from, to).unwrap();
                    }
                }
            }
        }
    }

    println!("Done! {}", out_path.to_string_lossy());
    Ok(())
}
