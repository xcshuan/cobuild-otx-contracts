#[path = "../src/generated/mod.rs"]
mod generated;

#[test]
fn generated_module_tree_compiles() {
    let _ = generated::core::Action::default();
    let _ = generated::witness::WitnessLayout::default();
}
