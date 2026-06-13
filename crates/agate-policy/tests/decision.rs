//! Scenario tests over the policy context's public API: build a ruleset, decide
//! an action, assert the verdict. Pure and deterministic — no I/O.

use agate_policy::application::PolicyService;
use agate_policy::domain::decision::services::REDACTION_MASK;
use agate_policy::domain::decision::{
    ArgumentRule, DenyReason, InspectedAction, Pattern, PolicyDecision, PolicyRuleset, ToolMatcher,
    ToolName, ToolPolicy,
};

fn tool_names(names: &[&str]) -> Vec<ToolMatcher> {
    names
        .iter()
        .map(|name| ToolMatcher::exact(*name).expect("valid tool name"))
        .collect()
}

fn tool_call(name: &str) -> InspectedAction {
    InspectedAction::ToolCall {
        name: name.to_owned(),
        arguments: "{}".to_owned(),
    }
}

fn tool_call_with(name: &str, arguments: &str) -> InspectedAction {
    InspectedAction::ToolCall {
        name: name.to_owned(),
        arguments: arguments.to_owned(),
    }
}

#[test]
fn allow_all_permits_every_tool() {
    let service = PolicyService::new(PolicyRuleset::allow_all());

    assert_eq!(service.decide(&tool_call("rm_rf")), PolicyDecision::Allow);
}

#[test]
fn allowlist_denies_a_tool_not_on_it() {
    let ruleset = PolicyRuleset::new(
        ToolPolicy::Allowlist(tool_names(&["search"])),
        Vec::new(),
        Vec::new(),
    );
    let service = PolicyService::new(ruleset);

    assert_eq!(
        service.decide(&tool_call("rm_rf")),
        PolicyDecision::Deny(DenyReason::new("tool 'rm_rf' is not permitted"))
    );
}

#[test]
fn allowlist_permits_a_listed_tool() {
    let ruleset = PolicyRuleset::new(
        ToolPolicy::Allowlist(tool_names(&["search"])),
        Vec::new(),
        Vec::new(),
    );
    let service = PolicyService::new(ruleset);

    assert_eq!(service.decide(&tool_call("search")), PolicyDecision::Allow);
}

#[test]
fn denylist_blocks_only_listed_tools() {
    let ruleset = PolicyRuleset::new(
        ToolPolicy::Denylist(tool_names(&["rm_rf"])),
        Vec::new(),
        Vec::new(),
    );
    let service = PolicyService::new(ruleset);

    assert!(matches!(
        service.decide(&tool_call("rm_rf")),
        PolicyDecision::Deny(_)
    ));
    assert_eq!(service.decide(&tool_call("search")), PolicyDecision::Allow);
}

#[test]
fn a_regex_pattern_redacts_matching_secrets_in_a_message() {
    let pattern = Pattern::regex(r"sk-[a-z0-9]{4}").expect("valid pattern");
    let service = PolicyService::new(PolicyRuleset::new(
        ToolPolicy::AllowAll,
        Vec::new(),
        vec![pattern],
    ));
    let action = InspectedAction::Message {
        text: "tokens sk-ab12 and sk-cd34".to_owned(),
    };

    assert_eq!(
        service.decide(&action),
        PolicyDecision::RedactText(format!("tokens {REDACTION_MASK} and {REDACTION_MASK}"))
    );
}

#[test]
fn message_with_a_secret_is_redacted() {
    let pattern = Pattern::literal("sk-SECRET").expect("valid pattern");
    let service = PolicyService::new(PolicyRuleset::new(
        ToolPolicy::AllowAll,
        Vec::new(),
        vec![pattern],
    ));
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
    let pattern = Pattern::literal("sk-SECRET").expect("valid pattern");
    let service = PolicyService::new(PolicyRuleset::new(
        ToolPolicy::AllowAll,
        Vec::new(),
        vec![pattern],
    ));
    let action = InspectedAction::Message {
        text: "nothing sensitive here".to_owned(),
    };

    assert_eq!(service.decide(&action), PolicyDecision::Allow);
}

