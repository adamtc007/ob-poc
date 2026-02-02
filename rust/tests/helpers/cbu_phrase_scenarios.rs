//! Extended CBU phrase test scenarios for accelerated learning validation
//!
//! This module provides comprehensive test scenarios for CBU-related verbs.
//! Use with the verb_search_integration harness to:
//! 1. Identify phrases that DON'T currently work (need teaching)
//! 2. Validate that taught phrases match correctly
//! 3. Detect regressions after threshold tuning
//!
//! Run: cargo test --features database --test verb_search_integration test_cbu_extended -- --ignored --nocapture

#![allow(unused_imports)]
use crate::{ExpectedOutcome, TestScenario};

/// Extended CBU create scenarios - common ways users ask to create funds/structures
pub fn cbu_create_scenarios() -> Vec<TestScenario> {
    vec![
        // Core phrases
        TestScenario::matched("create cbu direct", "create cbu", "cbu.create")
            .with_category("cbu_create"),
        TestScenario::matched("create a cbu", "create a cbu", "cbu.create")
            .with_category("cbu_create"),
        TestScenario::matched("new cbu", "new cbu", "cbu.create").with_category("cbu_create"),
        TestScenario::matched("add cbu", "add cbu", "cbu.create").with_category("cbu_create"),
        // Fund terminology
        TestScenario::matched("create fund", "create a fund", "cbu.create")
            .with_category("cbu_create"),
        TestScenario::matched("new fund", "new fund", "cbu.create").with_category("cbu_create"),
        TestScenario::matched("set up fund", "set up a fund", "cbu.create")
            .with_category("cbu_create"),
        TestScenario::matched("spin up fund", "spin up a fund", "cbu.create")
            .with_category("cbu_create"),
        TestScenario::matched("establish fund", "establish a fund", "cbu.create")
            .with_category("cbu_create"),
        // Structure terminology
        TestScenario::matched("create structure", "create a structure", "cbu.create")
            .with_category("cbu_create"),
        TestScenario::matched("new structure", "new structure", "cbu.create")
            .with_category("cbu_create"),
        TestScenario::matched("set up structure", "set up a structure", "cbu.create")
            .with_category("cbu_create"),
        TestScenario::matched("add structure", "add a new structure", "cbu.create")
            .with_category("cbu_create"),
        // Trading unit terminology
        TestScenario::matched("create trading unit", "create a trading unit", "cbu.create")
            .with_category("cbu_create"),
        TestScenario::matched("new trading unit", "new trading unit", "cbu.create")
            .with_category("cbu_create"),
        // Client/account terminology
        TestScenario::matched("onboard client", "onboard a new client", "cbu.create")
            .with_category("cbu_create"),
        TestScenario::matched("register client", "register a client", "cbu.create")
            .with_category("cbu_create"),
        TestScenario::matched("create account", "create an account", "cbu.create")
            .with_category("cbu_create"),
        TestScenario::matched("open account", "open an account", "cbu.create")
            .with_category("cbu_create"),
        // Fund type variations
        TestScenario::matched("create sicav", "create a sicav", "cbu.create")
            .with_category("cbu_create"),
        TestScenario::matched("set up sicav", "set up a sicav", "cbu.create")
            .with_category("cbu_create"),
        TestScenario::matched("create ucits", "create a ucits fund", "cbu.create")
            .with_category("cbu_create"),
        TestScenario::matched("set up ucits", "set up ucits", "cbu.create")
            .with_category("cbu_create"),
        TestScenario::matched("create pe fund", "create a pe fund", "cbu.create")
            .with_category("cbu_create"),
        TestScenario::matched(
            "create private equity",
            "create a private equity fund",
            "cbu.create",
        )
        .with_category("cbu_create"),
        TestScenario::matched("create hedge fund", "create a hedge fund", "cbu.create")
            .with_category("cbu_create"),
        TestScenario::matched("set up hedge fund", "set up hedge fund", "cbu.create")
            .with_category("cbu_create"),
        TestScenario::matched(
            "create segregated mandate",
            "create a segregated mandate",
            "cbu.create",
        )
        .with_category("cbu_create"),
        TestScenario::matched("create subfund", "create a subfund", "cbu.create")
            .with_category("cbu_create"),
        TestScenario::matched("add subfund", "add a subfund", "cbu.create")
            .with_category("cbu_create"),
        TestScenario::matched("create compartment", "create a compartment", "cbu.create")
            .with_category("cbu_create"),
        // With entity names
        TestScenario::matched(
            "create fund named",
            "create a fund called Alpha Growth",
            "cbu.create",
        )
        .with_category("cbu_create"),
        TestScenario::matched(
            "spin up named",
            "spin up a fund for Acme Corp",
            "cbu.create",
        )
        .with_category("cbu_create"),
        TestScenario::matched(
            "onboard specific",
            "onboard Blackrock Alpha Fund",
            "cbu.create",
        )
        .with_category("cbu_create"),
    ]
}

