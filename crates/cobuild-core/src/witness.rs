use alloc::vec::Vec;

use cobuild_types::lazy_reader::support::Cursor;

use crate::{
    error::CoreError,
    layout::{has_otx_witness_id, OtxLayoutCollector, OtxLayoutScan},
    view::{CobuildWitnessLayoutView, SighashAllWitnessView},
};

pub(crate) struct CobuildWitnessScanner {
    tx_level: WitnessScan,
    otx_layout: OtxLayoutCollector,
    witness_count: usize,
}

pub(crate) struct ScannedCobuildWitnesses {
    pub(crate) tx_level: WitnessScan,
    pub(crate) otx_layout: OtxLayoutScan,
}

pub(crate) struct WitnessScan {
    sighash_all_summaries: Vec<SighashAllWitnessSummary>,
    has_cobuild_witness_layout: bool,
}

#[derive(Clone)]
enum SighashAllWitnessSummary {
    Empty,
    Legacy,
    OtherCobuildLayout,
    SighashAll { seal: Cursor, message: Cursor },
    SighashAllOnly { seal: Cursor },
}

pub(crate) enum TxLevelCarrierView {
    WithMessage { seal: Cursor, message: Cursor },
    SealOnly { seal: Cursor },
}

impl CobuildWitnessScanner {
    pub(crate) fn with_capacity(capacity: usize) -> Self {
        Self {
            tx_level: WitnessScan::with_capacity(capacity),
            otx_layout: OtxLayoutCollector::new(),
            witness_count: 0,
        }
    }

    pub(crate) fn push_witness(&mut self, witness: &[u8]) -> Result<(), CoreError> {
        let index = self.witness_count;
        self.witness_count += 1;

        if witness.is_empty() {
            self.tx_level
                .record_witness_summary(SighashAllWitnessSummary::Empty);
            return Ok(());
        }

        match CobuildWitnessLayoutView::from_slice(witness) {
            Ok(view) => {
                let summary = WitnessScan::summarize_cobuild_layout(&view)?;
                self.tx_level.record_witness_summary(summary);
                self.otx_layout.record_cobuild_layout(index, &view)
            }
            Err(error) => {
                let summary = WitnessScan::summarize_legacy_or_reject(witness, error)?;
                self.tx_level.record_witness_summary(summary);
                Ok(())
            }
        }
    }

    pub(crate) fn finish(
        self,
        input_count: usize,
        output_count: usize,
        cell_dep_count: usize,
        header_dep_count: usize,
    ) -> Result<ScannedCobuildWitnesses, CoreError> {
        let otx_layout =
            self.otx_layout
                .finish(input_count, output_count, cell_dep_count, header_dep_count)?;
        Ok(ScannedCobuildWitnesses {
            tx_level: self.tx_level,
            otx_layout,
        })
    }
}

impl WitnessScan {
    pub(crate) fn with_capacity(capacity: usize) -> Self {
        Self {
            sighash_all_summaries: Vec::with_capacity(capacity),
            has_cobuild_witness_layout: false,
        }
    }

    fn record_witness_summary(&mut self, summary: SighashAllWitnessSummary) {
        if summary.is_cobuild_witness_layout() {
            self.has_cobuild_witness_layout = true;
        }
        self.sighash_all_summaries.push(summary);
    }

    pub(crate) fn has_cobuild_witness_layout(&self) -> bool {
        self.has_cobuild_witness_layout
    }

    pub(crate) fn tx_level_carrier_has_sighash_all_layout(
        &self,
        index: usize,
    ) -> Result<bool, CoreError> {
        match self.sighash_all_summaries.get(index) {
            Some(SighashAllWitnessSummary::SighashAll { .. })
            | Some(SighashAllWitnessSummary::SighashAllOnly { .. }) => Ok(true),
            Some(
                SighashAllWitnessSummary::Empty
                | SighashAllWitnessSummary::Legacy
                | SighashAllWitnessSummary::OtherCobuildLayout,
            )
            | None => Ok(false),
        }
    }

    pub(crate) fn unique_sighash_all_message(&self) -> Result<Option<Cursor>, CoreError> {
        let mut message = None;
        for summary in &self.sighash_all_summaries {
            match summary {
                SighashAllWitnessSummary::SighashAll {
                    message: candidate, ..
                } => {
                    if message.is_some() {
                        return Err(CoreError::DuplicateSighashAll);
                    }
                    message = Some(candidate.clone());
                }
                _ => {}
            }
        }
        Ok(message)
    }

    pub(crate) fn unique_sighash_all_message_with_index(
        &self,
    ) -> Result<Option<(usize, Cursor)>, CoreError> {
        let mut message = None;
        for (index, summary) in self.sighash_all_summaries.iter().enumerate() {
            match summary {
                SighashAllWitnessSummary::SighashAll {
                    message: candidate, ..
                } => {
                    if message.is_some() {
                        return Err(CoreError::DuplicateSighashAll);
                    }
                    message = Some((index, candidate.clone()));
                }
                _ => {}
            }
        }
        Ok(message)
    }

