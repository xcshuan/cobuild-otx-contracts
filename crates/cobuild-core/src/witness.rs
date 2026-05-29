use crate::{error::CoreError, view::WitnessLayoutView};

pub enum ParsedWitness<'a> {
    None,
    Cobuild(WitnessLayoutView<'a>),
}

pub fn parse_witness(data: &[u8]) -> Result<ParsedWitness<'_>, CoreError> {
    match WitnessLayoutView::from_slice(data) {
        Ok(view) => Ok(ParsedWitness::Cobuild(view)),
        Err(CoreError::MalformedCobuild | CoreError::InvalidLayout) => Ok(ParsedWitness::None),
        Err(err) => Err(err),
    }
}
