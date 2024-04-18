use std::{
    fmt::Write as _,
    fs::{read_dir, File},
    io::Read,
    io::Write as _,
    path::{Path, PathBuf},
};

use clap::Parser as _;
use fs_extra::dir::CopyOptions;
use winnow::Parser as _;

use parser::{Ident, Var};

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
    let template_parsed = load_template(&mut buf, &template_path);
    std::fs::create_dir_all(&output).expect("creating output dir");
    for entry in read_dir(&source_dir).expect("read dir") {
        let source_path = entry.expect("reading dir entry").path();
        convert_template_file(
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

fn load_template<'a>(buffer: &'a mut String, template_path: &PathBuf) -> Vec<parser::Line<'a>> {
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

fn convert_template_file(
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

    println!(
        "writing file '{}' from template '{}' and content '{}'",
        out_path.display(),
        template_path.display(),
        context.source_file_path.display()
    );
    let out = apply_template(&context);
    write!(&mut out_file, "{}", out).expect("write output file");
}

/// get the output path corresponding to the given input path
fn path_for(
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

fn read_string(path: &Path) -> Result<String, std::io::Error> {
    let mut s = String::new();
    File::open(path)?.read_to_string(&mut s)?;
    Ok(s)
}

struct Context<'a> {
    template_path: &'a Path,
    template_parsed: &'a [parser::Line<'a>],
    source_file_path: &'a Path,
    source_file: &'a str,
    source_title: &'a str,
}

fn apply_template(context: &Context) -> String {
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

fn evaluate<'a>(v: &Var, context: &'a Context) -> &'a str {
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

fn load(from: &Path, p: &parser::Path<'_>) -> Result<String, std::io::Error> {
    let mut path = from
        .parent()
        .expect("path is to a file, so it must have a parent")
        .to_path_buf();
    path.push(p);
    Ok(read_string(&path)?)
}

mod parser {
    use std::path::{Path as FilePath, PathBuf as FilePathBuf};

