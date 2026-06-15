#[cfg(feature = "type-id")]
use ckb_std::type_id::check_type_id;
use ckb_std::{
    ckb_constants::Source,
    ckb_types::bytes::Bytes,
    high_level::{QueryIter, load_cell_data},
};
#[cfg(not(feature = "type-id"))]
use ckb_std::{ckb_types::prelude::*, high_level::load_script};

use crate::error::Error;

const TYPE_ID_LEN: usize = 32;
const MAX_NAME_LEN: usize = 32;
const ATTRIBUTES_LEN: usize = 4;
const CREATED_AT_LEN: usize = 8;
const HEADER_LEN: usize = 1;

pub fn main() -> Result<(), Error> {
    validate_type_id()?;

    let input_payload = single_payload(Source::GroupInput)?;
    let output_payload = single_payload(Source::GroupOutput)?;

    match (input_payload.as_ref(), output_payload.as_ref()) {
        (None, Some(output)) => parse_nft_data(output).map(|_| ()),
        (Some(input), Some(output)) => verify_transfer(input, output),
        (Some(input), None) => parse_nft_data(input).map(|_| ()),
        _ => Err(Error::NftData),
    }
}

#[cfg(feature = "type-id")]
fn validate_type_id() -> Result<(), Error> {
    check_type_id(0, TYPE_ID_LEN).map_err(Error::from)
}

#[cfg(not(feature = "type-id"))]
fn validate_type_id() -> Result<(), Error> {
    let script = load_script()?;
    let type_id: Bytes = script.args().unpack();
    if type_id.len() != TYPE_ID_LEN {
        return Err(Error::InvalidArgs);
    }

    Ok(())
}

fn single_payload(source: Source) -> Result<Option<Bytes>, Error> {
    let mut payloads = QueryIter::new(load_cell_data, source);
    let Some(payload) = payloads.next() else {
        return Ok(None);
    };
    parse_nft_data(&payload)?;

    if payloads.next().is_some() {
        return Err(Error::NftData);
    }

    Ok(Some(payload.into()))
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct NftData<'a> {
    name: &'a [u8],
    attributes: [u8; ATTRIBUTES_LEN],
    created_at: u64,
}

fn parse_nft_data(data: &[u8]) -> Result<NftData<'_>, Error> {
    let Some((&name_len, body)) = data.split_first() else {
        return Err(Error::NftData);
    };
    let name_len = usize::from(name_len);
    if name_len == 0 || name_len > MAX_NAME_LEN {
        return Err(Error::NftData);
    }

    let expected_len = HEADER_LEN + name_len + ATTRIBUTES_LEN + CREATED_AT_LEN;
    if data.len() != expected_len {
        return Err(Error::NftData);
    }

    let (name, rest) = body.split_at(name_len);
    let (attributes, created_at) = rest.split_at(ATTRIBUTES_LEN);

    let mut attributes_bytes = [0u8; ATTRIBUTES_LEN];
    attributes_bytes.copy_from_slice(attributes);

    let mut created_at_bytes = [0u8; CREATED_AT_LEN];
    created_at_bytes.copy_from_slice(created_at);

    Ok(NftData {
        name,
        attributes: attributes_bytes,
        created_at: u64::from_le_bytes(created_at_bytes),
    })
}

fn verify_transfer(input_payload: &[u8], output_payload: &[u8]) -> Result<(), Error> {
    parse_nft_data(input_payload)?;
    parse_nft_data(output_payload)?;

    if input_payload != output_payload {
        return Err(Error::NftData);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use alloc::vec::Vec;

    use super::*;

    fn nft_data(name: &[u8], attributes: [u8; 4], created_at: u64) -> Vec<u8> {
        let mut data = Vec::with_capacity(1 + name.len() + 4 + 8);
        data.push(name.len() as u8);
        data.extend_from_slice(name);
        data.extend_from_slice(&attributes);
        data.extend_from_slice(&created_at.to_le_bytes());
        data
    }

    #[test]
    fn parse_nft_data_reads_name_attributes_and_created_at() {
        let data = nft_data(b"demo-nft", [1, 2, 3, 4], 1_717_171_717);

        let parsed = parse_nft_data(&data).expect("valid nft data");

        assert_eq!(parsed.name, b"demo-nft");
        assert_eq!(parsed.attributes, [1, 2, 3, 4]);
        assert_eq!(parsed.created_at, 1_717_171_717);
    }

    #[test]
    fn parse_nft_data_rejects_empty_or_too_long_name() {
        assert_eq!(
            parse_nft_data(&nft_data(b"", [0, 0, 0, 0], 1)),
            Err(Error::NftData)
        );

        let long_name = [b'a'; MAX_NAME_LEN + 1];
        assert_eq!(
            parse_nft_data(&nft_data(&long_name, [0, 0, 0, 0], 1)),
            Err(Error::NftData)
        );
    }

    #[test]
    fn parse_nft_data_rejects_trailing_or_truncated_bytes() {
        let mut truncated = nft_data(b"demo", [1, 2, 3, 4], 9);
        truncated.pop();
        assert_eq!(parse_nft_data(&truncated), Err(Error::NftData));

        let mut trailing = nft_data(b"demo", [1, 2, 3, 4], 9);
        trailing.push(0xff);
        assert_eq!(parse_nft_data(&trailing), Err(Error::NftData));
    }

    #[test]
    fn transfer_keeps_nft_data_unchanged_without_reminting() {
        let data = nft_data(b"same", [9, 8, 7, 6], 123);

        assert_eq!(verify_transfer(&data, &data), Ok(()));
    }

    #[test]
    fn transfer_rejects_nft_data_mutation_for_same_type_id() {
        let input = nft_data(b"old", [1, 1, 1, 1], 123);
        let output = nft_data(b"new", [1, 1, 1, 1], 123);

        assert_eq!(verify_transfer(&input, &output), Err(Error::NftData));
    }
}
