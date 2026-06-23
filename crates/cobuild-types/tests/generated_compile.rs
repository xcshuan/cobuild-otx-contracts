use cobuild_types::{entity, lazy_reader};

#[test]
fn exposes_lazy_reader_and_entity_modules() {
    let _ = core::any::type_name::<lazy_reader::witness::WitnessLayout>();
    let _ = core::any::type_name::<entity::witness::WitnessLayout>();
    let _ = core::any::type_name::<lazy_reader::core::Otx>();
    let _ = core::any::type_name::<entity::core::Otx>();
}

#[test]
fn generated_segmented_append_types_compile() {
    let _ = core::any::type_name::<lazy_reader::core::OtxAppendSegment>();
    let _ = core::any::type_name::<lazy_reader::core::OtxAppendSegmentVec>();
    let _ = core::any::type_name::<lazy_reader::core::LockSeal>();
    let _ = core::any::type_name::<lazy_reader::core::LockSealVec>();
    let _ = core::any::type_name::<entity::core::OtxAppendSegment>();
    let _ = core::any::type_name::<entity::core::OtxAppendSegmentVec>();
    let _ = core::any::type_name::<entity::core::LockSeal>();
    let _ = core::any::type_name::<entity::core::LockSealVec>();
}
