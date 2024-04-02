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
    use std::path::Path;

    use winnow::{
        combinator::{alt, cut_err, delimited, preceded, repeat, rest, separated},
        stream::AsChar,
        token::{any, literal, take_till, take_until},
        PResult, Parser,
    };
    #[derive(Debug)]
    pub enum Line<'a> {
        Raw(&'a str),
        Command(Command<'a>),
    }
    #[derive(Debug)]
    pub enum Command<'a> {
        Insert(Insert<'a>),
    }
    #[derive(Debug)]
    pub enum Insert<'a> {
        Path(&'a Path),
        Var(&'a str),
    }

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
            quoted("\"").map(AsRef::as_ref).map(Insert::Path),
            quoted("\'").map(AsRef::as_ref).map(Insert::Path),
            preceded('$', take_till(.., char::is_newline)).map(Insert::Var),
        ))
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
}
