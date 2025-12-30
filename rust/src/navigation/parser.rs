//! Nom Parser for Navigation Commands
//!
//! This module provides a natural language parser for navigation commands.
//! It uses the Nom parser combinator library for efficient, composable parsing.
//!
//! ## Design Goals
//!
//! 1. **Natural language feel** - "show me the Allianz book" not "LOAD_BOOK Allianz"
//! 2. **Case insensitive** - "Go Up" = "go up" = "GO UP"
//! 3. **Flexible quoting** - `find "AI Fund"` or `find AI Fund` for single words
//! 4. **Clear error messages** - Help users understand what went wrong

use chrono::NaiveDate;
use nom::{
    branch::alt,
    bytes::complete::{tag, tag_no_case, take_until, take_while1},
    character::complete::{char, digit1, multispace1, space0, space1},
    combinator::{all_consuming, map, map_res, opt, value},
    multi::separated_list1,
    sequence::{delimited, preceded, terminated, tuple},
    IResult,
};

use super::commands::{Direction, NavCommand, ZoomLevel};
use crate::graph::ProngFilter;

// =============================================================================
// MAIN PARSER ENTRY POINT
// =============================================================================

/// Parse a navigation command from natural language input
///
/// # Examples
///
/// ```
/// use ob_poc::navigation::parse_nav_command;
///
/// let (remaining, cmd) = parse_nav_command("go up").unwrap();
/// assert!(remaining.is_empty());
/// ```
pub fn parse_nav_command(input: &str) -> IResult<&str, NavCommand> {
    let input = input.trim();

    // Nest alt() calls to stay under the 21-alternative limit
    // Order matters: more specific commands first (e.g., "owners" before "owner")
    all_consuming(alt((
        parse_scope_commands,
        parse_filter_commands,
        parse_query_commands, // Before navigation: "owners" must match before "owner"
        parse_navigation_commands,
        parse_display_commands,
        parse_meta_commands,
    )))(input)
}

/// Parse all scope-related commands
fn parse_scope_commands(input: &str) -> IResult<&str, NavCommand> {
    alt((
        parse_load_cbu,
        parse_load_book,
        parse_load_jurisdiction,
        parse_load_neighborhood,
    ))(input)
}

/// Parse all filter-related commands
fn parse_filter_commands(input: &str) -> IResult<&str, NavCommand> {
    alt((
        parse_filter_jurisdiction,
        parse_filter_fund_type,
        parse_filter_prong,
        parse_filter_min_ownership,
        parse_filter_path_only,
        parse_clear_filters,
        parse_as_of_date,
    ))(input)
}

/// Parse all navigation-related commands
fn parse_navigation_commands(input: &str) -> IResult<&str, NavCommand> {
    alt((
        parse_go_to,
        parse_go_up,
        parse_go_down,
        parse_go_sibling,
        parse_go_terminus,
        parse_go_client,
        parse_go_back,
        parse_go_forward,
    ))(input)
}

/// Parse all query-related commands
fn parse_query_commands(input: &str) -> IResult<&str, NavCommand> {
    alt((
        parse_find,
        parse_where_is,
        parse_find_by_role,
        parse_list_children,
        parse_list_owners,
        parse_list_controllers,
        parse_list_cbus,
    ))(input)
}

/// Parse all display-related commands
fn parse_display_commands(input: &str) -> IResult<&str, NavCommand> {
    alt((
        parse_show_path,
        parse_show_context,
        parse_show_tree,
        parse_expand_cbu,
        parse_collapse_cbu,
        parse_zoom,
        parse_zoom_in,
        parse_zoom_out,
        parse_fit_to_view,
    ))(input)
}

/// Parse all meta commands
fn parse_meta_commands(input: &str) -> IResult<&str, NavCommand> {
    alt((parse_help, parse_undo, parse_redo))(input)
}

// =============================================================================
// HELPER PARSERS
// =============================================================================

/// Parse a quoted string: "some text" or 'some text'
fn quoted_string(input: &str) -> IResult<&str, &str> {
    alt((
        delimited(char('"'), take_until("\""), char('"')),
        delimited(char('\''), take_until("'"), char('\'')),
    ))(input)
}

