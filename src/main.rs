use std::path::Path;

fn main() {
    println!("Hello, world!");
}

enum Cmd<'a> {
    Raw(&'a str),
    Insert(Insert<'a>),
}
enum Insert<'a> {
    Path(&'a Path),
    Var(&'a str),
}

mod parse {
    use std::path::Path;

    use winnow::{
        token::{any, literal, take_until},
        PResult,
    };

    use crate::Cmd;
    use winnow::Parser;

    pub fn parse<'a>(s: &'a str) -> PResult<Vec<Cmd<'a>>> {
        let mut s = s;
        commands(&mut s)
    }

    fn commands<'a>(s: &mut &'a str) -> PResult<Vec<Cmd<'a>>> {
        command
    }
    fn command<'a>(s: &mut &'a str) -> PResult<Cmd<'a>> {
        todo!()
    }
    fn path<'a>(s: &mut &'a str) -> PResult<&'a Path> {
        literal("\"");
        take_until("\"").parse_peek(s)
    }
}
