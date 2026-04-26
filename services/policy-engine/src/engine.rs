use crate::model::{
    DecisionEffect, DecisionReason, PolicyDecision, PolicyDocument, PolicyRequest, PolicyRule,
    RuleEffect,
};

pub fn evaluate_policy(policy: &PolicyDocument, request: &PolicyRequest) -> PolicyDecision {
    let mut first_allow: Option<&PolicyRule> = None;
    let mut first_deny: Option<&PolicyRule> = None;

    for rule in &policy.rules {
        if !matches_request(rule, request) {
            continue;
        }

        match rule.effect {
            RuleEffect::Allow => {
                if first_allow.is_none() {
                    first_allow = Some(rule);
                }
            }
            RuleEffect::Deny => {
                if first_deny.is_none() {
                    first_deny = Some(rule);
                }
            }
        }
    }

    if let Some(rule) = first_deny {
        return PolicyDecision {
            effect: DecisionEffect::Deny,
            reason: DecisionReason::MatchedDeny {
                rule_id: rule.id.clone(),
            },
        };
    }

    if let Some(rule) = first_allow {
        return PolicyDecision {
            effect: DecisionEffect::Allow,
            reason: DecisionReason::MatchedAllow {
                rule_id: rule.id.clone(),
            },
        };
    }

    PolicyDecision {
        effect: DecisionEffect::Deny,
        reason: DecisionReason::NoMatch,
    }
}

fn matches_request(rule: &PolicyRule, request: &PolicyRequest) -> bool {
    matches_pattern(&rule.subject, &request.subject)
        && any_pattern_matches(&rule.actions, &request.action)
        && any_pattern_matches(&rule.resources, &request.resource)
}

fn any_pattern_matches(patterns: &[String], value: &str) -> bool {
    patterns
        .iter()
        .any(|pattern| matches_pattern(pattern, value))
}

fn matches_pattern(pattern: &str, value: &str) -> bool {
    if pattern == "*" {
        return true;
    }

    if let Some(prefix) = pattern.strip_suffix('*') {
        return value.starts_with(prefix);
    }

    pattern == value
}

#[cfg(test)]
mod tests {
    use crate::engine::evaluate_policy;
    use crate::model::{
        DecisionEffect, DecisionReason, PolicyDocument, PolicyRequest, PolicyRule, RuleEffect,
    };

    fn test_policy(rules: Vec<PolicyRule>) -> PolicyDocument {
        PolicyDocument {
            version: "v1".to_string(),
            rules,
        }
    }

    fn allow_rule(id: &str) -> PolicyRule {
        PolicyRule {
            id: id.to_string(),
            effect: RuleEffect::Allow,
            subject: "runtime/model-runtime".to_string(),
            actions: vec!["network:connect".to_string()],
            resources: vec!["api.openai.com".to_string()],
        }
    }

    fn deny_rule(id: &str) -> PolicyRule {
        PolicyRule {
            id: id.to_string(),
            effect: RuleEffect::Deny,
            subject: "runtime/model-runtime".to_string(),
            actions: vec!["network:connect".to_string()],
            resources: vec!["api.openai.com".to_string()],
        }
    }

    #[test]
    fn allow_when_allow_rule_matches() {
        let policy = test_policy(vec![allow_rule("allow-1")]);
        let request = PolicyRequest {
            subject: "runtime/model-runtime".to_string(),
            action: "network:connect".to_string(),
            resource: "api.openai.com".to_string(),
        };

        let decision = evaluate_policy(&policy, &request);
        assert_eq!(decision.effect, DecisionEffect::Allow);
        assert_eq!(
            decision.reason,
            DecisionReason::MatchedAllow {
                rule_id: "allow-1".to_string()
            }
        );
    }

    #[test]
    fn deny_when_no_rule_matches() {
        let policy = test_policy(vec![allow_rule("allow-1")]);
        let request = PolicyRequest {
            subject: "runtime/mcp-runtime".to_string(),
            action: "fs:write".to_string(),
            resource: "/".to_string(),
        };

        let decision = evaluate_policy(&policy, &request);
        assert_eq!(decision.effect, DecisionEffect::Deny);
        assert_eq!(decision.reason, DecisionReason::NoMatch);
    }

    #[test]
    fn deny_rule_overrides_allow_rule() {
        let policy = test_policy(vec![allow_rule("allow-1"), deny_rule("deny-1")]);
        let request = PolicyRequest {
            subject: "runtime/model-runtime".to_string(),
            action: "network:connect".to_string(),
            resource: "api.openai.com".to_string(),
        };

        let decision = evaluate_policy(&policy, &request);
        assert_eq!(decision.effect, DecisionEffect::Deny);
        assert_eq!(
            decision.reason,
            DecisionReason::MatchedDeny {
                rule_id: "deny-1".to_string()
            }
        );
    }

    #[test]
    fn wildcard_prefix_matches() {
        let policy = test_policy(vec![PolicyRule {
            id: "allow-prefix".to_string(),
            effect: RuleEffect::Allow,
            subject: "runtime/*".to_string(),
            actions: vec!["network:*".to_string()],
            resources: vec!["api.*".to_string()],
        }]);
        let request = PolicyRequest {
            subject: "runtime/model-runtime".to_string(),
            action: "network:connect".to_string(),
            resource: "api.openai.com".to_string(),
        };

        let decision = evaluate_policy(&policy, &request);
        assert_eq!(decision.effect, DecisionEffect::Allow);
    }
}
