use alloc::vec::Vec;

use cobuild_types::lazy_reader::support::Cursor;

use crate::{error::CoreError, view::WitnessLayoutView};

pub(crate) struct WitnessScan {
    summaries: Vec<WitnessSummary>,
}

#[derive(Clone)]
enum WitnessSummary {
    Empty,
    Other,
    Malformed(CoreError),
    SighashAll { message: Cursor },
    SighashAllOnly,
}

impl WitnessScan {
    pub(crate) fn with_capacity(capacity: usize) -> Self {
        Self {
            summaries: Vec::with_capacity(capacity),
        }
    }

    pub(crate) fn push_witness(&mut self, witness: &[u8]) -> Result<(), CoreError> {
        self.summaries.push(Self::summarize_witness(witness)?);
        Ok(())
    }

    pub(crate) fn tx_level_carrier_has_sighash_all_layout(
        &self,
        index: usize,
    ) -> Result<bool, CoreError> {
        match self.summaries.get(index) {
            Some(WitnessSummary::SighashAll { .. }) | Some(WitnessSummary::SighashAllOnly) => {
                Ok(true)
            }
            Some(WitnessSummary::Malformed(error)) => Err(error.clone()),
            Some(WitnessSummary::Empty | WitnessSummary::Other) | None => Ok(false),
        }
    }

    pub(crate) fn unique_sighash_all_message(&self) -> Result<Option<Cursor>, CoreError> {
        let mut message = None;
        for summary in &self.summaries {
            match summary {
                WitnessSummary::SighashAll { message: candidate } => {
                    if message.is_some() {
                        return Err(CoreError::DuplicateSighashAll);
                    }
                    message = Some(candidate.clone());
                }
                WitnessSummary::Malformed(error) => return Err(error.clone()),
                _ => {}
            }
        }
        Ok(message)
    }

    pub(crate) fn unique_sighash_all_message_with_index(
        &self,
    ) -> Result<Option<(usize, Cursor)>, CoreError> {
        let mut message = None;
        for (index, summary) in self.summaries.iter().enumerate() {
            match summary {
                WitnessSummary::SighashAll { message: candidate } => {
                    if message.is_some() {
                        return Err(CoreError::DuplicateSighashAll);
                    }
                    message = Some((index, candidate.clone()));
                }
                WitnessSummary::Malformed(error) => return Err(error.clone()),
                _ => {}
            }
        }
        Ok(message)
    }

    fn summarize_witness(witness: &[u8]) -> Result<WitnessSummary, CoreError> {
        if witness.is_empty() {
            return Ok(WitnessSummary::Empty);
        }

        let view = match WitnessLayoutView::from_slice(witness) {
            Ok(view) => view,
            Err(error) => {
                return if has_tx_level_witness_id(witness) {
                    Ok(WitnessSummary::Malformed(error))
                } else {
                    Ok(WitnessSummary::Other)
                };
            }
        };
        if let Some(message) = view.sighash_all_message()? {
            return Ok(WitnessSummary::SighashAll { message });
        }
        if view.is_sighash_all_only() {
            return Ok(WitnessSummary::SighashAllOnly);
        }
        Ok(WitnessSummary::Other)
    }
}

fn has_tx_level_witness_id(witness: &[u8]) -> bool {
    if witness.len() < 4 {
        return false;
    }
    let item_id = u32::from_le_bytes([witness[0], witness[1], witness[2], witness[3]]);
    matches!(item_id, 0xff00_0001 | 0xff00_0002)
}

#[cfg(test)]
mod tests {
    use alloc::vec::Vec;

    use super::WitnessScan;
    use crate::error::CoreError;

    #[test]
    fn tx_level_carrier_returns_false_for_empty_or_other_witness() {
        let mut scan = WitnessScan::with_capacity(2);
        scan.push_witness(&[]).unwrap();
        scan.push_witness(&[0, 1, 2, 3]).unwrap();

        assert_eq!(scan.tx_level_carrier_has_sighash_all_layout(0), Ok(false));
        assert_eq!(scan.tx_level_carrier_has_sighash_all_layout(1), Ok(false));
        assert_eq!(scan.tx_level_carrier_has_sighash_all_layout(2), Ok(false));
    }

    #[test]
    fn malformed_relevant_tx_level_witness_is_retained_and_reported() {
        let mut scan = WitnessScan::with_capacity(1);
        scan.push_witness(&0xff00_0001u32.to_le_bytes()).unwrap();

        assert_eq!(
            scan.tx_level_carrier_has_sighash_all_layout(0),
            Err(CoreError::InvalidOtxLayout)
        );
        assert_eq!(
            scan.unique_sighash_all_message()
                .map(|message| message.is_some()),
            Err(CoreError::InvalidOtxLayout)
        );
    }

    #[test]
    fn duplicate_sighash_all_messages_are_rejected() {
        let mut scan = WitnessScan::with_capacity(2);
        let message = empty_message();
        scan.push_witness(&sighash_all_witness_bytes(&[0x11], &message))
            .unwrap();
        scan.push_witness(&sighash_all_witness_bytes(&[0x22], &message))
            .unwrap();

        assert_eq!(
            scan.unique_sighash_all_message()
                .map(|message| message.is_some()),
            Err(CoreError::DuplicateSighashAll)
        );
        assert_eq!(
            scan.unique_sighash_all_message_with_index()
                .map(|message| message.map(|(index, _)| index)),
            Err(CoreError::DuplicateSighashAll)
        );
    }

    #[test]
    fn sighash_all_only_is_carrier_layout_without_unique_message() {
        let mut scan = WitnessScan::with_capacity(1);
        scan.push_witness(&sighash_all_only_witness_bytes(&[0x11, 0x22]))
            .unwrap();

        assert_eq!(scan.tx_level_carrier_has_sighash_all_layout(0), Ok(true));
        assert_eq!(
            scan.unique_sighash_all_message()
                .map(|message| message.is_some()),
            Ok(false)
        );
        assert_eq!(
            scan.unique_sighash_all_message_with_index()
                .map(|message| message.map(|(index, _)| index)),
            Ok(None)
        );
    }

    fn sighash_all_witness_bytes(seal: &[u8], message: &[u8]) -> Vec<u8> {
        let seal_bytes = molecule_bytes(seal);
        let item = table_bytes(&[seal_bytes, message.to_vec()]);
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&0xff00_0001u32.to_le_bytes());
        bytes.extend_from_slice(&item);
        bytes
    }

    fn sighash_all_only_witness_bytes(seal: &[u8]) -> Vec<u8> {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&0xff00_0002u32.to_le_bytes());
        bytes.extend_from_slice(&table_bytes(&[molecule_bytes(seal)]));
        bytes
    }

    fn empty_message() -> Vec<u8> {
        table_bytes(&[4u32.to_le_bytes().to_vec()])
    }

    fn molecule_bytes(raw: &[u8]) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(4 + raw.len());
        bytes.extend_from_slice(&(raw.len() as u32).to_le_bytes());
        bytes.extend_from_slice(raw);
        bytes
    }

    fn table_bytes(fields: &[Vec<u8>]) -> Vec<u8> {
        let header_size = 4 + fields.len() * 4;
        let total_size = header_size + fields.iter().map(Vec::len).sum::<usize>();
        let mut bytes = Vec::with_capacity(total_size);
        bytes.extend_from_slice(&(total_size as u32).to_le_bytes());

        let mut offset = header_size;
        for field in fields {
            bytes.extend_from_slice(&(offset as u32).to_le_bytes());
            offset += field.len();
        }
        for field in fields {
            bytes.extend_from_slice(field);
        }

        bytes
    }
}
