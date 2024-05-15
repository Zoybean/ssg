use std::{fs::read_dir, path::PathBuf};

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
}

fn main() {
    let App {
        template: template_path,
        source_dir,
        output,
        assets,
    } = App::parse();
    let mut buf = String::new();
    let template_parsed = template::load_template(&mut buf, &template_path);
    std::fs::create_dir_all(&output).expect("creating output dir");
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
}