/// Parse an unquoted word (no spaces)
fn word(input: &str) -> IResult<&str, &str> {
    take_while1(|c: char| !c.is_whitespace())(input)
}

/// Parse a name - either quoted string or single word
fn name(input: &str) -> IResult<&str, String> {
    map(alt((quoted_string, word)), |s: &str| s.to_string())(input)
}

/// Parse a list of jurisdiction codes: "LU, IE, DE" or "LU IE DE"
fn jurisdiction_list(input: &str) -> IResult<&str, Vec<String>> {
    separated_list1(
        alt((preceded(space0, preceded(char(','), space0)), multispace1)),
        map(take_while1(|c: char| c.is_alphanumeric()), |s: &str| {
            s.to_uppercase()
        }),
    )(input)
}

/// Parse a positive integer
fn positive_integer(input: &str) -> IResult<&str, usize> {
    map_res(digit1, |s: &str| s.parse::<usize>())(input)
}

/// Parse a positive float
fn positive_float(input: &str) -> IResult<&str, f64> {
    map_res(
        take_while1(|c: char| c.is_ascii_digit() || c == '.'),
        |s: &str| s.parse::<f64>(),
    )(input)
}

/// Parse optional article "the" or "a"
fn optional_article(input: &str) -> IResult<&str, ()> {
    value(
        (),
        opt(terminated(
            alt((tag_no_case("the"), tag_no_case("a"), tag_no_case("an"))),
            space1,
        )),
    )(input)
}

/// Parse optional "me" after show
fn optional_me(input: &str) -> IResult<&str, ()> {
    value((), opt(terminated(tag_no_case("me"), space1)))(input)
}

// =============================================================================
// SCOPE COMMAND PARSERS
// =============================================================================

/// Parse: load cbu "Name" | show cbu "Name" | cbu "Name"
fn parse_load_cbu(input: &str) -> IResult<&str, NavCommand> {
    let (input, _) = opt(alt((
        terminated(tag_no_case("load"), space1),
        terminated(tag_no_case("show"), space1),
        terminated(tag_no_case("open"), space1),
    )))(input)?;
    let (input, _) = opt(optional_me)(input)?;
    let (input, _) = opt(optional_article)(input)?;
    let (input, _) = tag_no_case("cbu")(input)?;
    let (input, _) = space1(input)?;
    let (input, cbu_name) = name(input)?;

    Ok((input, NavCommand::LoadCbu { cbu_name }))
}

/// Parse: show book "Client" | load book "Client" | show the Allianz book
fn parse_load_book(input: &str) -> IResult<&str, NavCommand> {
    alt((
        // "show [me] [the] X book"
        map(
            tuple((
                alt((
                    tag_no_case("show"),
                    tag_no_case("load"),
                    tag_no_case("open"),
                )),
                space1,
                opt(terminated(tag_no_case("me"), space1)),
                opt(terminated(tag_no_case("the"), space1)),
                name,
                space1,
                tag_no_case("book"),
            )),
            |(_, _, _, _, client_name, _, _)| NavCommand::LoadBook { client_name },
        ),
        // "load book X" | "book X"
        map(
            tuple((
                opt(terminated(
                    alt((tag_no_case("load"), tag_no_case("show"))),
                    space1,
                )),
                tag_no_case("book"),
                space1,
                name,
            )),
            |(_, _, _, client_name)| NavCommand::LoadBook { client_name },
        ),
    ))(input)
}

/// Parse: show jurisdiction LU | focus on Luxembourg | jurisdiction LU
fn parse_load_jurisdiction(input: &str) -> IResult<&str, NavCommand> {
    alt((
        // "show jurisdiction X" | "load jurisdiction X"
        map(
            tuple((
                alt((tag_no_case("show"), tag_no_case("load"))),
                space1,
                tag_no_case("jurisdiction"),
                space1,
                name,
            )),
            |(_, _, _, _, code)| NavCommand::LoadJurisdiction {
                code: code.to_uppercase(),
            },
        ),
        // "jurisdiction X"
        map(
            preceded(tuple((tag_no_case("jurisdiction"), space1)), name),
            |code| NavCommand::LoadJurisdiction {
                code: code.to_uppercase(),
            },
        ),
    ))(input)
}

