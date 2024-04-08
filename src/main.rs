use std::{
    fs::File,
    io::Read,
    path::{Path, PathBuf},
};

use clap::Parser as _;
use winnow::Parser as _;

fn main() {
    let App {
        template: template_path,
    } = App::parse();
    let template = {
        let mut template = std::fs::OpenOptions::new()
            .read(true)
            .open(&template_path)
            .expect("Could not open template file");
        let mut s = String::new();
        template
            .read_to_string(&mut s)
            .expect("could not read template file");
        s
    };
    let mut template = template.as_str();
    let template_parsed = parser::lines
        .parse_next(&mut template)
        .expect("parsing error");
    for item in template_parsed {
        match item {
            parser::Line::Raw(r) => println!("{}", r),
            parser::Line::Command(parser::Command::Insert(i)) => match i {
                parser::Insert::Path(p) => insert_path(&template_path, p),
                parser::Insert::Var(v) => insert_var(v),
            },
        }
    }
}

fn insert_var(v: parser::Var<'_>) {
    // todo!();
    println!("=== VAR === {:?}", v);
}

fn insert_path(from: &Path, p: parser::Path<'_>) {
    let mut path = from
        .parent()
        .expect("path is to a file, so it must have a parent")
        .to_path_buf();
    path.push(p);
    let mut s = String::new();
    File::open(path)
        .expect("could not open")
        .read_to_string(&mut s)
        .expect("falied to read");
    println!("{}", s);
}

#[derive(clap::Parser)]
pub struct App {
    template: PathBuf,
}

mod parser {
    use std::path::{Path as FilePath, PathBuf as FilePathBuf};

    use winnow::{
        ascii::escaped_transform,
        combinator::{alt, cut_err, delimited, preceded, repeat, rest, separated},
        stream::AsChar,
        token::{any, literal, none_of, take_till, take_until},
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
    pub struct Ident<'a>(&'a str);
    #[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
    pub struct Var<'a>(Vec<Ident<'a>>);

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
            use winnow::ascii::alpha0;
            use winnow::ascii::alpha1;
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
