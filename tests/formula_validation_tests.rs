use pattern_gif_studio::render::{
    formula::{CompiledFormula, FormulaError, FormulaLayer, FormulaSource},
    renderer::{EffectLayer, RenderParams},
};

#[test]
fn invalid_syntax_is_reported() {
    assert_eq!(
        CompiledFormula::compile("sin((x)").unwrap_err(),
        FormulaError::MissingClosingParen
    );
}

#[test]
fn unknown_function_is_reported() {
    assert_eq!(
        CompiledFormula::compile("wobble(x)").unwrap_err(),
        FormulaError::UnknownFunction("wobble".to_owned())
    );
}

#[test]
fn wrong_function_arity_is_reported() {
    assert_eq!(
        CompiledFormula::compile("sin(x, y)").unwrap_err(),
        FormulaError::WrongArity("sin".to_owned())
    );
}

#[test]
fn unknown_variable_is_reported() {
    assert_eq!(
        CompiledFormula::compile("sin(speed)").unwrap_err(),
        FormulaError::UnknownVariable("speed".to_owned())
    );
}

#[test]
fn render_params_report_active_custom_formula_errors() {
    let mut params = RenderParams::default();
    params.effects.push(EffectLayer::new(
        "Bad effect",
        FormulaSource {
            layers: vec![FormulaLayer {
                expression: "bad_fn(x)".to_owned(),
                ..FormulaLayer::default()
            }],
            ..FormulaSource::default()
        },
    ));

    let issues = params.formula_issues();

    assert_eq!(issues.len(), 1);
    assert!(issues[0].message.contains("Unknown function"));
}

#[test]
fn render_params_report_domain_formula_errors_next_to_domain_label() {
    let mut params = RenderParams::default();
    params.patterns[0].source.layers[0].domain_x = "bad_domain(x)".to_owned();

    let issues = params.formula_issues();

    assert_eq!(issues.len(), 1);
    assert!(issues[0].label.contains("domain X"));
    assert!(issues[0].message.contains("Unknown function"));
}
