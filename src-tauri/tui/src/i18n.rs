use std::fmt;
use std::sync::OnceLock;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Lang {
    Zh,
    En,
    Ja,
}

impl Lang {
    pub fn from_config(value: &str) -> Self {
        match value {
            "en" => Self::En,
            "ja" => Self::Ja,
            _ => Self::Zh,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Key {
    Resolving,
    QueueFinished,
    AlreadyLoggedIn,
    FetchingQr,
    ScanQr,
    QrScannedConfirm,
    QrExpired,
    NetworkRetrying,
    SessionExpired,
    SearchPlaceholder,
    NowPlaying,
    Library,
    Search,
    Queue,
    QuitQuestion,
    Quit,
    Cancel,
    Play,
    Select,
    TopBottom,
    Back,
    JumpToTrack,
    ChangeTrack,
    RefreshQr,
    Pause,
    Seek,
    Volume,
    Zen,
    LoginTitle,
    LoginInstruction,
    NotLoggedInPressG,
    LikedSongs,
    DailyRecommendations,
    PersonalFm,
    CloudDrive,
    SyncingLibrary,
    EmptyLibrary,
    ColumnTitle,
    ModeSequential,
    ModeShuffle,
    ModeRepeatOne,
    Liked,
    Unliked,
    LikeFailed,
    ColumnArtist,
    ColumnDuration,
    EmptyQueue,
    Relogin,
    ScanLogin,
    NotLoggedInSync,
    #[allow(dead_code)] // logout action is wired in a later stage
    OpClearSession,
    OpQrKey,
    ApiQrKeyMissing,
    OpQrCheck,
    ApiLoginCookieMissing,
    OpPersistSession,
    OpAccount,
    ApiInvalidSession,
    OpUserPlaylist,
    ApiLikedPlaylistMissing,
    OpPlaylistTracks,
    OpSongUrl,
    ApiPlaybackUrlUnavailable,
    OpLyrics,
    OpSearch,
    OpFetchCover,
    OpReadCover,
    OpBuildQr,
}

static LANGUAGE: OnceLock<Lang> = OnceLock::new();

pub fn init(lang: Lang) {
    let _ = LANGUAGE.set(lang);
}

pub fn t(key: Key) -> &'static str {
    t_for(language(), key)
}

pub fn t_songs_ready(n: usize) -> String {
    songs_ready_for(language(), n)
}

pub fn t_welcome(name: &str) -> String {
    match language() {
        Lang::Zh => format!("欢迎，{name}"),
        Lang::En => format!("Welcome, {name}"),
        Lang::Ja => format!("ようこそ、{name}"),
    }
}

pub fn t_liked_songs_count(n: usize) -> String {
    match language() {
        Lang::Zh => format!("我喜欢的音乐 · {n} 首"),
        Lang::En => format!("Liked Songs · {n} tracks"),
        Lang::Ja => format!("お気に入り · {n}曲"),
    }
}

pub fn t_playing(kind: &str) -> String {
    match language() {
        Lang::Zh => format!("播放中 · {kind}"),
        Lang::En => format!("Playing · {kind}"),
        Lang::Ja => format!("再生中 · {kind}"),
    }
}

pub fn t_login_interrupted(error: impl fmt::Display) -> String {
    match language() {
        Lang::Zh => format!("网络不稳定，登录中断（{error}）；按 g 重试"),
        Lang::En => format!("Login interrupted by an unstable network ({error}); press g to retry"),
        Lang::Ja => format!("通信が不安定なためログインを中断しました（{error}）。gで再試行"),
    }
}

pub fn t_library_load_failed(error: impl fmt::Display) -> String {
    match language() {
        Lang::Zh => format!("歌单加载失败：{error}"),
        Lang::En => format!("Failed to load library: {error}"),
        Lang::Ja => format!("ライブラリを読み込めませんでした：{error}"),
    }
}

pub fn t_api_failed(operation: Key, error: impl fmt::Debug) -> String {
    let operation = t(operation);
    match language() {
        Lang::Zh => format!("{operation}失败：{error:?}"),
        Lang::En => format!("Failed to {operation}: {error:?}"),
        Lang::Ja => format!("{operation}に失敗しました：{error:?}"),
    }
}

pub fn t_unknown_qr_status(status: i64) -> String {
    match language() {
        Lang::Zh => format!("二维码状态码未知：{status}"),
        Lang::En => format!("Unknown QR code status: {status}"),
        Lang::Ja => format!("不明なQRコードステータス：{status}"),
    }
}

pub fn t_search_not_found(keywords: &str) -> String {
    match language() {
        Lang::Zh => format!("搜索不到「{keywords}」"),
        Lang::En => format!("No results for “{keywords}”"),
        Lang::Ja => format!("「{keywords}」が見つかりません"),
    }
}

pub fn t_candidates_unavailable(keywords: &str) -> String {
    match language() {
        Lang::Zh => format!("「{keywords}」的候选都拿不到播放地址（可能需要登录）"),
        Lang::En => format!("No playable result for “{keywords}” (sign-in may be required)"),
        Lang::Ja => format!("「{keywords}」を再生できません（ログインが必要な場合があります）"),
    }
}

fn language() -> Lang {
    LANGUAGE.get().copied().unwrap_or(Lang::Zh)
}

fn songs_ready_for(lang: Lang, n: usize) -> String {
    match lang {
        Lang::Zh => format!("{n} 首已就绪"),
        Lang::En => format!("{n} tracks ready"),
        Lang::Ja => format!("{n}曲を同期済み"),
    }
}

fn t_for(lang: Lang, key: Key) -> &'static str {
    match lang {
        Lang::Zh => match key {
            Key::Resolving => "解析中…",
            Key::QueueFinished => "队列播完了",
            Key::AlreadyLoggedIn => "已经登录了",
            Key::FetchingQr => "正在获取二维码…",
            Key::ScanQr => "用网易云音乐 App 扫码",
            Key::QrScannedConfirm => "已扫码，在手机上确认…",
            Key::QrExpired => "二维码已过期，按 g 重新获取",
            Key::NetworkRetrying => "网络抖动，重试中…",
            Key::SessionExpired => "登录态已失效，按 g 重新扫码",
            Key::SearchPlaceholder => "搜索（下一阶段接入）",
            Key::NowPlaying => "正在播放",
            Key::Library => "曲库",
            Key::Search => "搜索",
            Key::Queue => "队列",
            Key::QuitQuestion => "退出 ypm？",
            Key::Quit => "退出",
            Key::Cancel => "取消",
            Key::Play => "播放",
            Key::Select => "选择",
            Key::TopBottom => "顶/底",
            Key::Back => "返回",
            Key::JumpToTrack => "跳到这首",
            Key::ChangeTrack => "切歌",
            Key::RefreshQr => "刷新二维码",
            Key::Pause => "暂停",
            Key::Seek => "跳转",
            Key::Volume => "音量",
            Key::Zen => "纯净",
            Key::ModeSequential => "顺序",
            Key::ModeShuffle => "随机",
            Key::ModeRepeatOne => "单曲",
            Key::Liked => "已收藏",
            Key::Unliked => "已取消收藏",
            Key::LikeFailed => "收藏失败",
            Key::LoginTitle => "扫码登录网易云",
            Key::LoginInstruction => {
                "请用网易云音乐 App 里的「扫一扫」（系统相机扫会提示无效）"
            }
            Key::NotLoggedInPressG => "未登录 · 按 g",
            Key::LikedSongs => "我喜欢的音乐",
            Key::DailyRecommendations => "每日推荐",
            Key::PersonalFm => "私人FM",
            Key::CloudDrive => "云盘",
            Key::SyncingLibrary => "歌单同步中…",
            Key::EmptyLibrary => "这里还是空的",
            Key::ColumnTitle => "歌名",
            Key::ColumnArtist => "歌手",
            Key::ColumnDuration => "时长",
            Key::EmptyQueue => "队列是空的——去曲库按 Enter，整列表就会成为播放队列",
            Key::Relogin => "重新扫码登录",
            Key::ScanLogin => "扫码登录",
            Key::NotLoggedInSync => "未登录 · 扫码后同步你的音乐",
            Key::OpClearSession => "清除登录信息",
            Key::OpQrKey => "请求二维码密钥",
            Key::ApiQrKeyMissing => "二维码密钥响应缺少 unikey",
            Key::OpQrCheck => "检查二维码状态",
            Key::ApiLoginCookieMissing => "登录成功但响应里没有 MUSIC_U cookie",
            Key::OpPersistSession => "保存登录信息",
            Key::OpAccount => "获取账号信息",
            Key::ApiInvalidSession => "登录态无效（拿不到账号 id）",
            Key::OpUserPlaylist => "获取用户歌单",
            Key::ApiLikedPlaylistMissing => "没有找到「我喜欢的音乐」歌单",
            Key::OpPlaylistTracks => "获取歌单歌曲",
            Key::OpSongUrl => "获取播放地址",
            Key::ApiPlaybackUrlUnavailable => {
                "这首歌暂时拿不到播放地址（可能需要登录或 VIP）"
            }
            Key::OpLyrics => "获取歌词",
            Key::OpSearch => "搜索歌曲",
            Key::OpFetchCover => "下载封面",
            Key::OpReadCover => "读取封面数据",
            Key::OpBuildQr => "生成二维码",
        },
        Lang::En => match key {
            Key::Resolving => "Resolving…",
            Key::QueueFinished => "End of queue",
            Key::AlreadyLoggedIn => "Already signed in",
            Key::FetchingQr => "Getting QR code…",
            Key::ScanQr => "Scan with the NetEase Cloud Music app",
            Key::QrScannedConfirm => "Scanned—confirm on your phone…",
            Key::QrExpired => "QR code expired; press g for a new one",
            Key::NetworkRetrying => "Network hiccup—retrying…",
            Key::SessionExpired => "Session expired; press g to scan again",
            Key::SearchPlaceholder => "Search (coming next)",
            Key::NowPlaying => "Now Playing",
            Key::Library => "Library",
            Key::Search => "Search",
            Key::Queue => "Queue",
            Key::QuitQuestion => "Quit ypm?",
            Key::Quit => "Quit",
            Key::Cancel => "Cancel",
            Key::Play => "Play",
            Key::Select => "Select",
            Key::TopBottom => "Top/Bottom",
            Key::Back => "Back",
            Key::JumpToTrack => "Play this",
            Key::ChangeTrack => "Prev/Next",
            Key::RefreshQr => "Refresh QR",
            Key::Pause => "Pause",
            Key::Seek => "Seek",
            Key::Volume => "Volume",
            Key::Zen => "Zen",
            Key::ModeSequential => "Order",
            Key::ModeShuffle => "Shuffle",
            Key::ModeRepeatOne => "Repeat 1",
            Key::Liked => "Liked",
            Key::Unliked => "Removed from likes",
            Key::LikeFailed => "Like failed",
            Key::LoginTitle => "Sign in to NetEase Cloud Music",
            Key::LoginInstruction => {
                "Use Scan in the NetEase Cloud Music app (the camera app will not work)"
            }
            Key::NotLoggedInPressG => "Signed out · press g",
            Key::LikedSongs => "Liked Songs",
            Key::DailyRecommendations => "Daily Picks",
            Key::PersonalFm => "Personal FM",
            Key::CloudDrive => "Cloud Drive",
            Key::SyncingLibrary => "Syncing library…",
            Key::EmptyLibrary => "Nothing here yet",
            Key::ColumnTitle => "Title",
            Key::ColumnArtist => "Artist",
            Key::ColumnDuration => "Time",
            Key::EmptyQueue => "Queue is empty—press Enter in Library to queue the list",
            Key::Relogin => "Scan again",
            Key::ScanLogin => "Sign in with QR",
            Key::NotLoggedInSync => "Signed out · scan to sync your music",
            Key::OpClearSession => "clear sign-in data",
            Key::OpQrKey => "request a QR code key",
            Key::ApiQrKeyMissing => "QR code response is missing unikey",
            Key::OpQrCheck => "check QR code status",
            Key::ApiLoginCookieMissing => "Signed in, but MUSIC_U cookie is missing",
            Key::OpPersistSession => "save sign-in data",
            Key::OpAccount => "load account details",
            Key::ApiInvalidSession => "Invalid session (account id is unavailable)",
            Key::OpUserPlaylist => "load user playlists",
            Key::ApiLikedPlaylistMissing => "Liked Songs playlist not found",
            Key::OpPlaylistTracks => "load playlist tracks",
            Key::OpSongUrl => "get the playback URL",
            Key::ApiPlaybackUrlUnavailable => {
                "No playback URL is available (sign-in or VIP may be required)"
            }
            Key::OpLyrics => "load lyrics",
            Key::OpSearch => "search for songs",
            Key::OpFetchCover => "download cover art",
            Key::OpReadCover => "read cover data",
            Key::OpBuildQr => "build the QR code",
        },
        Lang::Ja => match key {
            Key::Resolving => "読み込み中…",
            Key::QueueFinished => "キューの再生が終了しました",
            Key::AlreadyLoggedIn => "ログイン済みです",
            Key::FetchingQr => "QRコードを取得中…",
            Key::ScanQr => "NetEase Cloud Musicアプリでスキャン",
            Key::QrScannedConfirm => "スキャン済みです。スマートフォンで確認してください…",
            Key::QrExpired => "QRコードの期限切れです。gで再取得",
            Key::NetworkRetrying => "通信が不安定です。再試行中…",
            Key::SessionExpired => "ログイン期限切れです。gで再スキャン",
            Key::SearchPlaceholder => "検索（次の段階で対応）",
            Key::NowPlaying => "再生中",
            Key::Library => "ライブラリ",
            Key::Search => "検索",
            Key::Queue => "キュー",
            Key::QuitQuestion => "ypmを終了しますか？",
            Key::Quit => "終了",
            Key::Cancel => "キャンセル",
            Key::Play => "再生",
            Key::Select => "選択",
            Key::TopBottom => "先頭/末尾",
            Key::Back => "戻る",
            Key::JumpToTrack => "この曲を再生",
            Key::ChangeTrack => "曲を切替",
            Key::RefreshQr => "QRを更新",
            Key::Pause => "一時停止",
            Key::Seek => "シーク",
            Key::Volume => "音量",
            Key::Zen => "集中表示",
            Key::ModeSequential => "順番",
            Key::ModeShuffle => "シャッフル",
            Key::ModeRepeatOne => "1曲リピート",
            Key::Liked => "お気に入りに追加",
            Key::Unliked => "お気に入り解除",
            Key::LikeFailed => "追加に失敗",
            Key::LoginTitle => "NetEase Cloud Musicにログイン",
            Key::LoginInstruction => {
                "NetEase Cloud Musicアプリのスキャン機能を使用してください（カメラアプリは使用不可）"
            }
            Key::NotLoggedInPressG => "未ログイン · gを押す",
            Key::LikedSongs => "お気に入り",
            Key::DailyRecommendations => "デイリーレコメンド",
            Key::PersonalFm => "パーソナルFM",
            Key::CloudDrive => "クラウド",
            Key::SyncingLibrary => "同期中…",
            Key::EmptyLibrary => "まだ何もありません",
            Key::ColumnTitle => "曲名",
            Key::ColumnArtist => "アーティスト",
            Key::ColumnDuration => "時間",
            Key::EmptyQueue => "キューは空です。ライブラリでEnterを押すと一覧を追加できます",
            Key::Relogin => "もう一度スキャン",
            Key::ScanLogin => "QRコードでログイン",
            Key::NotLoggedInSync => "未ログイン · スキャンして音楽を同期",
            Key::OpClearSession => "ログイン情報の削除",
            Key::OpQrKey => "QRコードキーの取得",
            Key::ApiQrKeyMissing => "QRコードの応答にunikeyがありません",
            Key::OpQrCheck => "QRコード状態の確認",
            Key::ApiLoginCookieMissing => "ログイン成功後の応答にMUSIC_U cookieがありません",
            Key::OpPersistSession => "ログイン情報の保存",
            Key::OpAccount => "アカウント情報の取得",
            Key::ApiInvalidSession => "ログイン情報が無効です（アカウントIDを取得できません）",
            Key::OpUserPlaylist => "ユーザープレイリストの取得",
            Key::ApiLikedPlaylistMissing => "お気に入りプレイリストが見つかりません",
            Key::OpPlaylistTracks => "プレイリスト曲の取得",
            Key::OpSongUrl => "再生URLの取得",
            Key::ApiPlaybackUrlUnavailable => {
                "再生URLを取得できません（ログインまたはVIPが必要な場合があります）"
            }
            Key::OpLyrics => "歌詞の取得",
            Key::OpSearch => "曲の検索",
            Key::OpFetchCover => "カバー画像のダウンロード",
            Key::OpReadCover => "カバー画像データの読み込み",
            Key::OpBuildQr => "QRコードの生成",
        },
    }
}

#[cfg(test)]
mod tests {
    use super::{init, songs_ready_for, t, t_for, t_songs_ready, Key, Lang};

