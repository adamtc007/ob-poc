//! Custom parser combinators

use nom::IResult;

// Placeholder for future custom combinators
pub fn placeholder(input: &str) -> IResult<&str, ()> {
    Ok((input, ()))
}
