//! VRChat ログ行解析用のコンパイル済み正規表現パターンとヘルパー。
//!
//! 全パターンは `LazyLock` で一度だけコンパイルし、全インポート呼び出しで再利用する。
//! ハードコードされた不正パターンは即座にプロセスを中断する。
//! 壊れた正規表現を黙ってスキップするとパースデータが破損するため。

use regex::Regex;
use std::sync::LazyLock;

/// `VRChat` ログパーサーで使用する正規表現を1つコンパイルする。
///
/// ハードコードパターンが不正な場合はパーサーを中断する。壊れた正規表現で
/// 処理を続けると後続のパース結果が黙って破損するため。
///
/// # 引数
/// * `pattern` - コンパイルする正規表現ソース。
/// * `name` - パニックメッセージ用のパターン名。
///
/// # 戻り値
/// コンパイル済み正規表現。
#[allow(clippy::panic)]
fn compile_regex(pattern: &str, name: &str) -> Regex {
    match Regex::new(pattern) {
        Ok(regex) => regex,
        Err(err) => {
            panic!("固定正規表現が壊れています [{name}]: {err}");
        }
    }
}

/// ログ行先頭のタイムスタンプにマッチする。
pub static RE_TIME: LazyLock<Regex> =
    LazyLock::new(|| compile_regex(r"^(\d{4}\.\d{2}\.\d{2} \d{2}:\d{2}:\d{2})", "RE_TIME"));

/// セッションごとに1回出現するユーザー認証行にマッチする。
/// 例: `User Authenticated: Name (usr_xxxx)`
pub static RE_USER_AUTH: LazyLock<Regex> = LazyLock::new(|| {
    compile_regex(
        r"User Authenticated: (.*?) \((usr_[a-f0-9\-]+)\)",
        "RE_USER_AUTH",
    )
});

/// 参加行の直前に出現するワールド名行にマッチする。
/// 例: `[Behaviour] Entering Room: World Name`
pub static RE_ENTERING: LazyLock<Regex> =
    LazyLock::new(|| compile_regex(r"\[Behaviour\] Entering Room: (.*)", "RE_ENTERING"));

/// ワールド参加行にマッチする。
/// 例: `[Behaviour] Joining wrld_xxx:74156~private(usr_xxx)~region(jp)`
/// キャプチャ1は `world_id`、2は `instance_number`、3は `access_raw`、4は `region`。
pub static RE_JOINING: LazyLock<Regex> = LazyLock::new(|| {
    compile_regex(
        r"\[Behaviour\] Joining (wrld_[^:]+)(?::(\d+))?~?((?:private|friends|hidden|public|group)[^~]*)(?:~region\(([^)]+)\))?",
        "RE_JOINING",
    )
});

/// 退室マーカーにマッチする。
pub static RE_LEFT_ROOM: LazyLock<Regex> =
    LazyLock::new(|| compile_regex(r"\[Behaviour\] OnLeftRoom", "RE_LEFT_ROOM"));

/// プレイヤー参加行にマッチする。
/// 例: `[Behaviour] OnPlayerJoined Name (usr_xxxx)`
pub static RE_PLAYER_JOIN: LazyLock<Regex> = LazyLock::new(|| {
    compile_regex(
        r"\[Behaviour\] OnPlayerJoined (.*?) \((usr_[a-f0-9\-]+)\)",
        "RE_PLAYER_JOIN",
    )
});

/// プレイヤー退出行にマッチする。
pub static RE_PLAYER_LEFT: LazyLock<Regex> = LazyLock::new(|| {
    compile_regex(
        r"\[Behaviour\] OnPlayerLeft (.*?) \((usr_[a-f0-9\-]+)\)",
        "RE_PLAYER_LEFT",
    )
});

/// ワールド入室後に出力されるローカルプレイヤー分類行にマッチする。
/// 例: `[Behaviour] Initialized PlayerAPI "Name" is local`
pub static RE_IS_LOCAL: LazyLock<Regex> = LazyLock::new(|| {
    compile_regex(
        r#"\[Behaviour\] Initialized PlayerAPI "(.*?)" is (local|remote)"#,
        "RE_IS_LOCAL",
    )
});

/// 受信通知行にマッチする。
/// 例: `Received Notification: <Notification from username:Name, sender user id:usr_xxx ...>`
/// キャプチャ1は `sender_username`、2は `sender_user_id`、3は `notif_type`、4は `notif_id`、
/// 5は `created_at`、6はメッセージ本文。
pub static RE_NOTIFICATION: LazyLock<Regex> = LazyLock::new(|| {
    compile_regex(
        r#"Received Notification: <Notification from username:([^,]*), sender user id:([^ ]*) to [^ ]+ of type: ([^,]+), id: (not_[a-f0-9\-]+), created at: ([^,]+),[^>]*message: "([^"]*)"\s*>"#,
        "RE_NOTIFICATION",
    )
});

/// 通知の JSON 風ペイロードから `worldId` 値を抽出する。
pub static RE_NOTIFICATION_WORLD_ID: LazyLock<Regex> =
    LazyLock::new(|| compile_regex(r"worldId=(wrld_[^,}]+)", "RE_NOTIFICATION_WORLD_ID"));

