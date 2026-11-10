//! The data types that are stored on disk and sent between servers.
//!
//! These structs define the on-disk JSON format, so renaming a field or
//! changing its type changes the format. Add new fields with `#[serde(default)]`
//! so older files still load.

use serde::{Deserialize, Serialize};

use crate::envelope::Signed;
use crate::keys::NewKeys;

/// Who can see a version. Stored and sent as the lowercase string.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Reach {
    /// Listed in the directory and open to search engines.
    Public,
    /// Listed in the directory but kept out of search.
    Commons,
    /// Encrypted to the members of this node; never federated.
    Members,
}

impl Reach {
    /// The stored and signed spelling (`"public"`, `"commons"`, `"members"`).
    pub fn as_str(self) -> &'static str {
        match self {
            Reach::Public => "public",
            Reach::Commons => "commons",
            Reach::Members => "members",
        }
    }

    /// Parse the stored spelling. Unknown values give `None`.
    pub fn parse(s: &str) -> Option<Reach> {
        match s {
            "public" => Some(Reach::Public),
            "commons" => Some(Reach::Commons),
            "members" => Some(Reach::Members),
            _ => None,
        }
    }

    /// The reach chosen on the publish form; anything unexpected becomes public.
    pub fn from_form(s: &str) -> Reach {
        match s {
            "commons" => Reach::Commons,
            "members" => Reach::Members,
            _ => Reach::Public,
        }
    }
}

/// One member's identity on a node: their handle, keys and current version.
/// Lives in `handles/<handle>.json`. The private keys never leave the node.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct HandleRec {
    pub handle: String,
    pub display_name: String,
    pub created_at: u64,
    /// Ed25519 public key, hex. Signs every version.
    pub ed_pub: String,
    /// Ed25519 private key, hex. Server-held; never served.
    pub ed_priv: String,
    /// X25519 public key, hex. Used to encrypt members-only versions.
    pub box_pub: String,
    /// X25519 private key, hex. Server-held; never served.
    pub box_priv: String,
    /// Passphrase hash. Either a PHC string (`$argon2id$…`) or, for handles
    /// created before November 2026, a hex SHA-256 paired with `secret_salt`.
    pub secret_salt: String,
    pub secret_hash: String,
    /// The `cid` of the version currently shown, if any has been published.
    pub current_cid: Option<String>,
    /// Commons this handle has shared to, so the owner can re-share in one click.
    #[serde(default)]
    pub shared_to: Vec<String>,
    /// The public half of the recovery key (the private half is printed, never stored).
    #[serde(default)]
    pub recovery_pub: Option<String>,
    /// The signed declaration of that recovery key, sent with each share so a
    /// commons records it (see [`crate::identity`]).
    #[serde(default)]
    pub recovery_decl: Option<Signed>,
    /// A key change sent to some commons but not yet all; retried with the
    /// same new keys so no commons is left on a different key.
    #[serde(default)]
    pub pending_rotation: Option<PendingRotation>,
}

impl HandleRec {
    /// A new member record with these keys and passphrase hash.
    pub fn new(
        handle: &str,
        display_name: &str,
        created_at: u64,
        k: NewKeys,
        secret_salt: String,
        secret_hash: String,
    ) -> HandleRec {
        HandleRec {
            handle: handle.into(),
            display_name: display_name.into(),
            created_at,
            ed_pub: k.ed_pub,
            ed_priv: k.ed_priv,
            box_pub: k.box_pub,
            box_priv: k.box_priv,
            secret_salt,
            secret_hash,
            current_cid: None,
            shared_to: Vec::new(),
            recovery_pub: None,
            recovery_decl: None,
            pending_rotation: None,
        }
    }
}

/// A key change in progress on a node.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct PendingRotation {
    /// The keys the handle is moving to.
    pub keys: NewKeys,
    /// The signed `KeyRotation` or `RecoveryRotation`.
    pub change: Signed,
    /// For a recovery: the next recovery key `(public, private)`. The private
    /// half is shown to the member once the change is complete, then dropped.
    pub next_recovery: Option<(String, String)>,
    /// Commons that have accepted the change.
    pub done: Vec<String>,
}

