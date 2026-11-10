//! Ready-written invitation emails, as `mailto:` links.
//!
//! The server never sends mail: the link opens the person's own mail app with
//! the message filled in, and they choose who to send it to. So no email
//! address ever passes through a node or a commons.

/// Where newcomers download the app.
pub const APP_DOWNLOAD: &str = "https://github.com/HeldInCommon/FederatedBios/releases/tag/node-app-preview";

/// The subject and body of an invitation to `commons_label` (reached at
/// `commons_url`), from `from` (e.g. "@ada", or "the steward of …").
pub fn invitation(
    commons_label: &str,
    commons_url: &str,
    code: &str,
    from: &str,
    valid_days: Option<u64>,
) -> (String, String) {
    let subject = format!("An invitation to {commons_label} on Held in Common");
    let validity = match valid_days {
        Some(d) => format!("The code works once and expires in {d} days."),
        None => "The code works once.".to_string(),
    };
    let body = format!(
        "Hi,\n\n\
         You're invited to join {commons_label}, a commons on Held in Common: a federated \
         commons where creative people keep their work on their own computer and choose what to share.\n\n\
         1. Install the Held in Common app: {APP_DOWNLOAD}\n\
         2. Open it, choose \"A node\", and give it a name.\n\
         3. Create your handle and profile.\n\
         4. On your profile, under \"Share to a commons\", choose \"Share with an invite code, or to another commons\", \
         enter {commons_url} and paste this invite code:\n\n\
         {code}\n\n\
         {validity}\n\n\
         {from}\n"
    );
    (subject, body)
}

/// Percent-encode text for a `mailto:` link (everything but unreserved
/// characters, so spaces become %20 and new lines %0A).
fn encode(text: &str) -> String {
    text.bytes()
        .map(|b| match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => (b as char).to_string(),
            _ => format!("%{b:02X}"),
        })
        .collect()
}

/// A `mailto:` link with no recipient (the person fills it in), the subject
/// and the body.
pub fn mailto(subject: &str, body: &str) -> String {
    format!("mailto:?subject={}&body={}", encode(subject), encode(body))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invitation_contains_the_steps_and_the_code() {
        let (subject, body) = invitation("Room 4", "http://192.168.1.5:8080", "hic-invite-abc", "— @ada", Some(14));
        assert!(subject.contains("Room 4"));
        for part in [APP_DOWNLOAD, "http://192.168.1.5:8080", "hic-invite-abc", "expires in 14 days", "— @ada"] {
            assert!(body.contains(part), "{part}");
        }
    }

    #[test]
    fn mailto_is_encoded() {
        let link = mailto("Hi there", "line one\nline two & more");
        assert_eq!(link, "mailto:?subject=Hi%20there&body=line%20one%0Aline%20two%20%26%20more");
    }
}
