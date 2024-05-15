use std::{
    collections::HashSet,
    fs::{self, read_dir},
    io::{self, Read as _},
    path::PathBuf,
};

use clap::Parser as _;
use fs_extra::dir::CopyOptions;

use parser::Var;

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
}

fn main() {
    let App {
        template: template_path,
        source_dir,
        output,
        assets,
        rss,
    } = App::parse();
    let mut buf = String::new();
    let template_parsed = template::load_template(&mut buf, &template_path);
    fs::create_dir_all(&output).expect("creating output dir");
    for entry in read_dir(&source_dir).expect("read dir") {
        let source_path = entry.expect("reading dir entry").path();
        template::convert_template_file(
            &source_path,
            &template_path,
            &template_parsed,
            &source_dir,
            output.clone(),
        );
    }
    if let Some(assets) = assets {
        for dir in assets {
            println!(
                "copying files from '{}' to '{}'",
                dir.display(),
                output.display()
            );
            fs_extra::dir::copy(
                &dir,
                &output,
                &CopyOptions::new().overwrite(true).content_only(true),
            )
            .expect("copy assets");
        }
    }
    if let Some(rss_src) = rss {
        let items = read_rss(rss_src).expect("rss source should be well formed");
        let mut rss_path = output;
        rss_path.push("feed.xml");
        write_rss(rss_path, items).expect("writing rss output file");
    }
}

#[derive(thiserror::Error, Debug)]
enum RssSourceError {
    #[error(transparent)]
    De(#[from] toml::de::Error),
    #[error(transparent)]
    Io(#[from] io::Error),
    #[error("non-unique GUID used for RSS feed")]
    Guid,
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
    println!("writing rss feed to '{}'", rss_path.display());
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
        #[derive(serde::Deserialize)]
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
            Err(RssSourceError::Guid)?;
        }
    }
    Ok(items)
}
