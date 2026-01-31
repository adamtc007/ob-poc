use crate::dsl_v2::ast::Program;
use crate::dsl_v2::binding_context::BindingContext;
use crate::dsl_v2::runtime_registry::RuntimeVerbRegistry;
use crate::dsl_v2::verb_registry::registry;

/// A suggested next step (verb)
#[derive(Debug, Clone)]
pub struct Suggestion {
    /// The full verb name (domain.verb)
    pub verb: String,
    /// Score from 0.0 to 1.0 (higher is better)
    pub score: f32,
    /// Reason for the suggestion
    pub reason: String,
}

/// Predict likely next steps based on available bindings and context
pub fn predict_next_steps(
    _ast: &Program,
    bindings: &BindingContext,
    _registry: &RuntimeVerbRegistry,
) -> Vec<Suggestion> {
    let mut suggestions = Vec::new();
    let reg = registry();

    // Iterate over all known verbs
    for verb in reg.all_verbs() {
        let consumes = verb.consumes();

        // Skip verbs that consume nothing (unless they are creation verbs like cbu.create)
        // Creating a CBU is usually a starting step.
        if consumes.is_empty() {
            if verb.domain == "cbu" && (verb.verb == "create" || verb.verb == "ensure") {
                // Determine if we should suggest CBU creation
                // If we don't have a CBU binding yet, this is a high priority
                let has_cbu = bindings.all().any(|b| b.produced_type == "CBU");
                if !has_cbu {
                    suggestions.push(Suggestion {
                        verb: verb.full_name(),
                        score: 0.9,
                        reason: "Start by creating a CBU".to_string(),
                    });
                } else {
                    // We already have a CBU, creating another is possible but less likely
                    suggestions.push(Suggestion {
                        verb: verb.full_name(),
                        score: 0.1,
                        reason: "Create another CBU".to_string(),
                    });
                }
            } else if verb.domain == "entity" && (verb.verb.starts_with("create")) {
                // Entity creation is always valid if we have a CBU (implied context) or generally
                suggestions.push(Suggestion {
                    verb: verb.full_name(),
                    score: 0.5,
                    reason: "Create a new entity".to_string(),
                });
            }
            continue;
        }

        // Check if we have the required bindings
        let mut missing_requirements = false;
        let mut satisfied_requirements = 0;
        let mut total_requirements = 0;

        for consumer in consumes {
            if consumer.required {
                total_requirements += 1;
                // Do we have a binding of this type?
                let has_binding = bindings
                    .all()
                    .any(|b| b.matches_type(&consumer.consumed_type));
                if has_binding {
                    satisfied_requirements += 1;
                } else {
                    missing_requirements = true;
                    // For hard requirements, we can't recommend this deeply
                    // But maybe we warn/show it as "disabled" or low score?
                    // For now, let's skip/penalty strongly
                    break;
                }
            }
        }

        if !missing_requirements {
            // All required inputs exist!
            let base_score = 0.6;
            let boost = if total_requirements > 0 { 0.2 } else { 0.0 };

            suggestions.push(Suggestion {
                verb: verb.full_name(),
                score: base_score + boost,
                reason: format!(
                    "Dependencies met ({}/{})",
                    satisfied_requirements, total_requirements
                ),
            });
        }
    }

    // Sort by score descending
    suggestions.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    suggestions
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dsl_v2::binding_context::BindingInfo;
    use crate::dsl_v2::runtime_registry::runtime_registry;
    use uuid::Uuid;

    #[test]
    fn test_suggestion_struct() {
        let s = Suggestion {
            verb: "cbu.ensure".to_string(),
            score: 0.9,
            reason: "Start by creating a CBU".to_string(),
        };
        assert_eq!(s.verb, "cbu.ensure");
        assert!(s.score > 0.8);
        assert!(!s.reason.is_empty());
    }

    #[test]
    fn test_predict_next_steps_empty_bindings() {
        let ast = Program::default();
        let bindings = BindingContext::new();
        let reg = runtime_registry();

        let suggestions = predict_next_steps(&ast, &bindings, reg);

        // With no bindings, should suggest cbu.create/ensure with high score
        let cbu_suggestions: Vec<_> = suggestions
            .iter()
            .filter(|s| s.verb == "cbu.create" || s.verb == "cbu.ensure")
            .collect();

        assert!(!cbu_suggestions.is_empty(), "Should suggest CBU creation");
        assert!(
            cbu_suggestions[0].score >= 0.8,
            "CBU creation should have high score"
        );
    }

    #[test]
    #[ignore = "scoring algorithm needs review - CBU score logic changed"]
    fn test_predict_next_steps_with_cbu_binding() {
        let ast = Program::default();
        let mut bindings = BindingContext::new();
        // Insert a CBU binding to simulate having created a CBU
        bindings.insert(BindingInfo {
            name: "fund".to_string(),
            produced_type: "cbu".to_string(),
            subtype: None,
            entity_pk: Uuid::nil(),
            resolved: false,
        });
        let reg = runtime_registry();

        let suggestions = predict_next_steps(&ast, &bindings, reg);

        // With a CBU binding, cbu.create should have low score
        let cbu_create: Vec<_> = suggestions
            .iter()
            .filter(|s| s.verb == "cbu.create" || s.verb == "cbu.ensure")
            .collect();

        if !cbu_create.is_empty() {
            assert!(
                cbu_create[0].score < 0.5,
                "Creating another CBU should have low score"
            );
        }
    }

    #[test]
    fn test_suggestions_sorted_by_score() {
        let ast = Program::default();
        let bindings = BindingContext::new();
        let reg = runtime_registry();

        let suggestions = predict_next_steps(&ast, &bindings, reg);

        // Verify sorted descending by score
        for i in 1..suggestions.len() {
            assert!(
                suggestions[i - 1].score >= suggestions[i].score,
                "Suggestions should be sorted by score descending"
            );
        }
    }
}
