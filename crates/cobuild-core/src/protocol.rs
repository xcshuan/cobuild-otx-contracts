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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SealScope {
    Base,
    Append,
}

impl SealScope {
    pub fn raw(self) -> u8 {
        match self {
            Self::Base => 0,
            Self::Append => 1,
        }
    }
}

impl TryFrom<u8> for SealScope {
    type Error = CoreError;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::Base),
            1 => Ok(Self::Append),
            _ => Err(CoreError::InvalidSealScope),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AppendPermissions {
    raw: u8,
}

impl AppendPermissions {
    pub fn allows(self, bit: u8) -> bool {
        self.raw & (1 << bit) != 0
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
        if raw & 0xf0 != 0 {
            return Err(CoreError::InvalidOtxLayout);
        }
        Ok(Self { raw })
    }
}
