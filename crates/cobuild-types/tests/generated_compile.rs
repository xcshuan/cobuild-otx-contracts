use cobuild_types::{entity, lazy_reader};

#[test]
fn exposes_lazy_reader_and_entity_modules() {
    let _ = core::any::type_name::<lazy_reader::witness::WitnessLayout>();
    let _ = core::any::type_name::<entity::witness::WitnessLayout>();
    let _ = core::any::type_name::<lazy_reader::core::Otx>();
    let _ = core::any::type_name::<entity::core::Otx>();
}
