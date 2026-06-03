use cobuild_core::reader::{cursor_bytes, OwnedReader};
use cobuild_core::view::{SighashAllWitnessView, WitnessLayoutView};
use cobuild_types::lazy_reader::support::{Error as MoleculeError, Read};

#[test]
fn empty_witness_is_not_a_cobuild_layout() {
    assert!(WitnessLayoutView::from_slice(&[]).is_err());
}

#[test]
fn owned_reader_reports_out_of_bound_offsets() {
    let reader = OwnedReader::new(&[]);
    let mut buf = [0u8; 1];
    assert!(matches!(
        reader.read(&mut buf, 1),
        Err(MoleculeError::OutOfBound(1, 0))
    ));
}

#[test]
fn parsed_view_survives_source_slice_drop() {
    let view = {
        let witness = sighash_all_only_witness_bytes(&[0x11, 0x22, 0x33]);
        WitnessLayoutView::from_slice(&witness).unwrap()
    };

    assert_eq!(
        view.sighash_all_only_seal().unwrap(),
        Some(vec![0x11, 0x22, 0x33])
    );
}

#[test]
fn parsed_sighash_all_view_carries_cursor_backed_seal_and_message() {
    let seal = [0x11, 0x22, 0x33];
    let message = empty_message();
    let witness = sighash_all_witness_bytes(&seal, &message);
    let view = WitnessLayoutView::from_slice(&witness).unwrap();

    let layout = view.sighash_all_witness_layout().unwrap().unwrap();
    match layout {
        SighashAllWitnessView::WithMessage {
            seal: seal_cursor,
            message: message_cursor,
        } => {
            assert_eq!(cursor_bytes(&seal_cursor).unwrap(), seal.to_vec());
            assert_eq!(cursor_bytes(&message_cursor).unwrap(), message);
        }
        SighashAllWitnessView::SealOnly { .. } => panic!("expected sighash-all message view"),
    }
}

fn sighash_all_witness_bytes(seal: &[u8], message: &[u8]) -> Vec<u8> {
    let seal_bytes = molecule_bytes(seal);
    let item = table_bytes(&[seal_bytes, message.to_vec()]);
    let mut bytes = Vec::new();
    bytes.extend_from_slice(&4_278_190_081u32.to_le_bytes());
    bytes.extend_from_slice(&item);
    bytes
}

fn sighash_all_only_witness_bytes(seal: &[u8]) -> Vec<u8> {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(&4_278_190_082u32.to_le_bytes());
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