/// Parse: neighborhood "Entity" [hops N] | around "Entity"
fn parse_load_neighborhood(input: &str) -> IResult<&str, NavCommand> {
    map(
        tuple((
            alt((tag_no_case("neighborhood"), tag_no_case("around"))),
            space1,
            name,
            opt(preceded(
                tuple((space1, tag_no_case("hops"), space1)),
                positive_integer,
            )),
        )),
        |(_, _, entity_name, hops)| NavCommand::LoadNeighborhood {
            entity_name,
            hops: hops.unwrap_or(2) as u32,
        },
    )(input)
}

// =============================================================================
// FILTER COMMAND PARSERS
// =============================================================================

/// Parse: filter jurisdiction LU, IE | focus on LU
fn parse_filter_jurisdiction(input: &str) -> IResult<&str, NavCommand> {
    alt((
        // "filter jurisdiction X, Y"
        map(
            preceded(
                tuple((
                    tag_no_case("filter"),
                    space1,
                    tag_no_case("jurisdiction"),
                    opt(char('s')),
                    space1,
                )),
                jurisdiction_list,
            ),
            |codes| NavCommand::FilterJurisdiction { codes },
        ),
        // "focus on X" (single jurisdiction)
        map(
            preceded(
                tuple((tag_no_case("focus"), space1, tag_no_case("on"), space1)),
                name,
            ),
            |code| NavCommand::FilterJurisdiction {
                codes: vec![code.to_uppercase()],
            },
        ),
    ))(input)
}

/// Parse: filter fund type UCITS, AIF
fn parse_filter_fund_type(input: &str) -> IResult<&str, NavCommand> {
    map(
        preceded(
            tuple((
                tag_no_case("filter"),
                space1,
                tag_no_case("fund"),
                space1,
                tag_no_case("type"),
                opt(char('s')),
                space1,
            )),
            separated_list1(
                alt((preceded(space0, preceded(char(','), space0)), multispace1)),
                map(word, |s| s.to_uppercase()),
            ),
        ),
        |fund_types| NavCommand::FilterFundType { fund_types },
    )(input)
}

/// Parse: show ownership | show control | filter prong ownership
fn parse_filter_prong(input: &str) -> IResult<&str, NavCommand> {
    alt((
        // "show ownership [prong]"
        map(
            tuple((
                tag_no_case("show"),
                space1,
                tag_no_case("ownership"),
                opt(preceded(space1, tag_no_case("prong"))),
            )),
            |_| NavCommand::FilterProng {
                prong: ProngFilter::OwnershipOnly,
            },
        ),
        // "show control [prong]"
        map(
            tuple((
                tag_no_case("show"),
                space1,
                tag_no_case("control"),
                opt(preceded(space1, tag_no_case("prong"))),
            )),
            |_| NavCommand::FilterProng {
                prong: ProngFilter::ControlOnly,
            },
        ),
        // "show both [prongs]"
        map(
            tuple((
                tag_no_case("show"),
                space1,
                tag_no_case("both"),
                opt(preceded(
                    space1,
                    alt((tag_no_case("prongs"), tag_no_case("prong"))),
                )),
            )),
            |_| NavCommand::FilterProng {
                prong: ProngFilter::Both,
            },
        ),
        // "filter prong X"
        map(
            preceded(
                tuple((tag_no_case("filter"), space1, tag_no_case("prong"), space1)),
                alt((
                    value(ProngFilter::OwnershipOnly, tag_no_case("ownership")),
                    value(ProngFilter::ControlOnly, tag_no_case("control")),
                    value(ProngFilter::Both, tag_no_case("both")),
                )),
            ),
            |prong| NavCommand::FilterProng { prong },
        ),
    ))(input)
}

/// Parse: min ownership 25% | minimum ownership 25
fn parse_filter_min_ownership(input: &str) -> IResult<&str, NavCommand> {
    map(
        preceded(
            tuple((
                alt((tag_no_case("min"), tag_no_case("minimum"))),
                space1,
                tag_no_case("ownership"),
                space1,
            )),
            terminated(positive_float, opt(char('%'))),
        ),
        |percentage| NavCommand::FilterMinOwnership { percentage },
    )(input)
}