    use winnow::{
        ascii::escaped_transform,
        combinator::{alt, cut_err, delimited, preceded, separated},
        stream::AsChar,
        token::take_till,
        PResult, Parser,
    };
    #[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
    pub enum Line<'a> {
        Raw(&'a str),
        Command(Command<'a>),
    }
    #[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
    pub enum Command<'a> {
        Insert(Insert<'a>),
    }
    #[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
    pub enum Insert<'a> {
        Var(Var<'a>),
        Path(Path<'a>),
    }
    #[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
    pub enum Path<'a> {
        Path(&'a FilePath),
        PathBuf(FilePathBuf),
    }
    impl AsRef<FilePath> for Path<'_> {
        fn as_ref(&self) -> &FilePath {
            match self {
                Path::Path(p) => p,
                Path::PathBuf(p) => &p,
            }
        }
    }
    #[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
    pub struct Ident<'a>(pub &'a str);
    #[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
    pub struct Var<'a>(pub Vec<Ident<'a>>);

    pub fn lines<'a>(s: &mut &'a str) -> PResult<Vec<Line<'a>>> {
        separated(.., line, line_end).parse_next(s)
    }
    pub fn line_end<'a>(s: &mut &'a str) -> PResult<&'a str> {
        alt((winnow::ascii::line_ending, "\r\n"))
            .recognize()
            .parse_next(s)
    }
    pub fn line<'a>(s: &mut &'a str) -> PResult<Line<'a>> {
        alt((
            preceded(':', cut_err(command)).map(Line::Command),
            preceded('+', cut_err(raw)).map(Line::Raw),
        ))
        .parse_next(s)
    }
    fn command<'a>(s: &mut &'a str) -> PResult<Command<'a>> {
        preceded(("insert", winnow::ascii::space1), insert)
            .map(Command::Insert)
            .parse_next(s)
    }
    fn raw<'a>(s: &mut &'a str) -> PResult<&'a str> {
        take_till(.., char::is_newline).parse_next(s)
    }
    fn insert<'a>(s: &mut &'a str) -> PResult<Insert<'a>> {
        alt((preceded('$', var).map(Insert::Var), path.map(Insert::Path))).parse_next(s)
    }

    fn var<'a>(s: &mut &'a str) -> PResult<Var<'a>> {
        // let mut line = take_till(.., char::is_newline).recognize().parse_next(s)?;
        separated(.., ident, '.').map(Var).parse_next(s)
    }
    fn ident<'a>(s: &mut &'a str) -> PResult<Ident<'a>> {
        take_till(.., ('.', '\n', '\r', ' '))
            .map(Ident)
            .parse_next(s)
    }
    fn path<'a>(s: &mut &'a str) -> PResult<Path<'a>> {
        alt((
            str_single.map(AsRef::as_ref).map(Path::Path),
            str_double_unescaped.map(AsRef::as_ref).map(Path::Path),
            str_double_escaped.map(Into::into).map(Path::PathBuf),
        ))
        .parse_next(s)
    }
    fn str_single<'a>(s: &mut &'a str) -> PResult<&'a str> {
        delimited('\'', take_till(.., ('\'', char::is_newline)), '\'').parse_next(s)
    }
    fn str_double_unescaped<'a>(s: &mut &'a str) -> PResult<&'a str> {
        delimited('"', string_noescape, '"').parse_next(s)
    }
    fn str_double_escaped<'a>(s: &mut &'a str) -> PResult<String> {
        delimited('"', string_escaped, '"').parse_next(s)
    }
    fn string_escaped(s: &mut &str) -> PResult<String> {
        escaped_transform(string_noescape, '\\', escape).parse_next(s)
    }
    fn string_noescape<'a>(s: &mut &'a str) -> PResult<&'a str> {
        take_till(1.., ('"', '\\', char::is_newline))
            .recognize()
            .parse_next(s)
    }

    fn escape<'a, 'o>(s: &mut &'a str) -> PResult<&'o str> {
        alt((
            'n'.value("\n"),
            't'.value("\t"),
            'r'.value("\r"),
            '\\'.value("\\"),
            '\''.value("\'"),
            '"'.value("\""),
        ))
        .parse_next(s)
    }
    #[cfg(test)]
    mod test {
        use winnow::combinator::repeat;

        use super::*;
        #[test]
        fn full_parse() {
            let mut f = r#"+<!DOCTYPE html>
+  <head>
:insert $page.title
+              <nav id="navbar" style="margin-bottom: 0px;">
:insert "nav.html"
+              </nav>
+              <main/>
:insert $page.content
+              <aside id="leftSidebar" style="margin-right: 0px;">
:insert "leftbar.html"
+              </aside>
+            </div>
+
"#;
            let parsed = super::lines(&mut f).expect("parse failed");
            assert_eq!(
                parsed,
                [
                    Line::Raw("<!DOCTYPE html>",),
                    Line::Raw("  <head>",),
                    Line::Command(Command::Insert(Insert::Var(Var(vec![
                        Ident("page"),
                        Ident("title")
                    ])),),),
                    Line::Raw("              <nav id=\"navbar\" style=\"margin-bottom: 0px;\">",),
                    Line::Command(Command::Insert(Insert::Path(Path::Path(
                        "nav.html".as_ref()
                    ),),),),
                    Line::Raw("              </nav>",),
                    Line::Raw("              <main/>",),
                    Line::Command(Command::Insert(Insert::Var(Var(vec![
                        Ident("page"),
                        Ident("content"),
                    ])),),),
                    Line::Raw(
                        "              <aside id=\"leftSidebar\" style=\"margin-right: 0px;\">",
                    ),
                    Line::Command(Command::Insert(Insert::Path(Path::Path(
                        "leftbar.html".as_ref(),
                    )),),),
                    Line::Raw("              </aside>",),
                    Line::Raw("            </div>",),
                    Line::Raw("",),
                ],
            )
        }
        #[test]
        fn insert() {
            let mut f = r#":insert $page.title
:insert "nav.html"
:insert $page.content
:insert "leftbar.html"
"#;
            let parsed = super::lines(&mut f).expect("parse failed");
            assert_eq!(
                parsed,
                [
                    Line::Command(Command::Insert(Insert::Var(Var(vec![
                        Ident("page"),
                        Ident("title")
                    ])),),),
                    Line::Command(Command::Insert(Insert::Path(Path::Path(
                        "nav.html".as_ref()
                    ),),),),
                    Line::Command(Command::Insert(Insert::Var(Var(vec![
                        Ident("page"),
                        Ident("content")
                    ])),),),
                    Line::Command(Command::Insert(Insert::Path(Path::Path(
                        "leftbar.html".as_ref()
                    ),),),),
                ],
            )
        }
        #[test]
        fn insert_path() {
            let mut f = r#":insert "nav.html"
:insert "leftbar.html"
"#;
            let parsed = super::lines(&mut f).expect("parse failed");
            assert_eq!(
                parsed,
                [
                    Line::Command(Command::Insert(Insert::Path(Path::Path(
                        "nav.html".as_ref()
                    ),),),),
                    Line::Command(Command::Insert(Insert::Path(Path::Path(
                        "leftbar.html".as_ref()
                    ),),),),
                ],
            )
        }
        #[test]
        fn insert_path_escaped() {
            let mut f = r#":insert "nav\r\".h\ntml""#;
            let parsed = super::lines(&mut f).expect("parse failed");
            assert_eq!(
                parsed,
                [Line::Command(Command::Insert(Insert::Path(Path::PathBuf(
                    "nav\r\".h\ntml".into(),
                )),),),],
            )
        }
        #[test]
        fn path_escaped() {
            let mut f = r#""nav\r\".h\ntml""#;
            let parsed = super::str_double_escaped(&mut f).expect("parse failed");
            assert_eq!(parsed, "nav\r\".h\ntml")
        }
        #[test]
        fn basic_escaped() {
            use winnow::ascii::escaped_transform;
            use winnow::prelude::*;
            use winnow::token::none_of;

            fn parser<'s>(input: &mut &'s str) -> PResult<String> {
                let normal = repeat(1.., none_of(('"', '\\'))).fold(|| (), |(), _| ());
                escaped_transform(normal.recognize(), '\\', super::escape).parse_next(input)
                // escaped_transform(alpha1, '\\', super::escape).parse_next(input)
            }

            assert_eq!(
                parser.parse_peek("ab\\\"cd"),
                Ok(("", String::from("ab\"cd")))
            );
            assert_eq!(
                parser.parse_peek("ab\\ncd"),
                Ok(("", String::from("ab\ncd")))
            );
        }
        #[test]
        fn empty_string_escaped() {
            let mut f = "";
            let parsed: String = super::string_escaped(&mut f).expect("parse failed");
            assert_eq!(parsed, "")
        }
        #[test]
        fn string_escaped() {
            let mut f = r#"nav\r\".h\ntml"#;
            let parsed: String = super::string_escaped(&mut f).expect("parse failed");
            assert_eq!(parsed, "nav\r\".h\ntml")
        }
        #[test]
        fn insert_var() {
            let mut f = r#":insert $page.title
:insert $page.content
"#;
            let parsed = super::lines(&mut f).expect("parse failed");
            assert_eq!(
                parsed,
                [
                    Line::Command(Command::Insert(Insert::Var(Var(vec![
                        Ident("page"),
                        Ident("title")
                    ])),),),
                    Line::Command(Command::Insert(Insert::Var(Var(vec![
                        Ident("page"),
                        Ident("content")
                    ])),),),
                ],
            )
        }
        #[test]
        fn raw() {
            let mut f = r#"+<!DOCTYPE html>
+  <head>
+              <nav id="navbar" style="margin-bottom: 0px;">
+              </nav>
+              <main/>
+              <aside id="leftSidebar" style="margin-right: 0px;">
+              </aside>
+            </div>
+
"#;
            let parsed = super::lines(&mut f).expect("parse failed");
            assert_eq!(
                parsed,
                [
                    Line::Raw("<!DOCTYPE html>",),
                    Line::Raw("  <head>",),
                    Line::Raw("              <nav id=\"navbar\" style=\"margin-bottom: 0px;\">",),
                    Line::Raw("              </nav>",),
                    Line::Raw("              <main/>",),
                    Line::Raw(
                        "              <aside id=\"leftSidebar\" style=\"margin-right: 0px;\">",
                    ),
                    Line::Raw("              </aside>",),
                    Line::Raw("            </div>",),
                    Line::Raw("",),
                ],
            )
        }
    }
}
