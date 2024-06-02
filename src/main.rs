use std::{
    collections::{HashMap, HashSet},
    fs::{self, read_dir},
    io::{self, Read as _},
    ops::Neg as _,
    path::{self, PathBuf},
};

use clap::Parser as _;

use parser::Var;
use walkdir::{DirEntry, WalkDir};

mod parser;
mod template;

#[derive(clap::Parser)]
/// Static site generator with a fairly simple syntax
pub struct App {
    /// Path to directory containing page content files. The template will be applied to each file in this directory
    #[arg(short, long, alias = "source")]
    source_dir: PathBuf,
    /// Path to the primary template file
    #[arg(short, long)]
    template: PathBuf,
    /// Path to the directory that the output will be written to
    #[arg(short = 'o', long, alias = "target")]
    output: PathBuf,
    /// Path to the directory containing files that will be pasted to the output dir, unmodified.
    #[arg(short, long)]
    assets: Option<Vec<PathBuf>>,
    /// Folder containing RSS feed items
    #[arg(short, long)]
    rss: Option<PathBuf>,
    /// Increase log level by 1
    #[arg(short, long, action = clap::ArgAction::Count)]
    verbose: u8,
    /// Reduce log level by 1
    #[arg(short, long, action = clap::ArgAction::Count)]
    quiet: u8,
    /// Disable logger entirely
    #[arg(long)]
    no_log: bool,
}

fn main() {
    let App {
        template: template_path,
        source_dir,
        output,
        assets,
        rss,
        verbose,
        quiet,
        no_log,
    } = App::parse();
    if !no_log {
        simplelog::SimpleLogger::init(
            resolve_log_level(quiet, verbose),
            simplelog::ConfigBuilder::new()
                .set_level_color(log::Level::Error, Some(simplelog::Color::Red))
                .set_location_level(log::LevelFilter::Debug)
                .build(),
        )
        .expect("registering logger");
    }

    let mut buf = String::new();
    let template_parsed = template::load_template(&mut buf, &template_path);
    fs::create_dir_all(&output).expect("creating output dir");
    let (root, walk_dir) = walk_visible_entries(&source_dir);
    for entry in walk_dir {
        // TODO allow templates within subdirs
        let entry = entry.expect("reading dir entry");
        let source_path = entry.path();
        let dest_path = graft_path(root.path(), output.clone(), source_path);
        if entry.file_type().is_dir() {
            log::debug!(
                "creating subdirectory '{}' mirroring source '{}'",
                dest_path.display(),
                source_path.display()
            );
            fs::create_dir_all(&dest_path).expect("create subdir");
        }
        if entry.file_type().is_file() {
            log::debug!(
                "creating templated file '{}' mirroring source '{}'",
                dest_path.display(),
                source_path.display()
            );
            let to_dir = dest_path
                .parent()
                .expect("every path we care about has a parent")
                .to_owned();
            template::convert_template_file(
                &source_path,
                &template_path,
                &template_parsed,
                &source_dir,
                to_dir,
            );
        }
    }
    if let Some(assets) = assets {
        log::info!("copying assets");
        for dir in assets {
            log::info!(
                "copying files from '{}' to '{}'",
                dir.display(),
                output.display()
            );
            let (root, walk_dir) = walk_visible_entries(dir);
            for entry in walk_dir {
                let entry = entry.expect("read asset entry");
                let src = entry.path();
                let dest = &graft_path(root.path(), output.clone(), src);
                log::trace!("walking {}", src.display());
                if entry.file_type().is_dir() {
                    log::trace!("creating dir {}", dest.display());
                    fs::create_dir_all(dest).expect("create asset subdir");
                }
                if entry.file_type().is_file() {
                    log::debug!("copying '{}' to '{}'", src.display(), dest.display());
                    fs::copy(src, dest).expect("copy file");
                }
            }
        }
    }
    if let Some(rss_src) = rss {
        let items = read_rss(rss_src).expect("rss source should be well formed");
        let mut rss_path = output;
        rss_path.push("feed.xml");
        write_rss(rss_path, items).expect("writing rss output file");
    }
}

