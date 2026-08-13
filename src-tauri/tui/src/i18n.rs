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
#[allow(clippy::enum_variant_names)] // Op*/Api* prefixes group the key space deliberately
pub enum Key {
    Resolving,
    QueueFinished,
    FetchingQr,
    ScanQr,
    QrScannedConfirm,
    QrExpired,
    NetworkRetrying,
    SessionExpired,
    NowPlaying,
    Library,
    Search,
    Queue,
    Settings,
    SettingTheme,
    SettingLanguage,
    SettingQuality,
    SettingCoverMode,
    SettingLayout,
    SettingProgressStyle,
    SettingPixelDetail,
    SettingQueueBehavior,
    SettingIcons,
    SettingQueueList,
    SettingQueueSingle,
    SettingAdjust,
    SettingsHint,
    SettingsSaved,
    SettingsSaveFailed,
    Save,
    QuitQuestion,
    Quit,
    Cancel,
    Play,
    Select,
    TopBottom,
    Back,
    JumpToTrack,
    ChangeTrack,
    Page,
    Filter,
    ClearFilter,
    FinishFilter,
    AddToQueue,
    AddedToQueue,
    Mute,
    Shuffle,
    Repeat,
    LibraryFocus,
    Pause,
    Seek,
    Volume,
    Zen,
    LoginTitle,
    LoginInstruction,
    NotLoggedInMenu,
    LikedSongs,
    DailyRecommendations,
    PersonalFm,
    CloudDrive,
    SyncingLibrary,
    EmptyLibrary,
    ColumnTitle,
    SearchPrompt,
    HelpTitle,
    HelpAnyKey,
    LabelLike,
    LabelHelp,
    Searching,
    SearchFailed,
    NoResults,
    TypeToSearch,
    Liked,
    Unliked,
    LikeFailed,
    ColumnArtist,
    ColumnDuration,
    EmptyQueue,
    Relogin,
    ScanLogin,
    NotLoggedInSync,
    OpQrKey,
    ApiQrKeyMissing,
    OpQrCheck,
    ApiLoginCookieMissing,
    OpPersistSession,
    OpAccount,
    ApiInvalidSession,
    OpUserPlaylist,
    ApiLikedPlaylistMissing,
    ApiLibraryPayloadMissing,
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
        Lang::Zh => format!("网络不稳定，登录中断（{error}）；请从主菜单重试"),
        Lang::En => format!("Login interrupted by an unstable network ({error}); retry from the main menu"),
        Lang::Ja => format!("通信が不安定なためログインを中断しました（{error}）。メインメニューから再試行してください"),
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
            Key::FetchingQr => "正在获取二维码…",
            Key::ScanQr => "用网易云音乐 App 扫码",
            Key::QrScannedConfirm => "已扫码，在手机上确认…",
            Key::QrExpired => "二维码已过期，请从主菜单重新扫码",
            Key::NetworkRetrying => "网络抖动，重试中…",
            Key::SessionExpired => "登录态已失效，请从主菜单重新扫码",
            Key::NowPlaying => "正在播放",
            Key::Library => "曲库",
            Key::Search => "搜索",
            Key::Queue => "队列",
            Key::Settings => "设置",
            Key::SettingTheme => "主题",
            Key::SettingLanguage => "语言（重启后）",
            Key::SettingQuality => "音质",
            Key::SettingCoverMode => "封面模式（重启后）",
            Key::SettingLayout => "播放布局",
            Key::SettingProgressStyle => "进度条",
            Key::SettingPixelDetail => "像素细节",
            Key::SettingQueueBehavior => "Enter 行为",
            Key::SettingIcons => "图标",
            Key::SettingQueueList => "整列入队",
            Key::SettingQueueSingle => "只播单曲",
            Key::SettingAdjust => "调整",
            Key::SettingsHint => "j/k 选择 · h/l 即时预览 · Enter 保存 · Esc 取消",
            Key::SettingsSaved => "设置已保存",
            Key::SettingsSaveFailed => "设置保存失败",
            Key::Save => "保存",
            Key::QuitQuestion => "退出 ypm？",
            Key::Quit => "退出",
            Key::Cancel => "取消",
            Key::Play => "播放",
            Key::Select => "选择",
            Key::TopBottom => "顶/底",
            Key::Back => "返回",
            Key::JumpToTrack => "跳到这首",
            Key::ChangeTrack => "切歌",
            Key::Page => "翻页",
            Key::Filter => "过滤当前列表",
            Key::ClearFilter => "清除过滤",
            Key::FinishFilter => "完成输入",
            Key::AddToQueue => "加入队列",
            Key::AddedToQueue => "已加入队列",
            Key::Mute => "静音",
            Key::Shuffle => "随机开 / 关",
            Key::Repeat => "列表 / 单曲 / 关",
            Key::LibraryFocus => "侧栏 / 列表",
            Key::Pause => "暂停",
            Key::Seek => "快退 / 快进",
            Key::Volume => "音量",
            Key::Zen => "纯净",
            Key::SearchPrompt => "搜索网易云曲库",
            Key::HelpTitle => "快捷键",
            Key::HelpAnyKey => "按任意键关闭",
            Key::LabelLike => "收藏 / 取消收藏",
            Key::LabelHelp => "帮助",
            Key::Searching => "搜索中…",
            Key::SearchFailed => "搜索失败，请稍后重试",
            Key::NoResults => "没有找到相关歌曲",
            Key::TypeToSearch => "输入关键词，Enter 搜索",
            Key::Liked => "已收藏",
            Key::Unliked => "已取消收藏",
            Key::LikeFailed => "收藏失败",
            Key::LoginTitle => "扫码登录网易云",
            Key::LoginInstruction => {
                "请用网易云音乐 App 里的「扫一扫」（系统相机扫会提示无效）"
            }
            Key::NotLoggedInMenu => "未登录 · 从主菜单扫码",
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
            Key::OpQrKey => "请求二维码密钥",
            Key::ApiQrKeyMissing => "二维码密钥响应缺少 unikey",
            Key::OpQrCheck => "检查二维码状态",
            Key::ApiLoginCookieMissing => "登录成功但响应里没有 MUSIC_U cookie",
            Key::OpPersistSession => "保存登录信息",
            Key::OpAccount => "获取账号信息",
            Key::ApiInvalidSession => "登录态无效（拿不到账号 id）",
            Key::OpUserPlaylist => "获取用户歌单",
            Key::ApiLikedPlaylistMissing => "没有找到「我喜欢的音乐」歌单",
            Key::ApiLibraryPayloadMissing => "服务端没有返回有效的曲库数据",
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
            Key::FetchingQr => "Getting QR code…",
            Key::ScanQr => "Scan with the NetEase Cloud Music app",
            Key::QrScannedConfirm => "Scanned—confirm on your phone…",
            Key::QrExpired => "QR code expired; scan again from the main menu",
            Key::NetworkRetrying => "Network hiccup—retrying…",
            Key::SessionExpired => "Session expired; scan again from the main menu",
            Key::NowPlaying => "Now Playing",
            Key::Library => "Library",
            Key::Search => "Search",
            Key::Queue => "Queue",
            Key::Settings => "Settings",
            Key::SettingTheme => "Theme",
            Key::SettingLanguage => "Language (restart)",
            Key::SettingQuality => "Audio quality",
            Key::SettingCoverMode => "Cover mode (restart)",
            Key::SettingLayout => "Player layout",
            Key::SettingProgressStyle => "Progress style",
            Key::SettingPixelDetail => "Pixel detail",
            Key::SettingQueueBehavior => "Enter behavior",
            Key::SettingIcons => "Icons",
            Key::SettingQueueList => "Queue the list",
            Key::SettingQueueSingle => "Play one track",
            Key::SettingAdjust => "Adjust",
            Key::SettingsHint => "j/k select · h/l preview · Enter save · Esc cancel",
            Key::SettingsSaved => "Settings saved",
            Key::SettingsSaveFailed => "Could not save settings",
            Key::Save => "Save",
            Key::QuitQuestion => "Quit ypm?",
            Key::Quit => "Quit",
            Key::Cancel => "Cancel",
            Key::Play => "Play",
            Key::Select => "Select",
            Key::TopBottom => "Top/Bottom",
            Key::Back => "Back",
            Key::JumpToTrack => "Play this",
            Key::ChangeTrack => "Prev/Next",
            Key::Page => "Page",
            Key::Filter => "Filter this list",
            Key::ClearFilter => "Clear filter",
            Key::FinishFilter => "Finish filter",
            Key::AddToQueue => "Add to queue",
            Key::AddedToQueue => "Added to queue",
            Key::Mute => "Mute",
            Key::Shuffle => "Shuffle on / off",
            Key::Repeat => "List / one / off",
            Key::LibraryFocus => "Sidebar / list",
            Key::Pause => "Pause",
            Key::Seek => "Seek back / forward",
            Key::Volume => "Volume",
            Key::Zen => "Zen",
            Key::SearchPrompt => "Search NCM",
            Key::HelpTitle => "Keyboard",
            Key::HelpAnyKey => "Press any key to close",
            Key::LabelLike => "Like / unlike",
            Key::LabelHelp => "Help",
            Key::Searching => "Searching…",
            Key::SearchFailed => "Search failed. Try again later",
            Key::NoResults => "No songs found",
            Key::TypeToSearch => "Type keywords, Enter to search",
            Key::Liked => "Liked",
            Key::Unliked => "Removed from likes",
            Key::LikeFailed => "Like failed",
            Key::LoginTitle => "Sign in to NetEase Cloud Music",
            Key::LoginInstruction => {
                "Use Scan in the NetEase Cloud Music app (the camera app will not work)"
            }
            Key::NotLoggedInMenu => "Signed out · scan from the main menu",
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
            Key::OpQrKey => "request a QR code key",
            Key::ApiQrKeyMissing => "QR code response is missing unikey",
            Key::OpQrCheck => "check QR code status",
            Key::ApiLoginCookieMissing => "Signed in, but MUSIC_U cookie is missing",
            Key::OpPersistSession => "save sign-in data",
            Key::OpAccount => "load account details",
            Key::ApiInvalidSession => "Invalid session (account id is unavailable)",
            Key::OpUserPlaylist => "load user playlists",
            Key::ApiLikedPlaylistMissing => "Liked Songs playlist not found",
            Key::ApiLibraryPayloadMissing => "The server returned no valid library data",
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
            Key::FetchingQr => "QRコードを取得中…",
            Key::ScanQr => "NetEase Cloud Musicアプリでスキャン",
            Key::QrScannedConfirm => "スキャン済みです。スマートフォンで確認してください…",
            Key::QrExpired => "QRコードの期限切れです。メインメニューから再スキャンしてください",
            Key::NetworkRetrying => "通信が不安定です。再試行中…",
            Key::SessionExpired => "ログイン期限切れです。メインメニューから再スキャンしてください",
            Key::NowPlaying => "再生中",
            Key::Library => "ライブラリ",
            Key::Search => "検索",
            Key::Queue => "キュー",
            Key::Settings => "設定",
            Key::SettingTheme => "テーマ",
            Key::SettingLanguage => "言語（再起動後）",
            Key::SettingQuality => "音質",
            Key::SettingCoverMode => "カバーモード（再起動後）",
            Key::SettingLayout => "再生レイアウト",
            Key::SettingProgressStyle => "進行表示",
            Key::SettingPixelDetail => "ピクセル詳細",
            Key::SettingQueueBehavior => "Enter の動作",
            Key::SettingIcons => "アイコン",
            Key::SettingQueueList => "一覧をキューへ",
            Key::SettingQueueSingle => "1曲だけ再生",
            Key::SettingAdjust => "調整",
            Key::SettingsHint => "j/k 選択 · h/l プレビュー · Enter 保存 · Esc キャンセル",
            Key::SettingsSaved => "設定を保存しました",
            Key::SettingsSaveFailed => "設定を保存できませんでした",
            Key::Save => "保存",
            Key::QuitQuestion => "ypmを終了しますか？",
            Key::Quit => "終了",
            Key::Cancel => "キャンセル",
            Key::Play => "再生",
            Key::Select => "選択",
            Key::TopBottom => "先頭/末尾",
            Key::Back => "戻る",
            Key::JumpToTrack => "この曲を再生",
            Key::ChangeTrack => "曲を切替",
            Key::Page => "ページ移動",
            Key::Filter => "現在の一覧を絞り込む",
            Key::ClearFilter => "絞り込みを解除",
            Key::FinishFilter => "入力を完了",
            Key::AddToQueue => "キューに追加",
            Key::AddedToQueue => "キューに追加しました",
            Key::Mute => "ミュート",
            Key::Shuffle => "シャッフル オン / オフ",
            Key::Repeat => "リスト / 1曲 / オフ",
            Key::LibraryFocus => "サイドバー / 一覧",
            Key::Pause => "一時停止",
            Key::Seek => "前後にシーク",
            Key::Volume => "音量",
            Key::Zen => "集中表示",
            Key::SearchPrompt => "検索",
            Key::HelpTitle => "キー操作",
            Key::HelpAnyKey => "任意のキーで閉じる",
            Key::LabelLike => "お気に入り切替",
            Key::LabelHelp => "ヘルプ",
            Key::Searching => "検索中…",
            Key::SearchFailed => "検索できませんでした。後でもう一度お試しください",
            Key::NoResults => "見つかりませんでした",
            Key::TypeToSearch => "キーワードを入力して Enter",
            Key::Liked => "お気に入りに追加",
            Key::Unliked => "お気に入り解除",
            Key::LikeFailed => "追加に失敗",
            Key::LoginTitle => "NetEase Cloud Musicにログイン",
            Key::LoginInstruction => {
                "NetEase Cloud Musicアプリのスキャン機能を使用してください（カメラアプリは使用不可）"
            }
            Key::NotLoggedInMenu => "未ログイン · メインメニューからスキャン",
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
            Key::OpQrKey => "QRコードキーの取得",
            Key::ApiQrKeyMissing => "QRコードの応答にunikeyがありません",
            Key::OpQrCheck => "QRコード状態の確認",
            Key::ApiLoginCookieMissing => "ログイン成功後の応答にMUSIC_U cookieがありません",
            Key::OpPersistSession => "ログイン情報の保存",
            Key::OpAccount => "アカウント情報の取得",
            Key::ApiInvalidSession => "ログイン情報が無効です（アカウントIDを取得できません）",
            Key::OpUserPlaylist => "ユーザープレイリストの取得",
            Key::ApiLikedPlaylistMissing => "お気に入りプレイリストが見つかりません",
            Key::ApiLibraryPayloadMissing => "サーバーから有効なライブラリデータが返されませんでした",
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
    use super::{init, songs_ready_for, t, t_for, t_login_interrupted, t_songs_ready, Key, Lang};

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
    fn login_retry_messages_point_back_to_the_main_menu() {
        for (lang, marker) in [
            (Lang::Zh, "主菜单"),
            (Lang::En, "main menu"),
            (Lang::Ja, "メインメニュー"),
        ] {
            for message in [
                t_for(lang, Key::NotLoggedInMenu).to_owned(),
                t_for(lang, Key::QrExpired).to_owned(),
                t_for(lang, Key::SessionExpired).to_owned(),
            ] {
                assert!(message.contains(marker), "{lang:?}: {message}");
            }
        }
        let interrupted = t_login_interrupted("offline");
        assert!(["主菜单", "main menu", "メインメニュー"]
            .iter()
            .any(|marker| interrupted.contains(marker)));
    }

    #[test]
    fn global_init_drives_public_translation_functions() {
        init(Lang::En);
        assert_eq!(t(Key::Quit), "Quit");
        assert_eq!(t_songs_ready(3), "3 tracks ready");
    }
}