/// Parse: path only | show path only | path only off
fn parse_filter_path_only(input: &str) -> IResult<&str, NavCommand> {
    alt((map(
        tuple((
            opt(terminated(tag_no_case("show"), space1)),
            tag_no_case("path"),
            space1,
            tag_no_case("only"),
            opt(preceded(
                space1,
                alt((
                    value(false, tag_no_case("off")),
                    value(true, tag_no_case("on")),
                )),
            )),
        )),
        |(_, _, _, _, enabled)| NavCommand::FilterPathOnly {
            enabled: enabled.unwrap_or(true),
        },
    ),))(input)
}

/// Parse: clear filters | reset filters
fn parse_clear_filters(input: &str) -> IResult<&str, NavCommand> {
    value(
        NavCommand::ClearFilters,
        tuple((
            alt((tag_no_case("clear"), tag_no_case("reset"))),
            space1,
            tag_no_case("filter"),
            opt(char('s')),
        )),
    )(input)
}

/// Parse: as of 2024-01-01 | as of date 2024-01-01
fn parse_as_of_date(input: &str) -> IResult<&str, NavCommand> {
    map_res(
        preceded(
            tuple((
                tag_no_case("as"),
                space1,
                tag_no_case("of"),
                space1,
                opt(terminated(tag_no_case("date"), space1)),
            )),
            take_while1(|c: char| c.is_ascii_digit() || c == '-'),
        ),
        |date_str: &str| {
            NaiveDate::parse_from_str(date_str, "%Y-%m-%d")
                .map(|date| NavCommand::AsOfDate { date })
        },
    )(input)
}

// =============================================================================
// NAVIGATION COMMAND PARSERS
// =============================================================================

/// Parse: go to "Entity" | navigate to "Entity" | goto "Entity"
fn parse_go_to(input: &str) -> IResult<&str, NavCommand> {
    map(
        preceded(
            tuple((
                alt((
                    tuple((tag_no_case("go"), space1, tag_no_case("to"))),
                    tuple((tag_no_case("navigate"), space1, tag_no_case("to"))),
                    tuple((tag_no_case("goto"), space0, tag(""))),
                )),
                space1,
            )),
            name,
        ),
        |entity_name| NavCommand::GoTo { entity_name },
    )(input)
}

/// Parse: go up | up | parent | owner
fn parse_go_up(input: &str) -> IResult<&str, NavCommand> {
    alt((
        value(
            NavCommand::GoUp,
            tuple((tag_no_case("go"), space1, tag_no_case("up"))),
        ),
        value(NavCommand::GoUp, tag_no_case("up")),
        value(NavCommand::GoUp, tag_no_case("parent")),
        value(NavCommand::GoUp, tag_no_case("owner")),
    ))(input)
}

/// Parse: go down | down | child | down to "Name" | down 0
fn parse_go_down(input: &str) -> IResult<&str, NavCommand> {
    alt((
        // "down to X" or "go down to X"
        map(
            tuple((
                opt(terminated(tag_no_case("go"), space1)),
                tag_no_case("down"),
                space1,
                tag_no_case("to"),
                space1,
                name,
            )),
            |(_, _, _, _, _, entity_name)| NavCommand::GoDown {
                index: None,
                name: Some(entity_name),
            },
        ),
        // "down N" or "go down N"
        map(
            tuple((
                opt(terminated(tag_no_case("go"), space1)),
                tag_no_case("down"),
                space1,
                positive_integer,
            )),
            |(_, _, _, idx)| NavCommand::GoDown {
                index: Some(idx),
                name: None,
            },
        ),
        // Simple "go down"
        value(
            NavCommand::GoDown {
                index: None,
                name: None,
            },
            tuple((tag_no_case("go"), space1, tag_no_case("down"))),
        ),
        // Simple "down"
        value(
            NavCommand::GoDown {
                index: None,
                name: None,
            },
            tag_no_case("down"),
        ),
        // Simple "child"
        value(
            NavCommand::GoDown {
                index: None,
                name: None,
            },
            tag_no_case("child"),
        ),
    ))(input)
}