/// CBU list scenarios - viewing/querying CBU inventory
pub fn cbu_list_scenarios() -> Vec<TestScenario> {
    vec![
        TestScenario::matched("list cbus", "list cbus", "cbu.list").with_category("cbu_list"),
        TestScenario::matched("list all cbus", "list all cbus", "cbu.list")
            .with_category("cbu_list"),
        TestScenario::matched("show cbus", "show all cbus", "cbu.list").with_category("cbu_list"),
        TestScenario::matched("show me cbus", "show me all cbus", "cbu.list")
            .with_category("cbu_list"),
        // Fund terminology
        TestScenario::matched("list funds", "list all funds", "cbu.list").with_category("cbu_list"),
        TestScenario::matched("show funds", "show all funds", "cbu.list").with_category("cbu_list"),
        TestScenario::matched("show me funds", "show me all funds", "cbu.list")
            .with_category("cbu_list"),
        TestScenario::matched("what funds exist", "what funds exist", "cbu.list")
            .with_category("cbu_list"),
        TestScenario::matched("what funds do we have", "what funds do we have", "cbu.list")
            .with_category("cbu_list"),
        // Structure terminology
        TestScenario::matched("list structures", "list structures", "cbu.list")
            .with_category("cbu_list"),
        TestScenario::matched("show structures", "show all structures", "cbu.list")
            .with_category("cbu_list"),
        TestScenario::matched("what structures exist", "what structures exist", "cbu.list")
            .with_category("cbu_list"),
        // Client/account terminology
        TestScenario::matched("list clients", "list all clients", "cbu.list")
            .with_category("cbu_list"),
        TestScenario::matched("show clients", "show clients", "cbu.list").with_category("cbu_list"),
        TestScenario::matched("list accounts", "list accounts", "cbu.list")
            .with_category("cbu_list"),
        // With filters
        TestScenario::matched("list lux funds", "list luxembourg funds", "cbu.list")
            .with_category("cbu_list"),
        TestScenario::matched("show irish funds", "show irish funds", "cbu.list")
            .with_category("cbu_list"),
        TestScenario::matched("funds by jurisdiction", "funds by jurisdiction", "cbu.list")
            .with_category("cbu_list"),
        TestScenario::matched("active cbus", "show active cbus", "cbu.list")
            .with_category("cbu_list"),
        // Questions
        TestScenario::matched("how many cbus", "how many cbus", "cbu.list")
            .with_category("cbu_list"),
        TestScenario::matched("how many funds", "how many funds do we have", "cbu.list")
            .with_category("cbu_list"),
    ]
}

/// CBU update scenarios - modifying existing CBUs
pub fn cbu_update_scenarios() -> Vec<TestScenario> {
    vec![
        TestScenario::matched("update cbu", "update cbu", "cbu.update").with_category("cbu_update"),
        TestScenario::matched("update the cbu", "update the cbu", "cbu.update")
            .with_category("cbu_update"),
        TestScenario::matched("modify cbu", "modify cbu", "cbu.update").with_category("cbu_update"),
        TestScenario::matched("edit cbu", "edit cbu", "cbu.update").with_category("cbu_update"),
        // Fund terminology
        TestScenario::matched("update fund", "update fund details", "cbu.update")
            .with_category("cbu_update"),
        TestScenario::matched("modify fund", "modify the fund", "cbu.update")
            .with_category("cbu_update"),
        TestScenario::matched("edit fund", "edit fund details", "cbu.update")
            .with_category("cbu_update"),
        // Specific updates
        TestScenario::matched("rename cbu", "rename cbu", "cbu.update").with_category("cbu_update"),
        TestScenario::matched("rename fund", "rename the fund", "cbu.update")
            .with_category("cbu_update"),
        TestScenario::matched("change fund name", "change fund name", "cbu.update")
            .with_category("cbu_update"),
        TestScenario::matched("change cbu status", "change cbu status", "cbu.update")
            .with_category("cbu_update"),
        TestScenario::matched("update status", "update cbu status", "cbu.update")
            .with_category("cbu_update"),
        TestScenario::matched(
            "change jurisdiction",
            "change cbu jurisdiction",
            "cbu.update",
        )
        .with_category("cbu_update"),
        // Actions
        TestScenario::matched("activate cbu", "activate the cbu", "cbu.update")
            .with_category("cbu_update"),
        TestScenario::matched("activate fund", "activate fund", "cbu.update")
            .with_category("cbu_update"),
        TestScenario::matched("deactivate cbu", "deactivate cbu", "cbu.update")
            .with_category("cbu_update"),
    ]
}

