use cobuild_core::protocol::ScriptRole;
use cobuild_core::reader::{cursor_bytes, OwnedReader};
use cobuild_core::view::{MessageView, SighashAllWitnessView, WitnessLayoutView};
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

#[test]
fn message_view_exposes_backing_cursor() {
    let message = empty_message();
    let view = MessageView::new(cobuild_core::reader::cursor_from_slice(&message));

    assert_eq!(cursor_bytes(view.cursor()).unwrap(), message);
}

#[test]
fn message_view_returns_action_views_with_cursor_backed_data() {
    let script_info_hash = [0x11u8; 32];
    let script_hash = [0x22u8; 32];
    let message = message_with_actions(&[action_bytes(
        script_info_hash,
        0,
        script_hash,
        &[0xaa, 0xbb],
    )]);
    let view = MessageView::new(cobuild_core::reader::cursor_from_slice(&message));

    let actions = view.actions().unwrap();

    assert_eq!(actions.len(), 1);
    assert_eq!(actions[0].index, 0);
    assert_eq!(actions[0].script_info_hash, script_info_hash);
    assert_eq!(actions[0].script_role, ScriptRole::InputLock);
    assert_eq!(actions[0].script_hash, script_hash);
    assert_eq!(cursor_bytes(&actions[0].data).unwrap(), vec![0xaa, 0xbb]);
}

#[test]
fn message_view_filters_actions_by_role_and_script_hash() {
    let lock_hash = [0x33u8; 32];
    let other_hash = [0x44u8; 32];
    let message = message_with_actions(&[
        action_bytes([0x01u8; 32], 0, lock_hash, &[0x10]),
        action_bytes([0x02u8; 32], 1, lock_hash, &[0x20]),
        action_bytes([0x03u8; 32], 0, other_hash, &[0x30]),
        action_bytes([0x04u8; 32], 0, lock_hash, &[0x40]),
    ]);
    let view = MessageView::new(cobuild_core::reader::cursor_from_slice(&message));

    let actions = view.actions_for(ScriptRole::InputLock, lock_hash).unwrap();

    assert_eq!(actions.len(), 2);
    assert_eq!(actions[0].index, 0);
    assert_eq!(cursor_bytes(&actions[0].data).unwrap(), vec![0x10]);
    assert_eq!(actions[1].index, 3);
    assert_eq!(cursor_bytes(&actions[1].data).unwrap(), vec![0x40]);
}

#[test]
fn message_view_returns_empty_actions_for_role_mismatch() {
    let message = message_with_actions(&[action_bytes([0x01u8; 32], 2, [0x55u8; 32], &[0x99])]);
    let view = MessageView::new(cobuild_core::reader::cursor_from_slice(&message));

    let actions = view
        .actions_for(ScriptRole::InputLock, [0x55u8; 32])
        .unwrap();

    assert!(actions.is_empty());
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

fn message_with_actions(actions: &[Vec<u8>]) -> Vec<u8> {
    table_bytes(&[dynvec_bytes(actions)])
}

fn action_bytes(
    script_info_hash: [u8; 32],
    script_role: u8,
    script_hash: [u8; 32],
    data: &[u8],
) -> Vec<u8> {
    table_bytes(&[
        script_info_hash.to_vec(),
        vec![script_role],
        script_hash.to_vec(),
        molecule_bytes(data),
    ])
}

fn dynvec_bytes(items: &[Vec<u8>]) -> Vec<u8> {
    if items.is_empty() {
        return 4u32.to_le_bytes().to_vec();
    }
    let header_size = 4 + items.len() * 4;
    let total_size = header_size + items.iter().map(Vec::len).sum::<usize>();
    let mut bytes = Vec::with_capacity(total_size);
    bytes.extend_from_slice(&(total_size as u32).to_le_bytes());
    let mut offset = header_size;
    for item in items {
        bytes.extend_from_slice(&(offset as u32).to_le_bytes());
        offset += item.len();
    }
    for item in items {
        bytes.extend_from_slice(item);
    }
    bytes
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
