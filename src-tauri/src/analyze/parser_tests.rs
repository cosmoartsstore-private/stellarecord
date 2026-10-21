#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    // ── RE_TIME ──

    #[test]
    fn re_time_captures_standard_timestamp() {
        let caps = RE_TIME.captures("2025.04.30 20:15:00 Log        -  some message");
        assert!(caps.is_some());
        assert_eq!(
            caps.as_ref().and_then(|c| c.get(1)).map(|m| m.as_str()),
            Some("2025.04.30 20:15:00")
        );
    }

    #[test]
    fn re_time_rejects_malformed_timestamp() {
        assert!(RE_TIME.captures("not a timestamp").is_none());
        assert!(RE_TIME.captures("2025-04-30 20:15:00").is_none());
    }

    // ── RE_USER_AUTH ──

    #[test]
    fn re_user_auth_captures_name_and_id() {
        let line = "User Authenticated: TestUser (usr_abcdef12-3456-7890-abcd-ef1234567890)";
        let caps = RE_USER_AUTH.captures(line).unwrap();
        assert_eq!(caps.get(1).unwrap().as_str(), "TestUser");
        assert_eq!(
            caps.get(2).unwrap().as_str(),
            "usr_abcdef12-3456-7890-abcd-ef1234567890"
        );
    }

    #[test]
    fn re_user_auth_handles_spaces_in_name() {
        let line = "User Authenticated: Test User Name (usr_0000-1111-2222-3333-444444444444)";
        let caps = RE_USER_AUTH.captures(line).unwrap();
        assert_eq!(caps.get(1).unwrap().as_str(), "Test User Name");
    }

    // ── RE_ENTERING ──

    #[test]
    fn re_entering_captures_room_name() {
        let line = "[Behaviour] Entering Room: My Cool World";
        let caps = RE_ENTERING.captures(line).unwrap();
        assert_eq!(caps.get(1).unwrap().as_str(), "My Cool World");
    }

    // ── RE_JOINING ──

    #[test]
    fn re_joining_captures_full_location() {
        let line = "[Behaviour] Joining wrld_abc123:12345~private(usr_owner)~region(jp)";
        let caps = RE_JOINING.captures(line).unwrap();
        assert_eq!(
            caps.get(1).unwrap().as_str(),
            "wrld_abc123:12345~private(usr_owner)~region(jp)"
        );
    }

    // ── RE_LEFT_ROOM ──

    #[test]
    fn re_left_room_matches() {
        assert!(RE_LEFT_ROOM.is_match("[Behaviour] OnLeftRoom"));
    }

    #[test]
    fn re_left_room_rejects_other_lines() {
        assert!(!RE_LEFT_ROOM.is_match("[Behaviour] OnPlayerJoined SomeUser (usr_123)"));
    }

    // ── RE_PLAYER_JOIN / RE_PLAYER_LEFT ──

    #[test]
    fn re_player_join_captures_name_and_id() {
        let line = "[Behaviour] OnPlayerJoined TestPlayer (usr_aaaa-bbbb-cccc-dddd-eeeeeeeeeeee)";
        let caps = RE_PLAYER_JOIN.captures(line).unwrap();
        assert_eq!(caps.get(1).unwrap().as_str(), "TestPlayer");
        assert_eq!(
            caps.get(2).unwrap().as_str(),
            "usr_aaaa-bbbb-cccc-dddd-eeeeeeeeeeee"
        );
    }

    #[test]
    fn re_player_left_captures_name_and_id() {
        let line = "[Behaviour] OnPlayerLeft TestPlayer (usr_aaaa-bbbb-cccc-dddd-eeeeeeeeeeee)";
        let caps = RE_PLAYER_LEFT.captures(line).unwrap();
        assert_eq!(caps.get(1).unwrap().as_str(), "TestPlayer");
        assert_eq!(
            caps.get(2).unwrap().as_str(),
            "usr_aaaa-bbbb-cccc-dddd-eeeeeeeeeeee"
        );
    }

    // ── RE_IS_LOCAL ──

    #[test]
    fn re_is_local_captures_local_player() {
        let line = r#"[Behaviour] Initialized PlayerAPI "MyName" is local"#;
        let caps = RE_IS_LOCAL.captures(line).unwrap();
        assert_eq!(caps.get(1).unwrap().as_str(), "MyName");
        assert_eq!(caps.get(2).unwrap().as_str(), "local");
    }

    #[test]
    fn re_is_local_captures_remote_player() {
        let line = r#"[Behaviour] Initialized PlayerAPI "OtherPlayer" is remote"#;
        let caps = RE_IS_LOCAL.captures(line).unwrap();
        assert_eq!(caps.get(1).unwrap().as_str(), "OtherPlayer");
        assert_eq!(caps.get(2).unwrap().as_str(), "remote");
    }

    // ── RE_SCREENSHOT ──

    #[test]
    fn re_screenshot_captures_path_and_resolution() {
        let line = r"[VRC Camera] Took screenshot to: C:\Users\test\Pictures\VRChat\VRChat_2025-10-21_00-59-15.520_3840x2160.png";
        let caps = RE_SCREENSHOT.captures(line).unwrap();
        assert!(caps.get(1).unwrap().as_str().contains("VRChat_2025-10-21"));
        assert_eq!(caps.get(2).unwrap().as_str(), "3840");
        assert_eq!(caps.get(3).unwrap().as_str(), "2160");
    }

    // ── RE_OSC_FOUND ──

    #[test]
    fn re_osc_captures_service_info() {
        let line = "Found new OSC Service: OyasumiVR at 127.0.0.1:61080";
        let caps = RE_OSC_FOUND.captures(line).unwrap();
        assert_eq!(caps.get(1).unwrap().as_str(), "OyasumiVR");
        assert_eq!(caps.get(2).unwrap().as_str(), "127.0.0.1");
        assert_eq!(caps.get(3).unwrap().as_str(), "61080");
    }

    // ── RE_SUBSCRIPTION_STATUS ──

    #[test]
    fn re_subscription_status_captures_active() {
        let line = "Get VRChat Subscription Details! Subscription Id:sub_12345 active:True desc:VRChat Plus";
        let caps = RE_SUBSCRIPTION_STATUS.captures(line).unwrap();
        assert_eq!(caps.get(1).unwrap().as_str(), "sub_12345");
        assert_eq!(caps.get(2).unwrap().as_str(), "True");
        assert_eq!(caps.get(3).unwrap().as_str(), "VRChat Plus");
    }

    // ── parse_access_type ──

    #[test]
    fn parse_access_type_private_with_owner() {
        let (access, owner) = parse_access_type("private(usr_owner-id-1234)");
        assert_eq!(access.as_deref(), Some("private"));
        assert_eq!(owner.as_deref(), Some("usr_owner-id-1234"));
    }

    #[test]
    fn parse_access_type_friends() {
        let (access, _) = parse_access_type("friends(usr_abc)");
        assert_eq!(access.as_deref(), Some("friends"));
    }

    #[test]
    fn parse_access_type_hidden() {
        let (access, _) = parse_access_type("hidden(usr_abc)");
        assert_eq!(access.as_deref(), Some("hidden"));
    }

    #[test]
    fn parse_access_type_public() {
        let (access, owner) = parse_access_type("public");
        assert_eq!(access.as_deref(), Some("public"));
        assert!(owner.is_none());
    }

    #[test]
    fn parse_access_type_group() {
        let (access, _) = parse_access_type("group(usr_abc)");
        assert_eq!(access.as_deref(), Some("group"));
    }

    #[test]
    fn parse_access_type_unknown() {
        let (access, owner) = parse_access_type("something_else");
        assert!(access.is_none());
        assert!(owner.is_none());
    }

    // ── is_collectible_notification ──

    #[test]
    fn collectible_notification_types() {
        assert!(is_collectible_notification("boop"));
        assert!(is_collectible_notification("friendRequest"));
        assert!(is_collectible_notification("requestInvite"));
        assert!(is_collectible_notification("invite"));
        assert!(is_collectible_notification("group"));
    }

    #[test]
    fn non_collectible_notification_types() {
        assert!(!is_collectible_notification("broadcast"));
        assert!(!is_collectible_notification("unknown"));
        assert!(!is_collectible_notification(""));
    }

    #[test]
    fn collectible_notification_trims_whitespace() {
        assert!(is_collectible_notification("  boop  "));
    }

    // ── parse_location ──

    #[test]
    fn parse_location_full() {
        let loc = parse_location("wrld_abc123:99999~private(usr_owner-1234)~region(jp)");
        assert_eq!(loc.instance_id.as_deref(), Some("99999"));
        assert_eq!(loc.access_type.as_deref(), Some("private"));
        assert_eq!(loc.instance_owner.as_deref(), Some("usr_owner-1234"));
        assert_eq!(loc.region.as_deref(), Some("jp"));
    }

    #[test]
    fn parse_location_public_no_owner() {
        let loc = parse_location("wrld_abc123:12345~public~region(us)");
        assert_eq!(loc.instance_id.as_deref(), Some("12345"));
        assert_eq!(loc.access_type.as_deref(), Some("public"));
        assert!(loc.instance_owner.is_none());
        assert_eq!(loc.region.as_deref(), Some("us"));
    }

    #[test]
    fn parse_location_group_with_extra_segments() {
        let loc =
            parse_location("wrld_abc123:12345~group(usr_grp)~groupAccessType(public)~region(eu)");
        assert_eq!(loc.instance_id.as_deref(), Some("12345"));
        assert_eq!(loc.access_type.as_deref(), Some("group"));
        assert_eq!(loc.region.as_deref(), Some("eu"));
    }

    #[test]
    fn parse_location_no_world_prefix() {
        let loc = parse_location("not_a_world_id:12345");
        assert!(loc.instance_id.is_none());
        assert!(loc.access_type.is_none());
    }

    #[test]
    fn parse_location_no_colon() {
        let loc = parse_location("wrld_abc123");
        assert!(loc.instance_id.is_none());
    }

    #[test]
    fn parse_location_empty() {
        let loc = parse_location("");
        assert!(loc.instance_id.is_none());
    }
}
