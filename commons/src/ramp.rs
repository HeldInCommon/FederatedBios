//! The ramp (decision record 0022): what a member is shown, and when.
//!
//! Their profile page offers **one next step** at a time and groups everything
//! else into tabs that appear once they're useful. Both follow from what has
//! happened (published? shared? listed? backed up?), never from a setting, and
//! a tab never disappears once it has appeared for a reason that still holds.
//!
//! Pure: the handler gathers a [`MemberState`] and the pages render the result.

/// One tab of a member's own profile page.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tab {
    /// Who they are: the profile itself.
    Profile,
    /// Where their work goes: sharing, waiting, withdrawing.
    Sharing,
    /// Who they bring in: invites and vouches.
    People,
    /// Keeping their identity safe: backup and keys.
    Account,
}

impl Tab {
    /// Every tab, in page order.
    pub const ALL: [Tab; 4] = [Tab::Profile, Tab::Sharing, Tab::People, Tab::Account];

    /// Its name in the address (`?tab=…`).
    pub fn key(self) -> &'static str {
        match self {
            Tab::Profile => "profile",
            Tab::Sharing => "sharing",
            Tab::People => "people",
            Tab::Account => "account",
        }
    }

    /// Its label.
    pub fn label(self) -> &'static str {
        match self {
            Tab::Profile => "Profile",
            Tab::Sharing => "Sharing",
            Tab::People => "People",
            Tab::Account => "Account",
        }
    }

    /// Read [`Tab::key`] back; anything else is the profile.
    pub fn parse(s: Option<&str>) -> Tab {
        Tab::ALL.into_iter().find(|t| Some(t.key()) == s).unwrap_or(Tab::Profile)
    }
}

/// What the ramp needs to know about a member.
#[derive(Clone, Copy, Debug)]
pub struct MemberState<'a> {
    /// They have published a profile.
    pub published: bool,
    /// Their current profile may be shared (it isn't members-only).
    pub shareable: bool,
    /// The commons offered first: the node's default, else one shared to before.
    pub suggested_commons: Option<&'a str>,
    /// Commons they've shared to.
    pub shared_to: &'a [String],
    /// Commons where they're still waiting for a vouch.
    pub waiting_on: &'a [String],
    /// They've downloaded a backup, or said "Not now".
    pub backup_done: bool,
}

impl MemberState<'_> {
    /// The commons that list them: shared to, and not waiting there.
    pub fn listed_on(&self) -> Vec<String> {
        self.shared_to.iter().filter(|c| !self.waiting_on.contains(c)).cloned().collect()
    }
}

/// The one thing the profile page suggests next.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NextStep {
    /// Publish a profile.
    Publish,
    /// Share it: to this commons, or (`None`) to one they'll choose.
    Share(Option<String>),
    /// Waiting to be listed on these commons.
    Waiting(Vec<String>),
    /// Download a backup.
    Backup,
    /// Nothing to suggest.
    Done,
}

/// The next step, in ramp order: publish, share, get listed, back up.
/// Members-only profiles skip sharing and go straight to backing up.
pub fn next_step(s: &MemberState) -> NextStep {
    if !s.published {
        return NextStep::Publish;
    }
    if s.shareable && s.shared_to.is_empty() {
        return NextStep::Share(s.suggested_commons.map(String::from));
    }
    let listed = !s.listed_on().is_empty();
    if !listed && !s.waiting_on.is_empty() {
        return NextStep::Waiting(s.waiting_on.to_vec());
    }
    if !s.backup_done && (listed || !s.shareable) {
        return NextStep::Backup;
    }
    NextStep::Done
}

/// The tabs a member sees: Profile always; Sharing and Account once there is a
/// profile to share or keep safe; People once they're listed somewhere (before
/// that there's nobody to invite to, and nothing to vouch with).
pub fn tabs(s: &MemberState) -> Vec<Tab> {
    let mut out = vec![Tab::Profile];
    if s.published {
        out.push(Tab::Sharing);
    }
    if !s.listed_on().is_empty() {
        out.push(Tab::People);
    }
    if s.published {
        out.push(Tab::Account);
    }
    out
}

/// The tab to show: the one asked for if the member can see it, else Profile.
pub fn active_tab(asked: Tab, visible: &[Tab]) -> Tab {
    if visible.contains(&asked) {
        asked
    } else {
        Tab::Profile
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s<'a>(published: bool, shared: &'a [String], waiting: &'a [String], backup_done: bool) -> MemberState<'a> {
        MemberState {
            published,
            shareable: true,
            suggested_commons: Some("http://room:8081"),
            shared_to: shared,
            waiting_on: waiting,
            backup_done,
        }
    }

    #[test]
    fn the_steps_follow_what_has_happened() {
        let room = vec!["http://room:8081".to_string()];
        assert_eq!(next_step(&s(false, &[], &[], false)), NextStep::Publish);
        assert_eq!(next_step(&s(true, &[], &[], false)), NextStep::Share(Some("http://room:8081".into())));
        assert_eq!(next_step(&s(true, &room, &room, false)), NextStep::Waiting(room.clone()));
        assert_eq!(next_step(&s(true, &room, &[], false)), NextStep::Backup);
        assert_eq!(next_step(&s(true, &room, &[], true)), NextStep::Done);
    }

    #[test]
    fn without_a_default_commons_they_choose_one() {
        let none = MemberState { suggested_commons: None, ..s(true, &[], &[], false) };
        assert_eq!(next_step(&none), NextStep::Share(None));
    }

    #[test]
    fn members_only_profiles_skip_sharing() {
        let private = MemberState { shareable: false, ..s(true, &[], &[], false) };
        assert_eq!(next_step(&private), NextStep::Backup);
    }

    #[test]
    fn listed_somewhere_beats_waiting_elsewhere() {
        let both = vec!["http://a".to_string(), "http://b".to_string()];
        let waiting = vec!["http://b".to_string()];
        assert_eq!(next_step(&s(true, &both, &waiting, false)), NextStep::Backup);
    }

    #[test]
    fn tabs_arrive_as_they_become_useful() {
        let room = vec!["http://room:8081".to_string()];
        assert_eq!(tabs(&s(false, &[], &[], false)), vec![Tab::Profile]);
        assert_eq!(tabs(&s(true, &[], &[], false)), vec![Tab::Profile, Tab::Sharing, Tab::Account]);
        assert_eq!(tabs(&s(true, &room, &room, false)), vec![Tab::Profile, Tab::Sharing, Tab::Account], "waiting");
        assert_eq!(tabs(&s(true, &room, &[], false)), Tab::ALL.to_vec(), "listed");
    }

    #[test]
    fn a_hidden_tab_falls_back_to_the_profile() {
        assert_eq!(Tab::parse(Some("people")), Tab::People);
        assert_eq!(Tab::parse(Some("nonsense")), Tab::Profile);
        assert_eq!(active_tab(Tab::People, &[Tab::Profile, Tab::Sharing]), Tab::Profile);
        assert_eq!(active_tab(Tab::Sharing, &[Tab::Profile, Tab::Sharing]), Tab::Sharing);
    }
}
