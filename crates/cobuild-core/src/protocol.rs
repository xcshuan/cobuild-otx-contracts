use core::convert::TryFrom;

use crate::error::CoreError;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ScriptRole {
    InputLock,
    InputType,
    OutputType,
}

impl TryFrom<u8> for ScriptRole {
    type Error = CoreError;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::InputLock),
            1 => Ok(Self::InputType),
            2 => Ok(Self::OutputType),
            _ => Err(CoreError::InvalidMessageTarget),
        }
    }
}

pub const APPEND_PERMISSION_INPUTS_BIT: u8 = 0;
pub const APPEND_PERMISSION_OUTPUTS_BIT: u8 = 1;
pub const APPEND_PERMISSION_CELL_DEPS_BIT: u8 = 2;
pub const APPEND_PERMISSION_HEADER_DEPS_BIT: u8 = 3;
pub const APPEND_PERMISSION_ALLOWED_MASK: u8 = 0x0f;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AppendPermissions {
    raw: u8,
}

impl AppendPermissions {
    pub fn allows(self, bit: u8) -> bool {
        let Some(mask) = 1u8.checked_shl(u32::from(bit)) else {
            return false;
        };
        self.raw & mask != 0
    }

    pub fn require_allowed(self, bit: u8, count: usize) -> Result<(), CoreError> {
        if count > 0 && !self.allows(bit) {
            Err(CoreError::InvalidOtxLayout)
        } else {
            Ok(())
        }
    }
}

impl TryFrom<u8> for AppendPermissions {
    type Error = CoreError;

    fn try_from(raw: u8) -> Result<Self, Self::Error> {
        if raw & !APPEND_PERMISSION_ALLOWED_MASK != 0 {
            return Err(CoreError::InvalidOtxLayout);
        }
        Ok(Self { raw })
    }
}

pub const SEGMENT_FLAG_ALLOW_MORE_AFTER: u8 = 0x01;
pub const SEGMENT_FLAG_COVERAGE_PREVIOUS: u8 = 0x02;
pub const SEGMENT_FLAG_ALLOWED_MASK: u8 =
    SEGMENT_FLAG_ALLOW_MORE_AFTER | SEGMENT_FLAG_COVERAGE_PREVIOUS;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SegmentFlags {
    raw: u8,
}

impl SegmentFlags {
    pub fn raw(self) -> u8 {
        self.raw
    }

    pub fn allow_more_segments_after(self) -> bool {
        self.raw & SEGMENT_FLAG_ALLOW_MORE_AFTER != 0
    }

    pub fn coverage_previous_segments(self) -> bool {
        self.raw & SEGMENT_FLAG_COVERAGE_PREVIOUS != 0
    }
}

impl TryFrom<u8> for SegmentFlags {
    type Error = CoreError;

    fn try_from(raw: u8) -> Result<Self, Self::Error> {
        if raw & !SEGMENT_FLAG_ALLOWED_MASK != 0 {
            return Err(CoreError::InvalidOtxLayout);
        }
        Ok(Self { raw })
    }
}

#[cfg(test)]
mod tests {
    use super::{
        AppendPermissions, SegmentFlags, APPEND_PERMISSION_CELL_DEPS_BIT,
        APPEND_PERMISSION_HEADER_DEPS_BIT, APPEND_PERMISSION_INPUTS_BIT,
        APPEND_PERMISSION_OUTPUTS_BIT, SEGMENT_FLAG_ALLOW_MORE_AFTER,
        SEGMENT_FLAG_COVERAGE_PREVIOUS,
    };
    use crate::error::CoreError;

    #[test]
    fn append_permissions_accept_entity_append_bits() {
        let permissions = AppendPermissions::try_from(0x0f).unwrap();

        assert!(permissions.allows(APPEND_PERMISSION_INPUTS_BIT));
        assert!(permissions.allows(APPEND_PERMISSION_OUTPUTS_BIT));
        assert!(permissions.allows(APPEND_PERMISSION_CELL_DEPS_BIT));
        assert!(permissions.allows(APPEND_PERMISSION_HEADER_DEPS_BIT));
    }

    #[test]
    fn append_permissions_reject_reserved_bits() {
        assert_eq!(
            AppendPermissions::try_from(0x10),
            Err(CoreError::InvalidOtxLayout)
        );
        assert_eq!(
            AppendPermissions::try_from(0x80),
            Err(CoreError::InvalidOtxLayout)
        );
    }

    #[test]
    fn append_permissions_treat_out_of_range_bits_as_disallowed() {
        let permissions = AppendPermissions::try_from(0x0f).unwrap();

        assert!(!permissions.allows(8));
        assert_eq!(
            permissions.require_allowed(8, 1),
            Err(CoreError::InvalidOtxLayout)
        );
    }

    #[test]
    fn segment_flags_accept_defined_bits() {
        let flags = SegmentFlags::try_from(0x03).unwrap();

        assert_eq!(
            flags.raw(),
            SEGMENT_FLAG_ALLOW_MORE_AFTER | SEGMENT_FLAG_COVERAGE_PREVIOUS
        );
        assert!(flags.allow_more_segments_after());
        assert!(flags.coverage_previous_segments());
    }

    #[test]
    fn segment_flags_reject_reserved_bits() {
        assert_eq!(
            SegmentFlags::try_from(0x04),
            Err(CoreError::InvalidOtxLayout)
        );
        assert_eq!(
            SegmentFlags::try_from(0x80),
            Err(CoreError::InvalidOtxLayout)
        );
    }
}