/// Parse: left | right | next | prev
fn parse_go_sibling(input: &str) -> IResult<&str, NavCommand> {
    alt((
        // "go left/right/next/prev"
        map(
            preceded(
                tuple((tag_no_case("go"), space1)),
                alt((
                    value(Direction::Left, tag_no_case("left")),
                    value(Direction::Right, tag_no_case("right")),
                    value(Direction::Next, tag_no_case("next")),
                    value(
                        Direction::Prev,
                        alt((tag_no_case("prev"), tag_no_case("previous"))),
                    ),
                )),
            ),
            |direction| NavCommand::GoSibling { direction },
        ),
        // Just "left/right/next/prev"
        map(
            alt((
                value(Direction::Left, tag_no_case("left")),
                value(Direction::Right, tag_no_case("right")),
                value(Direction::Next, tag_no_case("next")),
                value(
                    Direction::Prev,
                    alt((tag_no_case("prev"), tag_no_case("previous"))),
                ),
            )),
            |direction| NavCommand::GoSibling { direction },
        ),
    ))(input)
}

/// Parse: terminus | top | go to terminus
fn parse_go_terminus(input: &str) -> IResult<&str, NavCommand> {
    alt((
        value(
            NavCommand::GoToTerminus,
            tuple((
                tag_no_case("go"),
                space1,
                tag_no_case("to"),
                space1,
                tag_no_case("terminus"),
            )),
        ),
        value(NavCommand::GoToTerminus, tag_no_case("terminus")),
        value(NavCommand::GoToTerminus, tag_no_case("top")),
        value(NavCommand::GoToTerminus, tag_no_case("apex")),
    ))(input)
}

/// Parse: client | go to client
fn parse_go_client(input: &str) -> IResult<&str, NavCommand> {
    alt((
        value(
            NavCommand::GoToClient,
            tuple((
                tag_no_case("go"),
                space1,
                tag_no_case("to"),
                space1,
                tag_no_case("client"),
            )),
        ),
        value(NavCommand::GoToClient, tag_no_case("client")),
    ))(input)
}

/// Parse: back | go back | <
fn parse_go_back(input: &str) -> IResult<&str, NavCommand> {
    alt((
        value(
            NavCommand::GoBack,
            tuple((tag_no_case("go"), space1, tag_no_case("back"))),
        ),
        value(NavCommand::GoBack, tag_no_case("back")),
        value(NavCommand::GoBack, tag("<")),
    ))(input)
}

/// Parse: forward | go forward | >
fn parse_go_forward(input: &str) -> IResult<&str, NavCommand> {
    alt((
        value(
            NavCommand::GoForward,
            tuple((tag_no_case("go"), space1, tag_no_case("forward"))),
        ),
        value(NavCommand::GoForward, tag_no_case("forward")),
        value(NavCommand::GoForward, tag(">")),
    ))(input)
}

// =============================================================================
// QUERY COMMAND PARSERS
// =============================================================================

/// Parse: find "Pattern" | search "Pattern"
fn parse_find(input: &str) -> IResult<&str, NavCommand> {
    map(
        preceded(
            tuple((alt((tag_no_case("find"), tag_no_case("search"))), space1)),
            name,
        ),
        |name_pattern| NavCommand::Find { name_pattern },
    )(input)
}

/// Parse: where is "Person" [a director]
fn parse_where_is(input: &str) -> IResult<&str, NavCommand> {
    map(
        tuple((
            tag_no_case("where"),
            space1,
            tag_no_case("is"),
            space1,
            name,
            opt(preceded(
                tuple((space1, opt(terminated(tag_no_case("a"), space1)))),
                name,
            )),
        )),
        |(_, _, _, _, person_name, role)| NavCommand::WhereIs { person_name, role },
    )(input)
}

/// Parse: find by role director | entities with role X
fn parse_find_by_role(input: &str) -> IResult<&str, NavCommand> {
    alt((
        map(
            preceded(
                tuple((
                    tag_no_case("find"),
                    space1,
                    tag_no_case("by"),
                    space1,
                    tag_no_case("role"),
                    space1,
                )),
                name,
            ),
            |role| NavCommand::FindByRole { role },
        ),
        map(
            preceded(
                tuple((
                    tag_no_case("entities"),
                    space1,
                    tag_no_case("with"),
                    space1,
                    tag_no_case("role"),
                    space1,
                )),
                name,
            ),
            |role| NavCommand::FindByRole { role },
        ),
    ))(input)
}

