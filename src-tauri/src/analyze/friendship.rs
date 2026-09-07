//! アバター取得ログから訪問時点のフレンド状態を保守的に推定する。
//!
//! `Friend` 値だけでは対象ユーザーを識別できないため、直前のアバター切り替え、
//! ダウンロード完了、展開完了、入室中ユーザーを組み合わせる。対応が一意でない場合や
//! 同一訪問内で証拠が矛盾した場合は、既知状態として扱わない。

use std::collections::{HashMap, HashSet};

/// DB 上で「判定不能」を表す値。
pub const UNKNOWN_FRIEND_STATUS: i64 = 2;

const MAX_DOWNLOAD_UNPACK_LINE_GAP: usize = 3;

/// `VRChat` の旧形式ダウンロードログに含まれるフレンド状態。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FriendStatus {
    NonFriend = 0,
    Friend = 1,
}

impl FriendStatus {
    /// ログ中の `Friend:0` / `Friend:1` を状態へ変換する。
    pub fn from_log_value(value: &str) -> Option<Self> {
        match value {
            "0" => Some(Self::NonFriend),
            "1" => Some(Self::Friend),
            _ => None,
        }
    }

    const fn db_value(self) -> i64 {
        self as i64
    }
}

#[derive(Debug, Clone, Copy, Default)]
enum PendingDownload {
    #[default]
    Empty,
    One {
        status: Option<FriendStatus>,
        line_number: usize,
    },
    Ambiguous,
}

#[derive(Debug, Clone, Copy)]
enum ObservedStatus {
    Known(FriendStatus),
    Conflict,
}

/// 1回のワールド訪問内で、ユーザーとアバター取得イベントの対応を追跡する。
#[derive(Debug, Default)]
pub struct FriendshipTracker {
    active_users_by_display: HashMap<String, HashSet<String>>,
    avatar_by_display: HashMap<String, String>,
    displays_by_avatar: HashMap<String, HashSet<String>>,
    pending_download: PendingDownload,
    observed_by_user: HashMap<String, ObservedStatus>,
}

impl FriendshipTracker {
    /// ワールド訪問境界で、推定に使う一時状態をすべて破棄する。
    pub fn reset_visit(&mut self) {
        self.active_users_by_display.clear();
        self.avatar_by_display.clear();
        self.displays_by_avatar.clear();
        self.pending_download = PendingDownload::Empty;
        self.observed_by_user.clear();
    }

    /// 入室したユーザーを表示名と `VRChat` ID の組で記録する。
    pub fn on_player_join(&mut self, display_name: &str, user_id: &str) {
        let is_ambiguous = {
            let users = self
                .active_users_by_display
                .entry(display_name.to_string())
                .or_default();
            users.insert(user_id.to_string());
            users.len() > 1
        };

        if is_ambiguous {
            self.clear_avatar_for_display(display_name);
        }
    }

    /// 退出したユーザーを除外し、表示名に結び付いた未確定アバターも破棄する。
    pub fn on_player_left(&mut self, display_name: &str, user_id: &str) {
        let remove_display = if let Some(users) = self.active_users_by_display.get_mut(display_name)
        {
            users.remove(user_id);
            users.is_empty()
        } else {
            false
        };
        if remove_display {
            self.active_users_by_display.remove(display_name);
        }
        self.clear_avatar_for_display(display_name);
    }

    /// 表示名に対する最新のアバター切り替えを記録する。
    pub fn on_avatar_switch(&mut self, display_name: &str, avatar_name: &str) {
        self.clear_avatar_for_display(display_name);
        self.avatar_by_display
            .insert(display_name.to_string(), avatar_name.to_string());
        self.displays_by_avatar
            .entry(avatar_name.to_string())
            .or_default()
            .insert(display_name.to_string());
    }

    /// アバターダウンロード完了を記録する。
    ///
    /// 展開完了までに複数件あった場合は対象を特定できないため、状態値の有無にかかわらず
    /// そのまとまり全体を曖昧として扱う。
    pub fn on_avatar_download(&mut self, status: Option<FriendStatus>, line_number: usize) {
        self.pending_download = match self.pending_download {
            PendingDownload::Empty => PendingDownload::One {
                status,
                line_number,
            },
            PendingDownload::One { .. } | PendingDownload::Ambiguous => PendingDownload::Ambiguous,
        };
    }

    /// アバター展開完了を直前の一意なダウンロードへ対応付ける。
    ///
    /// 戻り値は、対象ユーザー ID と DB 保存値（0: 非フレンド、1: フレンド、
    /// 2: 判定不能）。展開行を受け取るたびに未確定ダウンロードは消費する。
    pub fn on_avatar_unpack(
        &mut self,
        unpack_payload: &str,
        line_number: usize,
    ) -> Option<(String, i64)> {
        let pending = std::mem::take(&mut self.pending_download);
        let PendingDownload::One {
            status: Some(status),
            line_number: download_line,
        } = pending
        else {
            return None;
        };
        if line_number < download_line
            || line_number.saturating_sub(download_line) > MAX_DOWNLOAD_UNPACK_LINE_GAP
        {
            return None;
        }

        let mut matching_avatars = self
            .displays_by_avatar
            .keys()
            .filter(|avatar| unpack_payload.starts_with(&format!("{avatar} by ")));
        let avatar_name = matching_avatars.next()?.clone();
        if matching_avatars.next().is_some() {
            return None;
        }

        let displays = self.displays_by_avatar.get(&avatar_name)?;
        if displays.len() != 1 {
            return None;
        }
        let display_name = displays.iter().next()?;
        let users = self.active_users_by_display.get(display_name)?;
        if users.len() != 1 {
            return None;
        }
        let user_id = users.iter().next()?.clone();

        let db_value = match self.observed_by_user.get(&user_id).copied() {
            None => {
                self.observed_by_user
                    .insert(user_id.clone(), ObservedStatus::Known(status));
                status.db_value()
            }
            Some(ObservedStatus::Known(previous)) if previous == status => status.db_value(),
            Some(ObservedStatus::Known(_)) => {
                self.observed_by_user
                    .insert(user_id.clone(), ObservedStatus::Conflict);
                UNKNOWN_FRIEND_STATUS
            }
            Some(ObservedStatus::Conflict) => UNKNOWN_FRIEND_STATUS,
        };

        Some((user_id, db_value))
    }

    fn clear_avatar_for_display(&mut self, display_name: &str) {
        let Some(previous_avatar) = self.avatar_by_display.remove(display_name) else {
            return;
        };
        let should_remove_avatar =
            if let Some(displays) = self.displays_by_avatar.get_mut(&previous_avatar) {
                displays.remove(display_name);
                displays.is_empty()
            } else {
                false
            };
        if should_remove_avatar {
            self.displays_by_avatar.remove(&previous_avatar);
        }
    }
}