#[test]
fn ungoverned_actions_are_allowed() {
    let service = PolicyService::new(PolicyRuleset::new(
        ToolPolicy::Allowlist(tool_names(&["search"])),
        Vec::new(),
        vec![Pattern::literal("secret").expect("valid pattern")],
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
        Vec::new(),
    );
    let service = PolicyService::new(ruleset);

    assert_eq!(service.decide(&tool_call("search")), PolicyDecision::Allow);
}

#[test]
fn a_permitted_tool_is_denied_when_its_arguments_match_a_rule() {
    let ruleset = PolicyRuleset::new(
        ToolPolicy::AllowAll,
        vec![ArgumentRule::new(
            None,
            Pattern::literal("rm -rf").expect("valid"),
        )],
        Vec::new(),
    );
    let service = PolicyService::new(ruleset);

    // The tool name is allowed, but the dangerous argument is blocked.
    assert!(matches!(
        service.decide(&tool_call_with("shell", r#"{"cmd":"rm -rf /"}"#)),
        PolicyDecision::Deny(_)
    ));
    // A benign call to the same tool is allowed.
    assert_eq!(
        service.decide(&tool_call_with("shell", r#"{"cmd":"ls"}"#)),
        PolicyDecision::Allow
    );
}

#[test]
fn a_tool_scoped_argument_rule_ignores_other_tools() {
    let rule = ArgumentRule::new(
        Some(ToolName::new("shell").expect("valid")),
        Pattern::literal("curl").expect("valid"),
    );
    let service = PolicyService::new(PolicyRuleset::new(
        ToolPolicy::AllowAll,
        vec![rule],
        Vec::new(),
    ));

    assert!(matches!(
        service.decide(&tool_call_with("shell", r#"{"cmd":"curl x"}"#)),
        PolicyDecision::Deny(_)
    ));
    // Same marker, different tool → not governed by this rule.
    assert_eq!(
        service.decide(&tool_call_with("search", r#"{"q":"curl"}"#)),
        PolicyDecision::Allow
    );
}

#[test]
fn a_denied_tool_name_short_circuits_before_argument_rules() {
    let ruleset = PolicyRuleset::new(
        ToolPolicy::Allowlist(tool_names(&["search"])),
        vec![ArgumentRule::new(
            None,
            Pattern::literal("anything").expect("valid"),
        )],
        Vec::new(),
    );
    let service = PolicyService::new(ruleset);

    // `rm_rf` is not on the allowlist: the name check denies it first, with the
    // name reason — the argument rules never run.
    assert_eq!(
        service.decide(&tool_call_with("rm_rf", "anything goes")),
        PolicyDecision::Deny(DenyReason::new("tool 'rm_rf' is not permitted"))
    );
}

#[test]
fn a_secret_in_a_tool_result_is_redacted() {
    let service = PolicyService::new(PolicyRuleset::new(
        ToolPolicy::AllowAll,
        Vec::new(),
        vec![Pattern::literal("sk-LEAK").expect("valid pattern")],
    ));
    let action = InspectedAction::ToolResult {
        content: "the key is sk-leak, hide it".to_owned(),
    };

    assert_eq!(
        service.decide(&action),
        PolicyDecision::RedactText(format!("the key is {REDACTION_MASK}, hide it"))
    );
}

#[test]
fn a_secret_in_a_state_mutation_is_denied() {
    let service = PolicyService::new(PolicyRuleset::new(
        ToolPolicy::AllowAll,
        Vec::new(),
        vec![Pattern::literal("sk-LEAK").expect("valid pattern")],
    ));

    // State can't be masked in place, so a secret in it is denied, not leaked.
    assert!(matches!(
        service.decide(&InspectedAction::StateMutation {
            content: r#"{"token":"sk-LEAK"}"#.to_owned(),
        }),
        PolicyDecision::Deny(_)
    ));
    // Clean state passes.
    assert_eq!(
        service.decide(&InspectedAction::StateMutation {
            content: r#"{"count":1}"#.to_owned(),
        }),
        PolicyDecision::Allow
    );
}