/// CBU role assignment scenarios - adding parties to CBUs
pub fn cbu_assign_role_scenarios() -> Vec<TestScenario> {
    vec![
        // Generic role
        TestScenario::matched("assign role", "assign role to cbu", "cbu.assign-role")
            .with_category("cbu_role"),
        TestScenario::matched("add role", "add role to entity", "cbu.assign-role")
            .with_category("cbu_role"),
        // Director
        TestScenario::matched("add director", "add director to fund", "cbu.assign-role")
            .with_category("cbu_role"),
        TestScenario::matched("assign director", "assign director", "cbu.assign-role")
            .with_category("cbu_role"),
        TestScenario::matched("appoint director", "appoint director", "cbu.assign-role")
            .with_category("cbu_role"),
        TestScenario::matched(
            "make director",
            "make john smith a director",
            "cbu.assign-role",
        )
        .with_category("cbu_role"),
        // UBO
        TestScenario::matched("add ubo", "add ubo to fund", "cbu.assign-role")
            .with_category("cbu_role"),
        TestScenario::matched("assign ubo", "assign ubo", "cbu.assign-role")
            .with_category("cbu_role"),
        TestScenario::matched(
            "add beneficial owner",
            "add beneficial owner",
            "cbu.assign-role",
        )
        .with_category("cbu_role"),
        // Signatory
        TestScenario::matched("add signatory", "add signatory to fund", "cbu.assign-role")
            .with_category("cbu_role"),
        TestScenario::matched("assign signatory", "assign signatory", "cbu.assign-role")
            .with_category("cbu_role"),
        TestScenario::matched(
            "add authorized signatory",
            "add authorized signatory",
            "cbu.assign-role",
        )
        .with_category("cbu_role"),
        // Shareholder
        TestScenario::matched(
            "add shareholder",
            "add shareholder to fund",
            "cbu.assign-role",
        )
        .with_category("cbu_role"),
        TestScenario::matched(
            "assign shareholder",
            "assign shareholder",
            "cbu.assign-role",
        )
        .with_category("cbu_role"),
        // Service providers
        TestScenario::matched("add manco", "add manco to fund", "cbu.assign-role")
            .with_category("cbu_role"),
        TestScenario::matched(
            "assign management company",
            "assign management company",
            "cbu.assign-role",
        )
        .with_category("cbu_role"),
        TestScenario::matched(
            "add investment manager",
            "add investment manager",
            "cbu.assign-role",
        )
        .with_category("cbu_role"),
        TestScenario::matched("assign im", "assign im to fund", "cbu.assign-role")
            .with_category("cbu_role"),
        TestScenario::matched("add depositary", "add depositary", "cbu.assign-role")
            .with_category("cbu_role"),
        TestScenario::matched(
            "assign depositary",
            "assign depositary to fund",
            "cbu.assign-role",
        )
        .with_category("cbu_role"),
        TestScenario::matched("add custodian", "add custodian to fund", "cbu.assign-role")
            .with_category("cbu_role"),
        TestScenario::matched("assign custodian", "assign custodian", "cbu.assign-role")
            .with_category("cbu_role"),
        TestScenario::matched("add auditor", "add auditor to fund", "cbu.assign-role")
            .with_category("cbu_role"),
        TestScenario::matched("add administrator", "add administrator", "cbu.assign-role")
            .with_category("cbu_role"),
        TestScenario::matched(
            "add transfer agent",
            "add transfer agent",
            "cbu.assign-role",
        )
        .with_category("cbu_role"),
        TestScenario::matched("add prime broker", "add prime broker", "cbu.assign-role")
            .with_category("cbu_role"),
        // GP/LP
        TestScenario::matched("add gp", "add gp to fund", "cbu.assign-role")
            .with_category("cbu_role"),
        TestScenario::matched(
            "assign general partner",
            "assign general partner",
            "cbu.assign-role",
        )
        .with_category("cbu_role"),
        TestScenario::matched("add lp", "add lp to fund", "cbu.assign-role")
            .with_category("cbu_role"),
        TestScenario::matched(
            "assign limited partner",
            "assign limited partner",
            "cbu.assign-role",
        )
        .with_category("cbu_role"),
        // Generic party/entity
        TestScenario::matched("add party", "add party to fund", "cbu.assign-role")
            .with_category("cbu_role"),
        TestScenario::matched(
            "add participant",
            "add participant to cbu",
            "cbu.assign-role",
        )
        .with_category("cbu_role"),
        TestScenario::matched("link entity", "link entity to fund", "cbu.assign-role")
            .with_category("cbu_role"),
        TestScenario::matched("connect entity", "connect entity to cbu", "cbu.assign-role")
            .with_category("cbu_role"),
    ]
}

