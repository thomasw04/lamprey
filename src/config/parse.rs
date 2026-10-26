use alloc::format;
use anyhow::{Context, Result, anyhow};
use nom::{
    IResult, Parser,
    branch::alt,
    bytes::complete::take_while1,
    character::complete::{char, digit1, multispace0, not_line_ending, space0},
    combinator::{all_consuming, iterator, map, map_res},
    error::{Error as ParseError, ErrorKind},
    sequence::{preceded, terminated},
};

use crate::config::{Component, Directive};

fn word(input: &str) -> IResult<&str, &str> {
    terminated(take_while1(|c: char| !c.is_ascii_whitespace()), space0).parse(input)
}

fn decimal(input: &str) -> IResult<&str, usize> {
    map_res(terminated(digit1, space0), str::parse::<usize>).parse(input)
}

fn component(input: &str) -> IResult<&str, Component<'_>> {
    let (tail, cmdline) = not_line_ending(input)?;
    let (_, path) = word(cmdline)?;
    Ok((tail, Component { path, cmdline }))
}

fn comment(input: &str) -> IResult<&str, Directive<'_>> {
    preceded(char('#'), not_line_ending)
        .parse(input)
        .map(|(tail, comment)| (tail, Directive::Comment(comment)))
}

fn directive(input: &str) -> IResult<&str, Directive<'_>> {
    let (tail, dir) = word(input)?;

    match dir {
        "bootdriver" => map(component, Directive::BootDriver).parse(tail),
        "cpudriver" => map(component, Directive::CpuDriver).parse(tail),
        "module" => map(component, Directive::Module).parse(tail),
        "stack" => map(decimal, Directive::Stack).parse(tail),
        _ => Err(nom::Err::Failure(ParseError::new(input, ErrorKind::Tag))),
    }
}

fn parse(input: &str) -> IResult<&str, Directive<'_>> {
    preceded(multispace0, alt((comment, directive))).parse(input)
}

pub fn for_each(content: &str, mut fun: impl FnMut(&Directive) -> Result<()>) -> Result<()> {
    let mut directives = iterator(content, parse);
    for dir in &mut directives {
        fun(&dir).context(format!("failed to process directive {}", dir))?;
    }

    directives
        .finish()
        .and_then(|(tail, ())| all_consuming(multispace0).parse(tail))
        .map(|_| ())
        .map_err(|err| anyhow!("{err}"))
}