/// 通知の JSON 風ペイロードから `worldName` 値を抽出する。
pub static RE_NOTIFICATION_WORLD_NAME: LazyLock<Regex> =
    LazyLock::new(|| compile_regex(r"worldName=([^,}]+)", "RE_NOTIFICATION_WORLD_NAME"));

/// プレイヤーのロード完了を示す「OnPlayerJoinComplete」行にマッチする。
pub static RE_PLAYER_JOIN_COMPLETE: LazyLock<Regex> = LazyLock::new(|| {
    compile_regex(
        r"\[Behaviour\] OnPlayerJoinComplete (.+)",
        "RE_PLAYER_JOIN_COMPLETE",
    )
});

/// セッション開始時に出力される VRChat+ サブスクリプション状態行にマッチする。
pub static RE_SUBSCRIPTION_STATUS: LazyLock<Regex> = LazyLock::new(|| {
    compile_regex(
        r"Get VRChat Subscription Details! Subscription Id:([^ ]*) active:(True|False) desc:(.*)",
        "RE_SUBSCRIPTION_STATUS",
    )
});

/// VRC Camera が出力するスクリーンショット撮影行にマッチする。
/// 例: `[VRC Camera] Took screenshot to: C:\...\VRChat_2025-10-21_00-59-15.520_3840x2160.png`
/// キャプチャ1は `file_path`、2は `width`、3は `height`。
pub static RE_SCREENSHOT: LazyLock<Regex> = LazyLock::new(|| {
    compile_regex(
        r"\[VRC Camera\] Took screenshot to: (.+_(\d+)x(\d+)\.\w+)",
        "RE_SCREENSHOT",
    )
});

/// OSC サービス検出行にマッチする。
/// 例: `Found new OSC Service: OyasumiVR at 127.0.0.1:61080`
/// キャプチャ1は `service_name`、2は `ip_address`、3は `port`。
pub static RE_OSC_FOUND: LazyLock<Regex> = LazyLock::new(|| {
    compile_regex(
        r"Found new OSC Service: (.+?) at ([\d.]+):(\d+)",
        "RE_OSC_FOUND",
    )
});


/// 括弧で囲まれた汎用 `usr_...` 識別子にマッチする。
pub static RE_USR: LazyLock<Regex> = LazyLock::new(|| compile_regex(r"\((usr_[^)]+)\)", "RE_USR"));

/// 生のインスタンスアクセスサフィックスを正規化されたアクセスメタデータに変換する。
///
/// # 引数
/// * `access_raw` - 参加ログ行から抽出した生のアクセスセグメント。
///
/// # 戻り値
/// 正規化されたアクセスタイプとオプションのインスタンスオーナーユーザー ID のタプル。
pub fn parse_access_type(access_raw: &str) -> (Option<String>, Option<String>) {
    let lower = access_raw.to_lowercase();
    let access_type = if lower.starts_with("private") {
        Some("private".to_string())
    } else if lower.starts_with("friends") {
        Some("friends".to_string())
    } else if lower.starts_with("hidden") {
        Some("hidden".to_string())
    } else if lower.starts_with("public") {
        Some("public".to_string())
    } else if lower.starts_with("group") {
        Some("group".to_string())
    } else {
        None
    };

    let instance_owner = RE_USR
        .captures(access_raw)
        .and_then(|c| c.get(1))
        .map(|m| m.as_str().to_string());

    (access_type, instance_owner)
}

/// 通知タイプを永続化すべきか判定する。
///
/// 認識された通知タイプのみ収集する。
///
/// # 引数
/// * `notif_type` - ログから解析された通知タイプ。
///
/// # 戻り値
/// 通知を収集すべき場合は `true`。
pub fn is_collectible_notification(notif_type: &str) -> bool {
    matches!(
        notif_type.trim(),
        "boop" | "friendRequest" | "requestInvite" | "invite" | "group"
    )
}

/// `VRChat` ロケーション文字列から解析された構造化ロケーションメタデータ。
#[derive(Clone, Default)]
pub struct ParsedLocation {
    pub instance_id: Option<String>,
    pub access_type: Option<String>,
    pub instance_owner: Option<String>,
    pub region: Option<String>,
}

/// `VRChat` ロケーション文字列をワールド・インスタンス・リージョンに分解する。
///
/// # 引数
/// * `location` - ログに出力された生のロケーション文字列。
///
/// # 戻り値
/// `StellaRecord` が移動・ワールド訪問テーブルに格納する正規化フィールドを持つ
/// `ParsedLocation`。
pub fn parse_location(location: &str) -> ParsedLocation {
    let trimmed = location.trim();
    if !trimmed.starts_with("wrld_") {
        return ParsedLocation::default();
    }
    let Some((_world_id, tail)) = trimmed.split_once(':') else {
        return ParsedLocation::default();
    };

    let mut result = ParsedLocation::default();

    let mut parts = tail.split('~');
    if let Some(instance_id) = parts.next() {
        result.instance_id = Some(instance_id.to_string());
    }

    if let Some(access_raw) = parts.next() {
        let (access_type, instance_owner) = parse_access_type(access_raw);
        result.access_type = access_type;
        result.instance_owner = instance_owner;
    }

    for part in parts {
        if let Some(region) = part
            .strip_prefix("region(")
            .and_then(|value| value.strip_suffix(')'))
        {
            result.region = Some(region.to_string());
        }
    }

    result
}
