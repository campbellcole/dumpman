use clap::Parser;
use log::debug;
use strum::VariantNames;

use crate::mapper::{MapOpType, Mapper};

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

    /// Make the output directory if it does not exist
    #[clap(short, long, action, default_value_t = false)]
    mkdir: bool,
}

fn main() -> Result<(), anyhow::Error> {
    env_logger::builder().format_timestamp(None).init();

    let args = Args::parse();

    let mut mapper = Mapper::try_new(args.root, args.out, args.mkdir)?;
    mapper.load_media()?;

    let (start, end) = mapper.get_range()?;

    debug!("Filename range: {} -> {}", start, end);

    println!(
        "\u{00BB} {} videos ({}..{})\n\u{00BB} Available ops: {}",
        mapper.len(),
        start,
        end,
        MapOpType::VARIANTS.join(", ")
    );

    if args.auto {
        mapper.group_by_day()?;
    } else {
        mapper.prompt_for_ops()?;
    }

    mapper.execute()?;

    Ok(())
}