/// Parse: list children | children | owned
fn parse_list_children(input: &str) -> IResult<&str, NavCommand> {
    alt((
        value(
            NavCommand::ListChildren,
            tuple((tag_no_case("list"), space1, tag_no_case("children"))),
        ),
        value(NavCommand::ListChildren, tag_no_case("children")),
        value(NavCommand::ListChildren, tag_no_case("owned")),
    ))(input)
}

/// Parse: list owners | owners
fn parse_list_owners(input: &str) -> IResult<&str, NavCommand> {
    alt((
        value(
            NavCommand::ListOwners,
            tuple((tag_no_case("list"), space1, tag_no_case("owners"))),
        ),
        value(NavCommand::ListOwners, tag_no_case("owners")),
    ))(input)
}

/// Parse: list controllers | controllers
fn parse_list_controllers(input: &str) -> IResult<&str, NavCommand> {
    alt((
        value(
            NavCommand::ListControllers,
            tuple((tag_no_case("list"), space1, tag_no_case("controllers"))),
        ),
        value(NavCommand::ListControllers, tag_no_case("controllers")),
    ))(input)
}

/// Parse: list cbus | cbus
fn parse_list_cbus(input: &str) -> IResult<&str, NavCommand> {
    alt((
        value(
            NavCommand::ListCbus,
            tuple((tag_no_case("list"), space1, tag_no_case("cbus"))),
        ),
        value(NavCommand::ListCbus, tag_no_case("cbus")),
    ))(input)
}

// =============================================================================
// DISPLAY COMMAND PARSERS
// =============================================================================

/// Parse: show path | path | path to UBO
fn parse_show_path(input: &str) -> IResult<&str, NavCommand> {
    alt((
        value(
            NavCommand::ShowPath,
            tuple((tag_no_case("show"), space1, tag_no_case("path"))),
        ),
        value(
            NavCommand::ShowPath,
            tuple((
                tag_no_case("path"),
                opt(tuple((space1, tag_no_case("to"), space1, word))),
            )),
        ),
    ))(input)
}

/// Parse: show context | context | info
fn parse_show_context(input: &str) -> IResult<&str, NavCommand> {
    alt((
        value(
            NavCommand::ShowContext,
            tuple((tag_no_case("show"), space1, tag_no_case("context"))),
        ),
        value(NavCommand::ShowContext, tag_no_case("context")),
        value(NavCommand::ShowContext, tag_no_case("info")),
    ))(input)
}

/// Parse: show tree 3 | tree depth 3 | tree 3
fn parse_show_tree(input: &str) -> IResult<&str, NavCommand> {
    map(
        tuple((
            opt(terminated(tag_no_case("show"), space1)),
            tag_no_case("tree"),
            opt(preceded(space1, tag_no_case("depth"))),
            opt(preceded(space1, positive_integer)),
        )),
        |(_, _, _, depth)| NavCommand::ShowTree {
            depth: depth.unwrap_or(3) as u32,
        },
    )(input)
}

/// Parse: expand cbu | expand "CBU Name"
fn parse_expand_cbu(input: &str) -> IResult<&str, NavCommand> {
    map(
        preceded(
            tuple((tag_no_case("expand"), space1)),
            alt((
                map(preceded(tuple((tag_no_case("cbu"), space1)), name), Some),
                value(None, tag_no_case("cbu")),
                map(name, Some),
            )),
        ),
        |cbu_name| NavCommand::ExpandCbu { cbu_name },
    )(input)
}

/// Parse: collapse cbu | collapse "CBU Name"
fn parse_collapse_cbu(input: &str) -> IResult<&str, NavCommand> {
    map(
        preceded(
            tuple((tag_no_case("collapse"), space1)),
            alt((
                map(preceded(tuple((tag_no_case("cbu"), space1)), name), Some),
                value(None, tag_no_case("cbu")),
                map(name, Some),
            )),
        ),
        |cbu_name| NavCommand::CollapseCbu { cbu_name },
    )(input)
}

