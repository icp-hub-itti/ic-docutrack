use candid::{CandidType, Principal};
use ic_stable_structures::Storable;
use ic_stable_structures::storable::Bound;
use serde::{Deserialize, Serialize};

use super::PublicKey;

/// Maximum username size
pub const MAX_USERNAME_SIZE: usize = 255;

#[derive(CandidType, Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct User {
    pub public_key: PublicKey,
    pub username: String,
}

impl Storable for User {
    /// 1 for username length, up to 255 for username, 32 for public key
    const BOUND: Bound = Bound::Bounded {
        max_size: 1 + MAX_USERNAME_SIZE as u32 + PublicKey::BOUND.max_size(),
        is_fixed_size: false,
    };

    fn from_bytes(bytes: std::borrow::Cow<[u8]>) -> Self {
        let username_len: u8 = bytes[0];
        let username = String::from_utf8_lossy(&bytes[1..1 + username_len as usize]).to_string();

        let pubkey_start = 1 + username_len as usize;
        let public_key = PublicKey::from_bytes(bytes[pubkey_start..].into());

        User {
            username,
            public_key,
        }
    }

    fn to_bytes(&self) -> std::borrow::Cow<[u8]> {
        let username_len = self.username.len() as u8;
        let mut bytes =
            Vec::with_capacity(1 + username_len as usize + self.public_key.encoding_size() + 29);

        // encode username
        bytes.push(username_len);
        bytes.extend_from_slice(self.username.as_bytes());

        // encode public key
        bytes.extend_from_slice(&self.public_key.to_bytes());

        bytes.into()
    }
}

/// Public user information
#[derive(CandidType, Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct PublicUser {
    pub username: String,
    pub public_key: PublicKey,
    pub ic_principal: Principal,
}

impl PublicUser {
    /// Create a new PublicUser instance
    pub fn new(user: User, ic_principal: Principal) -> Self {
        PublicUser {
            username: user.username,
            public_key: user.public_key,
            ic_principal,
        }
    }
}

/// Response for the set_user method
#[derive(CandidType, Serialize, Deserialize, Debug, PartialEq, Eq)]
pub enum SetUserResponse {
    #[serde(rename = "ok")]
    Ok,
    /// The username is too long
    #[serde(rename = "username_too_long")]
    UsernameTooLong,
    /// The username already exists
    #[serde(rename = "username_exists")]
    UsernameExists,
    /// The caller is anonymous
    #[serde(rename = "anonymous_caller")]
    AnonymousCaller,
    /// The caller already has a user
    #[serde(rename = "caller_has_already_a_user")]
    CallerHasAlreadyAUser,
}

/// Response for the get_users method
#[derive(CandidType, Serialize, Deserialize, Clone, Debug, PartialEq)]
pub enum GetUsersResponse {
    #[serde(rename = "permission_error")]
    PermissionError,
    #[serde(rename = "users")]
    Users(GetUsersResponseUsers),
}

/// Response for the get_users method with pagination
#[derive(CandidType, Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct GetUsersResponseUsers {
    /// Returned users
    pub users: Vec<PublicUser>,
    /// The next page offset. If None, there are no more users to fetch
    pub next: Option<u64>,
    /// Total number of users
    pub total: u64,
}

#[cfg(test)]
mod test {

    use super::*;

    #[test]
    fn test_storable_user_roundtrip() {
        let user = User {
            username: "test_user".to_string(),
            public_key: vec![1; 5].try_into().unwrap(),
        };

        let bytes = user.to_bytes();
        let decoded_user = User::from_bytes(bytes);

        assert_eq!(user, decoded_user);
    }

    #[test]
    fn test_should_create_public_user_from_user() {
        let user = User {
            username: "test_user".to_string(),
            public_key: vec![1; PublicKey::MAX_KEY_SIZE].try_into().unwrap(),
        };
        let ic_principal = Principal::from_slice(&[2; 29]);

        let public_user = PublicUser::new(user.clone(), ic_principal);

        assert_eq!(public_user.username, user.username);
        assert_eq!(public_user.public_key, user.public_key);
        assert_eq!(public_user.ic_principal, ic_principal);
    }
}