/// CBU remove role scenarios
pub fn cbu_remove_role_scenarios() -> Vec<TestScenario> {
    vec![
        TestScenario::matched("remove role", "remove role from cbu", "cbu.remove-role")
            .with_category("cbu_role_remove"),
        TestScenario::matched("unassign role", "unassign role", "cbu.remove-role")
            .with_category("cbu_role_remove"),
        TestScenario::matched(
            "remove director",
            "remove director from fund",
            "cbu.remove-role",
        )
        .with_category("cbu_role_remove"),
        TestScenario::matched("remove ubo", "remove ubo from fund", "cbu.remove-role")
            .with_category("cbu_role_remove"),
        TestScenario::matched(
            "remove signatory",
            "remove signatory from fund",
            "cbu.remove-role",
        )
        .with_category("cbu_role_remove"),
        TestScenario::matched(
            "unlink entity",
            "unlink entity from fund",
            "cbu.remove-role",
        )
        .with_category("cbu_role_remove"),
        TestScenario::matched(
            "remove participant",
            "remove participant from cbu",
            "cbu.remove-role",
        )
        .with_category("cbu_role_remove"),
        TestScenario::matched(
            "delete role assignment",
            "delete role assignment",
            "cbu.remove-role",
        )
        .with_category("cbu_role_remove"),
    ]
}

/// CBU parties scenarios - listing who is on a fund
pub fn cbu_parties_scenarios() -> Vec<TestScenario> {
    vec![
        TestScenario::matched("show parties", "show fund parties", "cbu.parties")
            .with_category("cbu_parties"),
        TestScenario::matched("list parties", "list parties on fund", "cbu.parties")
            .with_category("cbu_parties"),
        TestScenario::matched("who is on fund", "who is on this fund", "cbu.parties")
            .with_category("cbu_parties"),
        TestScenario::matched("who is on cbu", "who is on this cbu", "cbu.parties")
            .with_category("cbu_parties"),
        TestScenario::matched("show participants", "show participants", "cbu.parties")
            .with_category("cbu_parties"),
        TestScenario::matched(
            "list participants",
            "list participants on cbu",
            "cbu.parties",
        )
        .with_category("cbu_parties"),
        TestScenario::matched("show fund roles", "show fund roles", "cbu.parties")
            .with_category("cbu_parties"),
        TestScenario::matched("list roles", "list roles on fund", "cbu.parties")
            .with_category("cbu_parties"),
        TestScenario::matched(
            "who are the directors",
            "who are the directors",
            "cbu.parties",
        )
        .with_category("cbu_parties"),
        TestScenario::matched("who are the ubos", "who are the ubos", "cbu.parties")
            .with_category("cbu_parties"),
        TestScenario::matched(
            "who are the signatories",
            "who are the signatories",
            "cbu.parties",
        )
        .with_category("cbu_parties"),
        TestScenario::matched("fund roster", "show fund roster", "cbu.parties")
            .with_category("cbu_parties"),
        TestScenario::matched("cbu roster", "get cbu roster", "cbu.parties")
            .with_category("cbu_parties"),
    ]
}