    #[test]
    fn comparable_keys_are_complete_and_distinct_in_all_languages() {
        for key in [Key::Quit, Key::LikedSongs, Key::SyncingLibrary] {
            let translations = [
                t_for(Lang::Zh, key),
                t_for(Lang::En, key),
                t_for(Lang::Ja, key),
            ];
            assert!(translations.iter().all(|text| !text.is_empty()));
            assert_ne!(translations[0], translations[1]);
            assert_ne!(translations[0], translations[2]);
            assert_ne!(translations[1], translations[2]);
        }
    }

    #[test]
    fn language_config_falls_back_to_chinese() {
        assert_eq!(Lang::from_config("zh"), Lang::Zh);
        assert_eq!(Lang::from_config("en"), Lang::En);
        assert_eq!(Lang::from_config("ja"), Lang::Ja);
        assert_eq!(Lang::from_config("fr"), Lang::Zh);
    }

    #[test]
    fn parameterized_song_counts_follow_each_language() {
        assert_eq!(songs_ready_for(Lang::Zh, 12), "12 首已就绪");
        assert_eq!(songs_ready_for(Lang::En, 12), "12 tracks ready");
        assert_eq!(songs_ready_for(Lang::Ja, 12), "12曲を同期済み");
    }

    #[test]
    fn global_init_drives_public_translation_functions() {
        init(Lang::En);
        assert_eq!(t(Key::Quit), "Quit");
        assert_eq!(t_songs_ready(3), "3 tracks ready");
    }
}
