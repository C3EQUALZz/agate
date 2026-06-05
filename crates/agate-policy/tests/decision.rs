//! Scenario tests over the policy context's public API: build a ruleset, decide
//! an action, assert the verdict. Pure and deterministic — no I/O.

use std::collections::BTreeSet;

use agate_policy::application::PolicyService;
use agate_policy::domain::decision::services::REDACTION_MASK;
use agate_policy::domain::decision::{
    DenyReason, InspectedAction, PolicyDecision, PolicyRuleset, SecretPattern, ToolName, ToolPolicy,
};

fn tool_names(names: &[&str]) -> BTreeSet<ToolName> {
    names
        .iter()
        .map(|name| ToolName::new(*name).expect("valid tool name"))
        .collect()
}

fn tool_call(name: &str) -> InspectedAction {
    InspectedAction::ToolCall {
        name: name.to_owned(),
        arguments: "{}".to_owned(),
    }
}

#[test]
fn allow_all_permits_every_tool() {
    let service = PolicyService::new(PolicyRuleset::allow_all());

    assert_eq!(service.decide(&tool_call("rm_rf")), PolicyDecision::Allow);
}

#[test]
fn allowlist_denies_a_tool_not_on_it() {
    let ruleset = PolicyRuleset::new(ToolPolicy::Allowlist(tool_names(&["search"])), Vec::new());
    let service = PolicyService::new(ruleset);

    assert_eq!(
        service.decide(&tool_call("rm_rf")),
        PolicyDecision::Deny(DenyReason::new("tool 'rm_rf' is not permitted"))
    );
}

#[test]
fn allowlist_permits_a_listed_tool() {
    let ruleset = PolicyRuleset::new(ToolPolicy::Allowlist(tool_names(&["search"])), Vec::new());
    let service = PolicyService::new(ruleset);

    assert_eq!(service.decide(&tool_call("search")), PolicyDecision::Allow);
}

#[test]
fn denylist_blocks_only_listed_tools() {
    let ruleset = PolicyRuleset::new(ToolPolicy::Denylist(tool_names(&["rm_rf"])), Vec::new());
    let service = PolicyService::new(ruleset);

    assert!(matches!(
        service.decide(&tool_call("rm_rf")),
        PolicyDecision::Deny(_)
    ));
    assert_eq!(service.decide(&tool_call("search")), PolicyDecision::Allow);
}

#[test]
fn message_with_a_secret_is_redacted() {
    let pattern = SecretPattern::new("sk-SECRET").expect("valid pattern");
    let service = PolicyService::new(PolicyRuleset::new(ToolPolicy::AllowAll, vec![pattern]));
    let action = InspectedAction::Message {
        text: "my key is sk-secret, keep it safe".to_owned(),
    };

    assert_eq!(
        service.decide(&action),
        PolicyDecision::RedactText(format!("my key is {REDACTION_MASK}, keep it safe"))
    );
}

#[test]
fn clean_message_is_allowed() {
    let pattern = SecretPattern::new("sk-SECRET").expect("valid pattern");
    let service = PolicyService::new(PolicyRuleset::new(ToolPolicy::AllowAll, vec![pattern]));
    let action = InspectedAction::Message {
        text: "nothing sensitive here".to_owned(),
    };

    assert_eq!(service.decide(&action), PolicyDecision::Allow);
}

#[test]
fn ungoverned_actions_are_allowed() {
    let service = PolicyService::new(PolicyRuleset::new(
        ToolPolicy::Allowlist(tool_names(&["search"])),
        vec![SecretPattern::new("secret").expect("valid pattern")],
    ));

    assert_eq!(
        service.decide(&InspectedAction::Other),
        PolicyDecision::Allow
    );
}

#[test]
fn blank_tool_name_is_rejected() {
    assert!(ToolName::new("   ").is_err());
}

#[test]
fn padded_allowlist_entry_still_matches_the_tool() {
    let ruleset = PolicyRuleset::new(
        ToolPolicy::Allowlist(tool_names(&["  search  "])),
        Vec::new(),
    );
    let service = PolicyService::new(ruleset);

    assert_eq!(service.decide(&tool_call("search")), PolicyDecision::Allow);
}
