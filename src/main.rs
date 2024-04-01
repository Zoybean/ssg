use std::path::Path;

use winnow::{
    combinator::{alt, cut_err, delimited, preceded, repeat, rest},
    token::{any, literal, take_until},
    PResult, Parser,
};

fn main() {
    println!("Hello, world!");
}

enum Line<'a> {
    Raw(&'a str),
    Insert(Insert<'a>),
}
enum Insert<'a> {
    Path(&'a Path),
    Var(&'a str),
}

fn lines<'a>(s: &mut &'a str) -> PResult<Vec<Line<'a>>> {
    repeat(.., line).parse_next(s)
}
fn line<'a>(s: &mut &'a str) -> PResult<Line<'a>> {
    alt((
        preceded(':', cut_err(insert)).map(Line::Insert),
        preceded('+', cut_err(raw)).map(Line::Raw),
    ))
    .parse_next(s)
}
fn raw<'a>(s: &mut &'a str) -> PResult<&'a str> {
    rest.parse_next(s)
}
fn insert<'a>(s: &mut &'a str) -> PResult<Insert<'a>> {
    todo!()
}
fn path<'a>(s: &mut &'a str) -> PResult<&'a Path> {
    delimited("\"", take_until(.., "\""), "\"")
        .map(AsRef::as_ref)
        .parse_next(s)
}
