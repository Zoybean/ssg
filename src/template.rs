use parser::Ident;
use std::fmt::Write as _;
use std::fs::File;
use std::io::{Read as _, Write as _};
use std::path::Path;
use std::path::PathBuf;
use winnow::Parser as _;

use super::parser;
use super::Var;

pub(crate) fn load_template<'a>(
    buffer: &'a mut String,
    template_path: &PathBuf,
) -> Vec<parser::Line<'a>> {
    let mut template = {
        let mut template = std::fs::OpenOptions::new()
            .read(true)
            .open(template_path)
            .expect("Could not open template file");
        template
            .read_to_string(buffer)
            .expect("could not read template file");
        buffer.as_str()
    };
    let template_parsed = parser::lines
        .parse_next(&mut template)
        .expect("parsing error");
    template_parsed
}

pub(crate) fn convert_template_file(
    source_path: &Path,
    template_path: &Path,
    template_parsed: &[parser::Line],
    from_dir: &Path,
    to_dir: PathBuf,
) {
    let source_content = read_string(&source_path).expect("reading source");
    let context = Context {
        template_path,
        template_parsed,
        source_file_path: &source_path,
        source_file: &source_content,
        source_title: "Candy Corvid",
    };
    let out_path = path_for(from_dir, to_dir, &context.source_file_path, None);
    let mut out_file = File::create(&out_path).expect("open output file");

    log::info!(
        "writing file '{}' from template '{}' and content '{}'",
        out_path.display(),
        template_path.display(),
        context.source_file_path.display()
    );
    let out = apply_template(&context);
    write!(&mut out_file, "{}", out).expect("write output file");
}

/// get the output path corresponding to the given input path
pub(crate) fn path_for(
    in_root: &Path,
    mut out_root: PathBuf,
    path: &Path,
    strip_extension: Option<&str>,
) -> PathBuf {
    out_root.push(
        path.strip_prefix(in_root)
            .expect("input path should be in input dir"),
    );
    if let Some(suf) = strip_extension {
        if out_root.extension() == Some(suf.as_ref()) {
            out_root.set_extension("");
        }
    }
    out_root
}

pub(crate) fn read_string(path: &Path) -> Result<String, std::io::Error> {
    let mut s = String::new();
    let file = &mut File::open(path)?;
    file.read_to_string(&mut s)?;
    Ok(s)
}

pub(crate) struct Context<'a> {
    pub(crate) template_path: &'a Path,
    pub(crate) template_parsed: &'a [parser::Line<'a>],
    pub(crate) source_file_path: &'a Path,
    pub(crate) source_file: &'a str,
    pub(crate) source_title: &'a str,
}

pub(crate) fn apply_template(context: &Context) -> String {
    let mut agg = String::new();
    for item in context.template_parsed {
        let store;
        let val = match item {
            parser::Line::Raw(r) => r,
            parser::Line::Command(parser::Command::Insert(i)) => match i {
                parser::Insert::Path(p) => {
                    store = load(context.template_path, p).expect(&format!(
                        "failed to resolve path template: {}",
                        p.as_ref().display()
                    ));
                    store.as_str()
                }
                parser::Insert::Var(v) => evaluate(v, context),
            },
        };
        writeln!(&mut agg, "{}", val).unwrap();
    }
    agg
}

pub(crate) fn evaluate<'a>(v: &Var, context: &'a Context) -> &'a str {
    let value = match &*v.0 {
        [Ident("self"), rest @ ..] => match rest {
            [Ident("content")] => context.source_file,
            [Ident("title")] => context.source_title,
            _ => todo!("unknown variable"),
        },
        _ => todo!("unknown variable"),
    };
    value
}

pub(crate) fn load(from: &Path, p: &parser::Path<'_>) -> Result<String, std::io::Error> {
    let mut path = from
        .parent()
        .expect("path is to a file, so it must have a parent")
        .to_path_buf();
    path.push(p);
    Ok(read_string(&path)?)
}