/// Parse: zoom 1.5 | zoom level 2
fn parse_zoom(input: &str) -> IResult<&str, NavCommand> {
    map(
        preceded(
            tuple((
                tag_no_case("zoom"),
                opt(preceded(space1, tag_no_case("level"))),
                space1,
            )),
            alt((
                value(ZoomLevel::Overview, tag_no_case("overview")),
                value(ZoomLevel::Standard, tag_no_case("standard")),
                value(ZoomLevel::Detail, tag_no_case("detail")),
                map(positive_float, |f| ZoomLevel::Custom(f as f32)),
            )),
        ),
        |level| NavCommand::Zoom { level },
    )(input)
}

/// Parse: zoom in | +
fn parse_zoom_in(input: &str) -> IResult<&str, NavCommand> {
    alt((
        value(
            NavCommand::ZoomIn,
            tuple((tag_no_case("zoom"), space1, tag_no_case("in"))),
        ),
        value(NavCommand::ZoomIn, tag("+")),
    ))(input)
}

/// Parse: zoom out | -
fn parse_zoom_out(input: &str) -> IResult<&str, NavCommand> {
    alt((
        value(
            NavCommand::ZoomOut,
            tuple((tag_no_case("zoom"), space1, tag_no_case("out"))),
        ),
        value(NavCommand::ZoomOut, tag("-")),
    ))(input)
}

/// Parse: fit | fit to view
fn parse_fit_to_view(input: &str) -> IResult<&str, NavCommand> {
    alt((
        value(
            NavCommand::FitToView,
            tuple((
                tag_no_case("fit"),
                space1,
                tag_no_case("to"),
                space1,
                tag_no_case("view"),
            )),
        ),
        value(NavCommand::FitToView, tag_no_case("fit")),
    ))(input)
}

// =============================================================================
// META COMMAND PARSERS
// =============================================================================

/// Parse: help | help navigation | ?
fn parse_help(input: &str) -> IResult<&str, NavCommand> {
    alt((
        // "help topic" or "? topic"
        map(preceded(tuple((tag_no_case("help"), space1)), word), |s| {
            NavCommand::Help {
                topic: Some(s.to_string()),
            }
        }),
        map(preceded(tuple((tag("?"), space1)), word), |s| {
            NavCommand::Help {
                topic: Some(s.to_string()),
            }
        }),
        // "help" or "?" alone
        value(NavCommand::Help { topic: None }, tag_no_case("help")),
        value(NavCommand::Help { topic: None }, tag("?")),
    ))(input)
}

/// Parse: undo
fn parse_undo(input: &str) -> IResult<&str, NavCommand> {
    value(NavCommand::Undo, tag_no_case("undo"))(input)
}

/// Parse: redo
fn parse_redo(input: &str) -> IResult<&str, NavCommand> {
    value(NavCommand::Redo, tag_no_case("redo"))(input)
}

