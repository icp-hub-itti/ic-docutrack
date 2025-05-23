use std::rc::Rc;

use candid::CandidType;
use ic_stable_structures::Storable;
use ic_stable_structures::storable::Bound;
use serde::ser::SerializeSeq;
use serde::{Deserialize, Serialize};

/// Public key for users
///
/// Currently it it a DER encoded RSA 4096 bit public key
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct PublicKey {
    bytes: [u8; Self::MAX_KEY_SIZE],
    len: u16,
}

impl Default for PublicKey {
    fn default() -> Self {
        PublicKey {
            bytes: [0; Self::MAX_KEY_SIZE],
            len: 0,
        }
    }
}

impl TryFrom<&[u8]> for PublicKey {
    type Error = String;

    fn try_from(bytes: &[u8]) -> Result<Self, Self::Error> {
        if bytes.len() > Self::MAX_KEY_SIZE {
            return Err(format!(
                "Public key is too long: {} bytes, max size is {} bytes",
                bytes.len(),
                Self::MAX_KEY_SIZE
            ));
        }

        let mut arr = [0; Self::MAX_KEY_SIZE];
        arr[..bytes.len()].copy_from_slice(bytes);

        Ok(PublicKey {
            bytes: arr,
            len: bytes.len() as u16,
        })
    }
}

impl TryFrom<Vec<u8>> for PublicKey {
    type Error = String;

    fn try_from(bytes: Vec<u8>) -> Result<Self, Self::Error> {
        Self::try_from(bytes.as_slice())
    }
}

impl PublicKey {
    /// Maximum size of the public key
    ///
    /// Actually it should never be larger than 700 bytes
    pub const MAX_KEY_SIZE: usize = 720;

    /// Size of the length of the key
    pub const KEY_LEN_SIZE: usize = 2;

    /// Returns the key as a byte slice
    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes[..self.len as usize]
    }

    /// Returns the required size for encoding
    pub fn encoding_size(&self) -> usize {
        self.len as usize + Self::KEY_LEN_SIZE
    }
}

impl CandidType for PublicKey {
    fn _ty() -> candid::types::Type {
        candid::types::Type(Rc::new(candid::types::TypeInner::Vec(candid::types::Type(
            Rc::new(candid::types::TypeInner::Nat8),
        ))))
    }

    fn idl_serialize<S>(&self, serializer: S) -> Result<(), S::Error>
    where
        S: candid::types::Serializer,
    {
        serializer.serialize_blob(self.as_bytes())
    }
}

impl Serialize for PublicKey {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let bytes = self.as_bytes();
        let mut seq = serializer.serialize_seq(Some(bytes.len()))?;
        for byte in bytes {
            seq.serialize_element(byte)?;
        }
        seq.end()
    }
}

impl<'de> Deserialize<'de> for PublicKey {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let bytes: Vec<u8> = Vec::deserialize(deserializer)?;
        PublicKey::try_from(bytes).map_err(serde::de::Error::custom)
    }
}

impl Storable for PublicKey {
    const BOUND: Bound = Bound::Bounded {
        max_size: Self::MAX_KEY_SIZE as u32 + Self::KEY_LEN_SIZE as u32, // 2 for length
        is_fixed_size: false,
    };

    fn from_bytes(bytes: std::borrow::Cow<[u8]>) -> Self {
        let len: u16 = u16::from_le_bytes(bytes[0..2].try_into().expect("Invalid length"));
        let mut arr = [0; Self::MAX_KEY_SIZE];
        arr[..len as usize].copy_from_slice(&bytes[2..2 + len as usize]);
        PublicKey { bytes: arr, len }
    }

    fn to_bytes(&self) -> std::borrow::Cow<[u8]> {
        let mut bytes = Vec::with_capacity(2 + self.len as usize);
        bytes.extend_from_slice(&self.len.to_le_bytes());
        bytes.extend_from_slice(&self.bytes[..self.len as usize]);
        bytes.into()
    }
}

#[cfg(test)]
mod test {

    use super::*;

    #[test]
    fn test_public_key_roundtrip() {
        let public_key = PublicKey::try_from(vec![1, 2, 3, 4, 5]).unwrap();
        let bytes = public_key.to_bytes();
        let public_key2 = PublicKey::from_bytes(bytes);
        assert_eq!(public_key, public_key2);
        assert_eq!(public_key.as_bytes(), public_key2.as_bytes());
        assert_eq!(public_key.encoding_size(), public_key2.encoding_size());
        assert_eq!(public_key.len, public_key2.len);
    }
}
