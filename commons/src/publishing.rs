//! Building a signed profile version (format version 0, see [`crate::canonical`]).
//!
//! Used when a member publishes, and when a node re-signs the current
//! profile with a new key after a key rotation. Pure apart from the fresh
//! one-time keys used to encrypt members-only versions.

use crate::canonical::{content_id, hash_body};
use crate::keys;
use crate::model::{HandleRec, Profile, Reach, Version};

/// What a version says, before it is signed.
pub struct Draft {
    pub profile: Profile,
    pub reach: Reach,
    pub license: String,
    pub signals: Vec<String>,
    pub created_at: u64,
}

/// Sign a draft with the member's key. For members-only versions, the profile
/// is encrypted once per recipient (`(handle, box_pub)`); otherwise it is
/// stored in the clear.
pub fn sign_version(rec: &HandleRec, d: Draft, recipients: &[(String, String)]) -> Result<Version, &'static str> {
    let (payload, enc, body) = if d.reach == Reach::Members {
        let boxes = keys::seal_for_members(&d.profile, recipients);
        let body = serde_json::to_value(&boxes).expect("boxes serialise");
        (None, Some(boxes), body)
    } else {
        let body = hash_body(&d.profile);
        (Some(d.profile), None, body)
    };
    let cid = content_id(&rec.handle, d.reach.as_str(), &d.license, &d.signals, d.created_at, &body);
    let signature = keys::sign_cid(&rec.ed_priv, &cid)?;
    Ok(Version {
        cid,
        handle: rec.handle.clone(),
        visibility: d.reach.as_str().into(),
        license: d.license,
        signals: d.signals,
        created_at: d.created_at,
        signature,
        payload,
        enc,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rec() -> HandleRec {
        let k = keys::generate();
        HandleRec::new("tega", "Tega", 0, k, String::new(), String::new())
    }

    fn draft(reach: Reach) -> Draft {
        Draft {
            profile: Profile { name: "Tega".into(), ..Default::default() },
            reach,
            license: "cc-by".into(),
            signals: vec![],
            created_at: 5,
        }
    }

    #[test]
    fn public_versions_verify() {
        let r = rec();
        let v = sign_version(&r, draft(Reach::Public), &[]).unwrap();
        assert!(keys::verify_cid(&r.ed_pub, &v.cid, &v.signature).is_ok());
        assert!(v.payload.is_some() && v.enc.is_none());
    }

    #[test]
    fn members_versions_are_encrypted() {
        let r = rec();
        let v = sign_version(&r, draft(Reach::Members), &[("tega".into(), r.box_pub.clone())]).unwrap();
        assert!(v.payload.is_none());
        assert_eq!(keys::open_for(v.enc.as_deref().unwrap(), "tega", &r.box_priv).unwrap().name, "Tega");
    }

    #[test]
    fn a_damaged_key_is_an_error() {
        let mut r = rec();
        r.ed_priv = "00".into();
        assert!(sign_version(&r, draft(Reach::Public), &[]).is_err());
    }
}