// =============================================================================
// TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_ok(input: &str) -> NavCommand {
        parse_nav_command(input)
            .expect(&format!("Failed to parse: {}", input))
            .1
    }

    #[test]
    fn test_scope_commands() {
        assert!(matches!(
            parse_ok("load cbu \"Test Fund\""),
            NavCommand::LoadCbu { cbu_name } if cbu_name == "Test Fund"
        ));

        assert!(matches!(
            parse_ok("show cbu TestFund"),
            NavCommand::LoadCbu { cbu_name } if cbu_name == "TestFund"
        ));

        assert!(matches!(
            parse_ok("show me the Allianz book"),
            NavCommand::LoadBook { client_name } if client_name == "Allianz"
        ));

        assert!(matches!(
            parse_ok("load book \"BlackRock Inc\""),
            NavCommand::LoadBook { client_name } if client_name == "BlackRock Inc"
        ));

        assert!(matches!(
            parse_ok("show jurisdiction LU"),
            NavCommand::LoadJurisdiction { code } if code == "LU"
        ));
    }

    #[test]
    fn test_filter_commands() {
        assert!(matches!(
            parse_ok("filter jurisdiction LU"),
            NavCommand::FilterJurisdiction { codes } if codes == vec!["LU"]
        ));

        assert!(matches!(
            parse_ok("show ownership"),
            NavCommand::FilterProng {
                prong: ProngFilter::OwnershipOnly
            }
        ));

        assert!(matches!(
            parse_ok("show control prong"),
            NavCommand::FilterProng {
                prong: ProngFilter::ControlOnly
            }
        ));

        assert!(matches!(
            parse_ok("clear filters"),
            NavCommand::ClearFilters
        ));

        assert!(matches!(
            parse_ok("min ownership 25%"),
            NavCommand::FilterMinOwnership { percentage } if (percentage - 25.0).abs() < 0.001
        ));
    }

    #[test]
    fn test_navigation_commands() {
        assert!(matches!(parse_ok("go up"), NavCommand::GoUp));
        assert!(matches!(parse_ok("up"), NavCommand::GoUp));
        assert!(matches!(parse_ok("parent"), NavCommand::GoUp));

        assert!(matches!(
            parse_ok("go down"),
            NavCommand::GoDown {
                index: None,
                name: None
            }
        ));

        assert!(matches!(
            parse_ok("down 2"),
            NavCommand::GoDown {
                index: Some(2),
                name: None
            }
        ));

        assert!(matches!(
            parse_ok("down to \"SubFund A\""),
            NavCommand::GoDown { index: None, name: Some(n) } if n == "SubFund A"
        ));

        assert!(matches!(parse_ok("back"), NavCommand::GoBack));
        assert!(matches!(parse_ok("<"), NavCommand::GoBack));
        assert!(matches!(parse_ok("forward"), NavCommand::GoForward));
        assert!(matches!(parse_ok(">"), NavCommand::GoForward));

        assert!(matches!(parse_ok("terminus"), NavCommand::GoToTerminus));
        assert!(matches!(parse_ok("top"), NavCommand::GoToTerminus));
    }

    #[test]
    fn test_query_commands() {
        assert!(matches!(
            parse_ok("find \"AI Fund\""),
            NavCommand::Find { name_pattern } if name_pattern == "AI Fund"
        ));

        assert!(matches!(
            parse_ok("where is \"Hans Schmidt\""),
            NavCommand::WhereIs { person_name, role: None } if person_name == "Hans Schmidt"
        ));

        assert!(matches!(
            parse_ok("where is Hans a director"),
            NavCommand::WhereIs { person_name, role: Some(r) } if person_name == "Hans" && r == "director"
        ));

        assert!(matches!(
            parse_ok("list children"),
            NavCommand::ListChildren
        ));
        assert!(matches!(parse_ok("owners"), NavCommand::ListOwners));
    }

    #[test]
    fn test_display_commands() {
        assert!(matches!(parse_ok("show path"), NavCommand::ShowPath));
        assert!(matches!(parse_ok("context"), NavCommand::ShowContext));

        assert!(matches!(
            parse_ok("show tree 5"),
            NavCommand::ShowTree { depth: 5 }
        ));

        assert!(matches!(
            parse_ok("tree"),
            NavCommand::ShowTree { depth: 3 }
        ));

        assert!(matches!(parse_ok("zoom in"), NavCommand::ZoomIn));
        assert!(matches!(parse_ok("+"), NavCommand::ZoomIn));
        assert!(matches!(parse_ok("zoom out"), NavCommand::ZoomOut));
        assert!(matches!(parse_ok("-"), NavCommand::ZoomOut));
        assert!(matches!(parse_ok("fit"), NavCommand::FitToView));
    }

    #[test]
    fn test_meta_commands() {
        assert!(matches!(parse_ok("help"), NavCommand::Help { topic: None }));
        assert!(matches!(
            parse_ok("help navigation"),
            NavCommand::Help { topic: Some(t) } if t == "navigation"
        ));
        assert!(matches!(parse_ok("undo"), NavCommand::Undo));
        assert!(matches!(parse_ok("redo"), NavCommand::Redo));
    }

    #[test]
    fn test_case_insensitive() {
        assert!(matches!(parse_ok("GO UP"), NavCommand::GoUp));
        assert!(matches!(parse_ok("Go Up"), NavCommand::GoUp));
        assert!(matches!(parse_ok("gO uP"), NavCommand::GoUp));
    }

    #[test]
    fn test_as_of_date() {
        let cmd = parse_ok("as of 2024-01-15");
        if let NavCommand::AsOfDate { date } = cmd {
            assert_eq!(date, NaiveDate::from_ymd_opt(2024, 1, 15).unwrap());
        } else {
            panic!("Expected AsOfDate command");
        }
    }
}