/// get the relative path from `source_root` to `path`, then add that relative path to `dest_root`
fn graft_path(source_root: &path::Path, mut dest_root: PathBuf, path: &path::Path) -> PathBuf {
    let orig_dest = dest_root.clone();
    let rel = path
        .strip_prefix(source_root)
        .expect("path should be prefixed with root path. no other symlinks are followed");
    dest_root.push(rel);
    assert_ne!(orig_dest, dest_root);
    dest_root
}

fn resolve_log_level(quiet: u8, verbose: u8) -> log::LevelFilter {
    let level_shift = (quiet as i8).neg().saturating_add_unsigned(verbose);
    // cannot create levels from integers, so create a map to do it for us
    let levels: HashMap<_, _> = log::LevelFilter::iter().map(|l| (l as i8, l)).collect();
    // initial log level, that is then shifted
    let mut level = log::LevelFilter::Info as i8;
    // shift the default log level
    level += level_shift;
    // ensure the shifted value is still within the range of valid log levels
    // WARN: this may fail if there are ever any gaps in the sequence of log::LevelFilter enum values
    level = level.clamp(log::LevelFilter::Off as i8, log::LevelFilter::max() as i8);
    levels[&level]
}

/// walk entries of the directory, skipping items that start with '.'
fn walk_visible_entries(
    dir: impl AsRef<path::Path>,
) -> (DirEntry, impl Iterator<Item = walkdir::Result<DirEntry>>) {
    fn entry_hidden(ent: &DirEntry) -> bool {
        ent.file_name().as_encoded_bytes().starts_with(b".")
    }

    let mut walk_dir = WalkDir::new(dir).into_iter();
    let root = walk_dir
        .next()
        .expect("directory should at least have a root element")
        .expect("traverse root dir of asset");
    let walk_dir = walk_dir.filter_entry(|ent| !entry_hidden(ent));
    (root, walk_dir)
}

#[derive(thiserror::Error, Debug)]
enum RssSourceError {
    #[error(transparent)]
    De(#[from] toml::de::Error),
    #[error(transparent)]
    Io(#[from] io::Error),
    #[error("non-unique GUID used for RSS feed")]
    DuplicateGuid,
}
#[derive(thiserror::Error, Debug)]
enum RssWriteError {
    #[error(transparent)]
    Io(#[from] io::Error),
    #[error(transparent)]
    Write(#[from] rss::Error),
}

fn write_rss(rss_path: PathBuf, items: Vec<rss::Item>) -> Result<(), RssWriteError> {
    // TODO atom self element
    // TODO properly list updates
    log::info!("writing rss feed to '{}'", rss_path.display());
    let rss_file = fs::File::create(rss_path)?;
    let mut c = rss::Channel::default();
    c.set_title(String::from("Candy Corvid"));
    c.set_link(String::from("https://candy-corvid.neocities.org/"));
    c.set_description(String::from("CandyCorvid's RSS feed"));
    c.set_language(String::from("en-AU"));
    c.set_items(items);
    c.write_to(rss_file)?;
    Ok(())
}

fn read_rss(rss_src: PathBuf) -> Result<Vec<rss::Item>, RssSourceError> {
    let mut items = Vec::new();
    let mut guids = HashSet::new();
    for file in fs::read_dir(rss_src)? {
        #[derive(serde::Deserialize, Debug)]
        struct RssSourceItem {
            title: String,
            desc: Option<String>,
            url: String,
        }
        let content = {
            let file = file?;
            let mut file = fs::File::open(file.path())?;
            let mut buf = String::new();
            file.read_to_string(&mut buf)?;
            buf
        };
        let item: RssSourceItem = toml::de::from_str(&content)?;
        log::info!("feed item: {:?}", item);

        let RssSourceItem { title, desc, url } = item;
        items.push(
            rss::ItemBuilder::default()
                .title(Some(title))
                .link(Some(url.clone()))
                .description(desc.map(|desc| String::from(desc)))
                .guid(Some(
                    rss::GuidBuilder::default()
                        .value(url.clone())
                        .permalink(true)
                        .build(),
                ))
                .build(),
        );
        if !guids.insert(url) {
            Err(RssSourceError::DuplicateGuid)?;
        }
    }
    Ok(items)
}