/// CBU delete scenarios (safety-critical - use safety_first)
pub fn cbu_delete_scenarios() -> Vec<TestScenario> {
    vec![
        // Regular delete - safety first (ambiguity with cascade is acceptable)
        TestScenario::safety_first("delete cbu", "delete cbu", "cbu.delete")
            .with_alternatives(&["cbu.delete-cascade"])
            .with_category("cbu_delete"),
        TestScenario::safety_first("delete fund", "delete fund", "cbu.delete")
            .with_alternatives(&["cbu.delete-cascade"])
            .with_category("cbu_delete"),
        TestScenario::safety_first("remove cbu", "remove cbu", "cbu.delete")
            .with_alternatives(&["cbu.delete-cascade"])
            .with_category("cbu_delete"),
        TestScenario::safety_first("remove fund", "remove fund", "cbu.delete")
            .with_alternatives(&["cbu.delete-cascade"])
            .with_category("cbu_delete"),
        // Cascade delete - explicit
        TestScenario::matched("cascade delete", "cascade delete cbu", "cbu.delete-cascade")
            .with_category("cbu_delete"),
        TestScenario::matched(
            "delete with all",
            "delete cbu and all related data",
            "cbu.delete-cascade",
        )
        .with_category("cbu_delete"),
        TestScenario::matched(
            "completely remove",
            "completely remove fund",
            "cbu.delete-cascade",
        )
        .with_category("cbu_delete"),
        TestScenario::matched("purge cbu", "purge cbu", "cbu.delete-cascade")
            .with_category("cbu_delete"),
        TestScenario::matched("purge fund", "purge fund", "cbu.delete-cascade")
            .with_category("cbu_delete"),
    ]
}

/// CBU product scenarios
pub fn cbu_product_scenarios() -> Vec<TestScenario> {
    vec![
        TestScenario::matched("add product", "add product to fund", "cbu.add-product")
            .with_category("cbu_product"),
        TestScenario::matched("assign product", "assign product to cbu", "cbu.add-product")
            .with_category("cbu_product"),
        TestScenario::matched(
            "add custody product",
            "add custody product to fund",
            "cbu.add-product",
        )
        .with_category("cbu_product"),
        TestScenario::matched(
            "enable custody",
            "enable custody for fund",
            "cbu.add-product",
        )
        .with_category("cbu_product"),
        TestScenario::matched("add trading", "add trading to fund", "cbu.add-product")
            .with_category("cbu_product"),
        TestScenario::matched(
            "subscribe to product",
            "subscribe fund to product",
            "cbu.add-product",
        )
        .with_category("cbu_product"),
        // Remove
        TestScenario::matched(
            "remove product",
            "remove product from fund",
            "cbu.remove-product",
        )
        .with_category("cbu_product"),
        TestScenario::matched(
            "unsubscribe product",
            "unsubscribe fund from product",
            "cbu.remove-product",
        )
        .with_category("cbu_product"),
        TestScenario::matched(
            "disable custody",
            "disable custody for fund",
            "cbu.remove-product",
        )
        .with_category("cbu_product"),
    ]
}

/// CBU bulk creation scenarios
pub fn cbu_bulk_create_scenarios() -> Vec<TestScenario> {
    vec![
        TestScenario::matched(
            "create from client group",
            "create cbus from client group",
            "cbu.create-from-client-group",
        )
        .with_category("cbu_bulk"),
        TestScenario::matched(
            "bulk create",
            "bulk create cbus for allianz",
            "cbu.create-from-client-group",
        )
        .with_category("cbu_bulk"),
        TestScenario::matched(
            "create from gleif",
            "create cbus from gleif import",
            "cbu.create-from-client-group",
        )
        .with_category("cbu_bulk"),
        TestScenario::matched(
            "onboard from research",
            "onboard entities from research",
            "cbu.create-from-client-group",
        )
        .with_category("cbu_bulk"),
        TestScenario::matched(
            "convert to cbus",
            "convert entities to cbus",
            "cbu.create-from-client-group",
        )
        .with_category("cbu_bulk"),
        TestScenario::matched(
            "mass create funds",
            "mass create funds for client",
            "cbu.create-from-client-group",
        )
        .with_category("cbu_bulk"),
        TestScenario::matched(
            "batch create cbus",
            "batch create cbus",
            "cbu.create-from-client-group",
        )
        .with_category("cbu_bulk"),
    ]
}

/// All CBU scenarios combined
pub fn all_cbu_scenarios() -> Vec<TestScenario> {
    let mut all = Vec::new();
    all.extend(cbu_create_scenarios());
    all.extend(cbu_list_scenarios());
    all.extend(cbu_update_scenarios());
    all.extend(cbu_assign_role_scenarios());
    all.extend(cbu_remove_role_scenarios());
    all.extend(cbu_parties_scenarios());
    all.extend(cbu_delete_scenarios());
    all.extend(cbu_product_scenarios());
    all.extend(cbu_bulk_create_scenarios());
    all
}