/// The profile someone publishes.
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct Profile {
    pub name: String,
    pub pronouns: String,
    pub role: String,
    pub bio: String,
    pub places: Vec<Place>,
    /// A small `data:image/…;base64,` picture. Not part of the content hash
    /// (see [`crate::canonical::hash_body`]).
    #[serde(default)]
    pub avatar: String,
}

/// A place someone works.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Place {
    pub name: String,
    pub city: String,
}

/// A members-only profile, encrypted to one member.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct MemberBox {
    pub handle: String,
    /// Hex X25519 public key of the one-time key used for this member.
    pub eph_pub: String,
    /// Hex nonce.
    pub nonce: String,
    /// Base64 ciphertext of the profile JSON.
    pub ct: String,
}

/// One published version. Immutable once written: `versions/<cid>.json`.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Version {
    pub cid: String,
    pub handle: String,
    /// `public`, `commons` or `members` (see [`Reach`]).
    pub visibility: String,
    pub license: String,
    pub signals: Vec<String>,
    pub created_at: u64,
    /// Hex Ed25519 signature over the `cid` string.
    pub signature: String,
    /// The profile, for public and commons versions.
    pub payload: Option<Profile>,
    /// One encrypted copy per member, for members-only versions.
    pub enc: Option<Vec<MemberBox>>,
}

impl Version {
    /// The version's reach. Unknown values are treated as members-only, the
    /// most restrictive choice.
    pub fn reach(&self) -> Reach {
        Reach::parse(&self.visibility).unwrap_or(Reach::Members)
    }
}

/// A profile shared to this commons by another node, after its signature
/// was checked. Lives in `remote/<keyid>.json`.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct RemoteEntry {
    /// The sender's Ed25519 public key: the portable identity.
    pub ed_pub: String,
    pub box_pub: String,
    pub handle: String,
    pub display_name: String,
    /// The node's address, as the node states it. Not verified.
    pub origin: String,
    /// The node's name for itself, as the node states it. Not verified.
    pub origin_label: String,
    pub cid: String,
    /// `public` or `commons`; members-only versions are never accepted.
    pub visibility: String,
    pub license: String,
    pub signals: Vec<String>,
    pub created_at: u64,
    pub profile: Profile,
    pub signature: String,
    /// When this commons received it.
    pub received_at: u64,
    /// The signed `hic/v1` Profile this entry came from, when the node sent
    /// one: then every field above is covered by the signature. `None` for
    /// shares in the older format, where only the content hash is signed.
    #[serde(default)]
    pub signed: Option<Signed>,
}

/// What a commons keeps after someone withdraws their shared profile: just
/// enough to keep the handle theirs and to refuse a replay of the withdrawn
/// version. Lives in `tombstones/<keyid>.json`. Holds no profile content.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Tombstone {
    pub ed_pub: String,
    pub handle: String,
    /// The withdrawal's own timestamp, as signed by the owner. Shares created
    /// at or before this time are refused.
    pub withdrawn_at: u64,
}

/// A share waiting on a commons' pending list for a member to vouch for it.
/// Lives in `pending/<keyid>.json`.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct PendingShare {
    /// The verified share, as it would be listed.
    pub entry: RemoteEntry,
    /// The message as received (it may carry a recovery-key declaration).
    pub message: serde_json::Value,
}

/// The name shown for a profile: the chosen name, or the handle if blank.
pub fn display_name_or_handle(profile_name: &str, handle: &str) -> String {
    if profile_name.is_empty() {
        handle.to_string()
    } else {
        profile_name.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reach_round_trips_through_its_spelling() {
        for r in [Reach::Public, Reach::Commons, Reach::Members] {
            assert_eq!(Reach::parse(r.as_str()), Some(r));
        }
        assert_eq!(Reach::parse("everyone"), None);
    }

    #[test]
    fn unexpected_form_reach_becomes_public() {
        assert_eq!(Reach::from_form("members"), Reach::Members);
        assert_eq!(Reach::from_form("nonsense"), Reach::Public);
    }

    #[test]
    fn reach_serialises_lowercase() {
        assert_eq!(serde_json::to_string(&Reach::Commons).unwrap(), "\"commons\"");
    }

    #[test]
    fn display_name_falls_back_to_handle() {
        assert_eq!(display_name_or_handle("", "tega"), "tega");
        assert_eq!(display_name_or_handle("Tega Brain", "tega"), "Tega Brain");
    }
}