    pub(crate) fn tx_level_carrier_view(
        &self,
        index: usize,
    ) -> Result<Option<TxLevelCarrierView>, CoreError> {
        match self.sighash_all_summaries.get(index) {
            Some(SighashAllWitnessSummary::SighashAll { seal, message }) => {
                Ok(Some(TxLevelCarrierView::WithMessage {
                    seal: seal.clone(),
                    message: message.clone(),
                }))
            }
            Some(SighashAllWitnessSummary::SighashAllOnly { seal }) => {
                Ok(Some(TxLevelCarrierView::SealOnly { seal: seal.clone() }))
            }
            Some(
                SighashAllWitnessSummary::Empty
                | SighashAllWitnessSummary::Legacy
                | SighashAllWitnessSummary::OtherCobuildLayout,
            )
            | None => Ok(None),
        }
    }

    pub(crate) fn ensure_non_carrier_witnesses_empty<I>(
        &self,
        indices: I,
        carrier_index: usize,
    ) -> Result<(), CoreError>
    where
        I: IntoIterator<Item = usize>,
    {
        for index in indices {
            if index == carrier_index {
                continue;
            }
            match self.sighash_all_summaries.get(index) {
                Some(SighashAllWitnessSummary::Empty) | None => {}
                Some(_) => return Err(CoreError::InvalidLockGroupWitness),
            }
        }
        Ok(())
    }

    fn summarize_legacy_or_reject(
        witness: &[u8],
        error: CoreError,
    ) -> Result<SighashAllWitnessSummary, CoreError> {
        if has_cobuild_witness_id(witness) {
            Err(error)
        } else {
            Ok(SighashAllWitnessSummary::Legacy)
        }
    }

    fn summarize_cobuild_layout(
        view: &CobuildWitnessLayoutView,
    ) -> Result<SighashAllWitnessSummary, CoreError> {
        if let Some(carrier) = view.sighash_all_cobuild_witness_layout()? {
            return match carrier {
                SighashAllWitnessView::WithMessage { seal, message } => {
                    Ok(SighashAllWitnessSummary::SighashAll { seal, message })
                }
                SighashAllWitnessView::SealOnly { seal } => {
                    Ok(SighashAllWitnessSummary::SighashAllOnly { seal })
                }
            };
        }
        Ok(SighashAllWitnessSummary::OtherCobuildLayout)
    }
}

impl SighashAllWitnessSummary {
    fn is_cobuild_witness_layout(&self) -> bool {
        !matches!(self, Self::Empty | Self::Legacy)
    }
}

fn has_tx_level_witness_id(witness: &[u8]) -> bool {
    if witness.len() < 4 {
        return false;
    }
    let item_id = u32::from_le_bytes([witness[0], witness[1], witness[2], witness[3]]);
    matches!(item_id, 0xff00_0001 | 0xff00_0002)
}

fn has_cobuild_witness_id(witness: &[u8]) -> bool {
    has_tx_level_witness_id(witness) || has_otx_witness_id(witness)
}

#[cfg(test)]
mod tests {
    use alloc::vec::Vec;

    use super::{CobuildWitnessScanner, WitnessScan};
    use crate::error::CoreError;

    #[test]
    fn tx_level_carrier_returns_false_for_empty_or_other_witness() {
        let empty = Vec::new();
        let legacy = [0, 1, 2, 3];
        let scan = scan_witnesses([empty.as_slice(), legacy.as_slice()]).unwrap();

        assert_eq!(scan.tx_level_carrier_has_sighash_all_layout(0), Ok(false));
        assert_eq!(scan.tx_level_carrier_has_sighash_all_layout(1), Ok(false));
        assert_eq!(scan.tx_level_carrier_has_sighash_all_layout(2), Ok(false));
    }

    #[test]
    fn malformed_reserved_cobuild_witness_id_fails_scanning() {
        for item_id in [0xff00_0001u32, 0xff00_0002, 0xff00_0003, 0xff00_0004] {
            let malformed = item_id.to_le_bytes();

            assert_eq!(
                scan_witnesses([malformed.as_slice()]).err(),
                Some(CoreError::InvalidOtxLayout)
            );
        }
    }

    #[test]
    fn duplicate_sighash_all_messages_are_rejected() {
        let message = empty_message();
        let first = sighash_all_witness_bytes(&[0x11], &message);
        let second = sighash_all_witness_bytes(&[0x22], &message);
        let scan = scan_witnesses([first.as_slice(), second.as_slice()]).unwrap();

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
        let witness = sighash_all_only_witness_bytes(&[0x11, 0x22]);
        let scan = scan_witnesses([witness.as_slice()]).unwrap();

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

    fn scan_witnesses<const N: usize>(witnesses: [&[u8]; N]) -> Result<WitnessScan, CoreError> {
        let mut scanner = CobuildWitnessScanner::with_capacity(N);
        for witness in witnesses {
            scanner.push_witness(witness)?;
        }
        scanner.finish(0, 0, 0, 0).map(|scanned| scanned.tx_level)
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
