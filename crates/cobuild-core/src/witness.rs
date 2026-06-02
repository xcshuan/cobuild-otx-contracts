use crate::{error::CoreError, view::WitnessLayoutView};

pub enum ParsedWitness {
    None,
    Cobuild(WitnessLayoutView),
}

pub fn parse_witness(data: &[u8]) -> Result<ParsedWitness, CoreError> {
    match WitnessLayoutView::from_slice(data) {
        Ok(view) => Ok(ParsedWitness::Cobuild(view)),
        Err(CoreError::MalformedCobuild | CoreError::InvalidOtxLayout) => Ok(ParsedWitness::None),
        Err(err) => Err(err),
    }
}
