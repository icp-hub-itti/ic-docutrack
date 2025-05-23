use candid::CandidType;
use serde::{Deserialize, Serialize};

/// User decryption key
#[derive(Debug, Clone, Copy, PartialEq, Eq, CandidType)]
pub struct OwnerKey([u8; Self::KEY_SIZE]);

impl OwnerKey {
    /// The size of the `OwnerKey` in bytes.
    pub const KEY_SIZE: usize = 512;

    /// Creates a new `OwnerKey` from a byte array.
    ///
    /// # Panics
    ///
    /// Panics if the length of the byte array is not equal to `OwnerKey::KEY_SIZE`.
    pub fn new(key: [u8; Self::KEY_SIZE]) -> Self {
        OwnerKey(key)
    }

    /// Returns the underlying byte array.
    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }
}

impl From<[u8; Self::KEY_SIZE]> for OwnerKey {
    fn from(key: [u8; Self::KEY_SIZE]) -> Self {
        OwnerKey(key)
    }
}

impl Serialize for OwnerKey {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_bytes(&self.0)
    }
}

impl<'de> Deserialize<'de> for OwnerKey {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let bytes = Vec::<u8>::deserialize(deserializer)?;
        if bytes.len() != Self::KEY_SIZE {
            return Err(serde::de::Error::custom(format!(
                "Invalid length for OwnerKey: expected {}, got {}",
                Self::KEY_SIZE,
                bytes.len()
            )));
        }
        let mut array = [0; Self::KEY_SIZE];
        array.copy_from_slice(&bytes);
        Ok(OwnerKey(array))
    }
}
