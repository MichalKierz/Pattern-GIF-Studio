pub mod colors_panel;
pub mod controls;
pub mod create_tab;
pub mod effects_panel;
pub mod formula_source_panel;
pub mod gif_output_panel;
pub mod parameter_panel;
pub mod patterns_panel;
pub mod preview_panel;
pub mod shape_panel;
pub mod style;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FormulaSourceTarget {
    Pattern(usize),
    Effect(usize),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UiAction {
    SaveWorkflow,
    LoadWorkflow,
    SaveFormulaSource(FormulaSourceTarget),
    LoadFormulaSource(FormulaSourceTarget),
    AddPattern,
    RemovePattern(usize),
    AddEffect,
    RemoveEffect(usize),
    SaveCustomColorSet,
    LoadCustomColorSet,
    SaveGif,
    CancelExport,
}
