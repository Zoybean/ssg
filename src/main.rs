use std::{io::Read, path::PathBuf};

use clap::Parser as _;
use winnow::Parser;

fn main() {
    let App { template } = App::parse();
    let template = {
        let mut template = std::fs::OpenOptions::new()
            .read(true)
            .open(template)
            .expect("Could not open template file");
        let mut s = String::new();
        template
            .read_to_string(&mut s)
            .expect("could not read template file");
        s
    };
    let mut template = template.as_str();
    let template_parsed = parser::lines.parse_next(&mut template);
    println!("({:#?}), {}", template_parsed, template);
}

#[derive(clap::Parser)]
pub struct App {
    template: PathBuf,
}

mod parser {
    use std::path::{Path, PathBuf};

    use winnow::{
        ascii::escaped_transform,
        combinator::{alt, cut_err, delimited, preceded, repeat, rest, separated},
        stream::AsChar,
        token::{any, literal, take_till, take_until},
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
        Path(&'a Path),
        Var(&'a str),
        PathBuf(PathBuf),
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
        alt((
            path_single.map(Insert::Path),
            path_unescaped.map(Insert::Path),
            path_escaped.map(Insert::PathBuf),
            quoted("\'").map(AsRef::as_ref).map(Insert::Path),
            preceded('$', take_till(.., char::is_newline)).map(Insert::Var),
        ))
        .parse_next(s)
    }

    fn path_single<'a>(s: &mut &'a str) -> PResult<&'a Path> {
        quoted("\'").map(AsRef::as_ref).parse_next(s)
    }
    fn path_unescaped<'a>(s: &mut &'a str) -> PResult<&'a Path> {
        quoted("\"").map(AsRef::as_ref).parse_next(s)
    }
    fn path_escaped<'a>(s: &mut &'a str) -> PResult<PathBuf> {
        delimited(
            '"',
            escaped_transform(take_until(.., '"'), '\\', escape).map(String::into),
            '"',
        )
        .parse_next(s)
    }

    fn quoted(q: &str) -> impl Parser<&str, &str, winnow::error::ContextError> {
        delimited(q, take_until(.., q), q)
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
                    Line::Command(Command::Insert(Insert::Var("page.title",),),),
                    Line::Raw("              <nav id=\"navbar\" style=\"margin-bottom: 0px;\">",),
                    Line::Command(Command::Insert(Insert::Path("nav.html".as_ref(),),),),
                    Line::Raw("              </nav>",),
                    Line::Raw("              <main/>",),
                    Line::Command(Command::Insert(Insert::Var("page.content",),),),
                    Line::Raw(
                        "              <aside id=\"leftSidebar\" style=\"margin-right: 0px;\">",
                    ),
                    Line::Command(Command::Insert(Insert::Path("leftbar.html".as_ref(),),),),
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
                    Line::Command(Command::Insert(Insert::Var("page.title",),),),
                    Line::Command(Command::Insert(Insert::Path("nav.html".as_ref(),),),),
                    Line::Command(Command::Insert(Insert::Var("page.content",),),),
                    Line::Command(Command::Insert(Insert::Path("leftbar.html".as_ref(),),),),
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
                    Line::Command(Command::Insert(Insert::Path("nav.html".as_ref(),),),),
                    Line::Command(Command::Insert(Insert::Path("leftbar.html".as_ref(),),),),
                ],
            )
        }
        #[test]
        fn insert_path_escaped() {
            let mut f = r#":insert "nav\r\".h\ntml""#;
            let parsed = super::lines(&mut f).expect("parse failed");
            assert_eq!(
                parsed,
                [Line::Command(Command::Insert(Insert::PathBuf(
                    "nav\r\".h\ntml".into(),
                ),),),],
            )
        }
        #[test]
        fn path_escaped() {
            let mut f = r#""nav\r\".h\ntml""#;
            let parsed: PathBuf = super::path_escaped(&mut f).expect("parse failed");
            assert_eq!(parsed, PathBuf::from("nav\r\".h\ntml"))
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
                    Line::Command(Command::Insert(Insert::Var("page.title",),),),
                    Line::Command(Command::Insert(Insert::Var("page.content",),),),
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
