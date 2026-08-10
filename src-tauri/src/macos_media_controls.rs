use std::{cell::RefCell, ffi::CStr, path::Path};

use objc2::{
    define_class, msg_send,
    rc::Retained,
    runtime::{AnyClass, AnyObject, ClassBuilder, NSObjectProtocol, Sel},
    sel, AnyThread, DefinedClass, MainThreadOnly,
};
use objc2_app_kit::{
    NSApplication, NSButtonTouchBarItem, NSImage, NSMenu, NSMenuItem, NSTouchBar, NSTouchBarItem,
    NSTouchBarItemIdentifierFlexibleSpace, NSWindow,
};
use objc2_foundation::{MainThreadMarker, NSArray, NSObject, NSSet, NSString};
use serde::Deserialize;
use tauri::{AppHandle, Emitter, Manager};

const DOCK_DELEGATE_CLASS: &CStr = c"YPMDockMenuApplicationDelegate";
const TOUCH_BAR_ASSET_DIR: &str = "renderer/img/touchbar";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum MediaAction {
    RouteBack,
    RouteForward,
    Search,
    Previous,
    Play,
    Next,
    Like,
    NextUp,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ActionPayload {
    None,
    Text(&'static str),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct PlayerState {
    playing: bool,
    liked: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PlayerStatePayload {
    playing: bool,
    liked_current_track: bool,
}

fn action_event(action: MediaAction) -> (&'static str, ActionPayload) {
    match action {
        MediaAction::RouteBack => ("routerGo", ActionPayload::Text("back")),
        MediaAction::RouteForward => ("routerGo", ActionPayload::Text("forward")),
        MediaAction::Search => ("search", ActionPayload::None),
        MediaAction::Previous => ("previous", ActionPayload::None),
        MediaAction::Play => ("play", ActionPayload::None),
        MediaAction::Next => ("next", ActionPayload::None),
        MediaAction::Like => ("like", ActionPayload::None),
        MediaAction::NextUp => ("nextUp", ActionPayload::None),
    }
}

fn action_asset(action: MediaAction) -> &'static str {
    match action {
        MediaAction::RouteBack => "page_prev.png",
        MediaAction::RouteForward => "page_next.png",
        MediaAction::Search => "search.png",
        MediaAction::Previous => "backward.png",
        MediaAction::Play => "play.png",
        MediaAction::Next => "forward.png",
        MediaAction::Like => "like.png",
        MediaAction::NextUp => "next_up.png",
    }
}

fn action_identifier(action: MediaAction) -> &'static str {
    match action {
        MediaAction::RouteBack => "route-back",
        MediaAction::RouteForward => "route-forward",
        MediaAction::Search => "search",
        MediaAction::Previous => "previous",
        MediaAction::Play => "play",
        MediaAction::Next => "next",
        MediaAction::Like => "like",
        MediaAction::NextUp => "next-up",
    }
}

fn state_assets(state: PlayerState) -> (&'static str, &'static str) {
    (
        if state.playing {
            "pause.png"
        } else {
            "play.png"
        },
        if state.liked {
            "like_fill.png"
        } else {
            "like.png"
        },
    )
}

fn emit_action(app: &AppHandle, action: MediaAction) {
    let (channel, payload) = action_event(action);
    let event = format!("desktop://{channel}");
    let result = match payload {
        ActionPayload::None => app.emit(&event, ()),
        ActionPayload::Text(value) => app.emit(&event, value),
    };
    if let Err(error) = result {
        eprintln!("[tauri] failed to emit {event}: {error}");
    }
}

struct MediaControlTargetIvars {
    app: AppHandle,
}

define_class!(
    #[unsafe(super = NSObject)]
    #[thread_kind = MainThreadOnly]
    #[ivars = MediaControlTargetIvars]
    struct MediaControlTarget;

    unsafe impl NSObjectProtocol for MediaControlTarget {}

    impl MediaControlTarget {
        #[unsafe(method(routeBack:))]
        fn route_back(&self, _sender: &AnyObject) {
            emit_action(&self.ivars().app, MediaAction::RouteBack);
        }

        #[unsafe(method(routeForward:))]
        fn route_forward(&self, _sender: &AnyObject) {
            emit_action(&self.ivars().app, MediaAction::RouteForward);
        }

        #[unsafe(method(search:))]
        fn search(&self, _sender: &AnyObject) {
            emit_action(&self.ivars().app, MediaAction::Search);
        }

        #[unsafe(method(previous:))]
        fn previous(&self, _sender: &AnyObject) {
            emit_action(&self.ivars().app, MediaAction::Previous);
        }

        #[unsafe(method(play:))]
        fn play(&self, _sender: &AnyObject) {
            emit_action(&self.ivars().app, MediaAction::Play);
        }

        #[unsafe(method(next:))]
        fn next(&self, _sender: &AnyObject) {
            emit_action(&self.ivars().app, MediaAction::Next);
        }

        #[unsafe(method(like:))]
        fn like(&self, _sender: &AnyObject) {
            emit_action(&self.ivars().app, MediaAction::Like);
        }

        #[unsafe(method(nextUp:))]
        fn next_up(&self, _sender: &AnyObject) {
            emit_action(&self.ivars().app, MediaAction::NextUp);
        }
    }
);

impl MediaControlTarget {
    fn new(app: AppHandle, mtm: MainThreadMarker) -> Retained<Self> {
        let this = Self::alloc(mtm).set_ivars(MediaControlTargetIvars { app });
        // SAFETY: NSObject's initializer has no extra requirements.
        unsafe { msg_send![super(this), init] }
    }
}

struct PlayerImages {
    play: Retained<NSImage>,
    pause: Retained<NSImage>,
    like: Retained<NSImage>,
    like_fill: Retained<NSImage>,
}

type TouchBarComponents = (
    Retained<NSTouchBar>,
    Retained<NSButtonTouchBarItem>,
    Retained<NSButtonTouchBarItem>,
    PlayerImages,
);

struct InstalledControls {
    _target: Retained<MediaControlTarget>,
    touch_bar: Retained<NSTouchBar>,
    dock_menu: Retained<NSMenu>,
    play_button: Retained<NSButtonTouchBarItem>,
    like_button: Retained<NSButtonTouchBarItem>,
    images: PlayerImages,
}

impl InstalledControls {
    fn apply_player_state(&self, state: PlayerState) {
        let (play_asset, like_asset) = state_assets(state);
        let play_image = match play_asset {
            "pause.png" => &self.images.pause,
            _ => &self.images.play,
        };
        let like_image = match like_asset {
            "like_fill.png" => &self.images.like_fill,
            _ => &self.images.like,
        };
        self.play_button.setImage(Some(play_image));
        self.like_button.setImage(Some(like_image));
    }
}

thread_local! {
    static CONTROLS: RefCell<Option<InstalledControls>> = const { RefCell::new(None) };
}

fn selector_for(action: MediaAction) -> Sel {
    match action {
        MediaAction::RouteBack => sel!(routeBack:),
        MediaAction::RouteForward => sel!(routeForward:),
        MediaAction::Search => sel!(search:),
        MediaAction::Previous => sel!(previous:),
        MediaAction::Play => sel!(play:),
        MediaAction::Next => sel!(next:),
        MediaAction::Like => sel!(like:),
        MediaAction::NextUp => sel!(nextUp:),
    }
}

fn load_image(
    asset_dir: &Path,
    file_name: &str,
    description: &str,
) -> Result<Retained<NSImage>, String> {
    let path = asset_dir.join(file_name);
    let path = path
        .to_str()
        .ok_or_else(|| format!("invalid Touch Bar asset path: {}", path.display()))?;
    let image = NSImage::initWithContentsOfFile(NSImage::alloc(), &NSString::from_str(path))
        .ok_or_else(|| format!("failed to load Touch Bar asset: {path}"))?;
    image.setTemplate(true);
    image.setAccessibilityDescription(Some(&NSString::from_str(description)));
    Ok(image)
}

fn touch_bar_button(
    target: &MediaControlTarget,
    action: MediaAction,
    image: &NSImage,
    mtm: MainThreadMarker,
) -> Retained<NSButtonTouchBarItem> {
    let identifier = NSString::from_str(&format!(
        "com.yesplaymusic.touchbar.{}",
        action_identifier(action)
    ));
    // SAFETY: Every selector is implemented by MediaControlTarget.
    unsafe {
        NSButtonTouchBarItem::buttonTouchBarItemWithIdentifier_image_target_action(
            &identifier,
            image,
            Some(target),
            Some(selector_for(action)),
            mtm,
        )
    }
}

fn create_touch_bar(
    target: &MediaControlTarget,
    asset_dir: &Path,
    mtm: MainThreadMarker,
) -> Result<TouchBarComponents, String> {
    let actions = [
        MediaAction::RouteBack,
        MediaAction::RouteForward,
        MediaAction::Search,
        MediaAction::Previous,
        MediaAction::Play,
        MediaAction::Next,
        MediaAction::Like,
        MediaAction::NextUp,
    ];
    let descriptions = [
        "Back",
        "Forward",
        "Search",
        "Previous track",
        "Play or pause",
        "Next track",
        "Like",
        "Up next",
    ];
    let mut buttons = Vec::with_capacity(actions.len());
    for (action, description) in actions.into_iter().zip(descriptions) {
        let image = load_image(asset_dir, action_asset(action), description)?;
        buttons.push(touch_bar_button(target, action, &image, mtm));
    }

    let play_button = buttons[4].clone();
    let like_button = buttons[6].clone();
    let images = PlayerImages {
        play: load_image(asset_dir, "play.png", "Play")?,
        pause: load_image(asset_dir, "pause.png", "Pause")?,
        like: load_image(asset_dir, "like.png", "Like")?,
        like_fill: load_image(asset_dir, "like_fill.png", "Unlike")?,
    };

    let identifiers: Vec<Retained<NSString>> = actions
        .into_iter()
        .map(|action| {
            NSString::from_str(&format!(
                "com.yesplaymusic.touchbar.{}",
                action_identifier(action)
            ))
        })
        .collect();
    // SAFETY: AppKit owns this process-wide identifier.
    let flexible_space = unsafe { NSTouchBarItemIdentifierFlexibleSpace };
    let default_identifiers = NSArray::from_slice(&[
        &*identifiers[0],
        &*identifiers[1],
        &*identifiers[2],
        flexible_space,
        &*identifiers[3],
        &*identifiers[4],
        &*identifiers[5],
        flexible_space,
        &*identifiers[6],
        &*identifiers[7],
    ]);
    let template_items: Vec<Retained<NSTouchBarItem>> =
        buttons.iter().cloned().map(Retained::into_super).collect();
    let template_items = NSSet::from_retained_slice(&template_items);
    let touch_bar = NSTouchBar::new(mtm);
    touch_bar.setDefaultItemIdentifiers(&default_identifiers);
    touch_bar.setTemplateItems(&template_items);
    Ok((touch_bar, play_button, like_button, images))
}

fn dock_menu_item(
    title: &str,
    target: &MediaControlTarget,
    action: MediaAction,
    mtm: MainThreadMarker,
) -> Retained<NSMenuItem> {
    // SAFETY: Every selector is implemented by MediaControlTarget.
    let item = unsafe {
        NSMenuItem::initWithTitle_action_keyEquivalent(
            NSMenuItem::alloc(mtm),
            &NSString::from_str(title),
            Some(selector_for(action)),
            &NSString::from_str(""),
        )
    };
    // SAFETY: The retained target outlives the menu.
    unsafe { item.setTarget(Some(target)) };
    item
}

fn create_dock_menu(target: &MediaControlTarget, mtm: MainThreadMarker) -> Retained<NSMenu> {
    let menu = NSMenu::initWithTitle(NSMenu::alloc(mtm), &NSString::from_str("Playback"));
    menu.addItem(&dock_menu_item("Play", target, MediaAction::Play, mtm));
    menu.addItem(&NSMenuItem::separatorItem(mtm));
    menu.addItem(&dock_menu_item("Next", target, MediaAction::Next, mtm));
    menu.addItem(&dock_menu_item(
        "Previous",
        target,
        MediaAction::Previous,
        mtm,
    ));
    menu
}

unsafe extern "C-unwind" fn application_dock_menu(
    _delegate: &AnyObject,
    _selector: Sel,
    _application: &NSApplication,
) -> *mut NSMenu {
    CONTROLS.with(|controls| {
        controls
            .borrow()
            .as_ref()
            .map(|controls| Retained::as_ptr(&controls.dock_menu).cast_mut())
            .unwrap_or(std::ptr::null_mut())
    })
}

fn dock_delegate_subclass(
    name: &CStr,
    superclass: &'static AnyClass,
) -> Result<&'static AnyClass, String> {
    if let Some(existing) = AnyClass::get(name) {
        if existing.superclass() != Some(superclass) {
            return Err("Dock menu delegate class has an unexpected superclass".to_string());
        }
        return Ok(existing);
    }
    let mut builder = ClassBuilder::new(name, superclass)
        .ok_or_else(|| "failed to create Dock menu delegate class".to_string())?;
    // SAFETY: The callback matches applicationDockMenu:'s ABI.
    unsafe {
        builder.add_method(
            sel!(applicationDockMenu:),
            application_dock_menu as unsafe extern "C-unwind" fn(_, _, _) -> _,
        );
    }
    Ok(builder.register())
}

fn install_dock_menu_provider(mtm: MainThreadMarker) -> Result<(), String> {
    let application = NSApplication::sharedApplication(mtm);
    let delegate = application
        .delegate()
        .ok_or_else(|| "NSApplication has no delegate".to_string())?;
    let delegate: &AnyObject = delegate.as_ref();
    if delegate.class().name() == DOCK_DELEGATE_CLASS {
        return Ok(());
    }

    let subclass = dock_delegate_subclass(DOCK_DELEGATE_CLASS, delegate.class())?;
    // SAFETY: The subclass adds one method and preserves the existing delegate layout.
    unsafe { AnyObject::set_class(delegate, subclass) };
    Ok(())
}

pub fn install(app: &AppHandle) -> Result<(), String> {
    let mtm = MainThreadMarker::new()
        .ok_or_else(|| "macOS media controls must be installed on the main thread".to_string())?;
    let window = app
        .get_webview_window("main")
        .ok_or_else(|| "main window is unavailable".to_string())?;
    let ns_window = window.ns_window().map_err(|error| error.to_string())?;
    // SAFETY: Tauri owns this NSWindow for the lifetime of the webview window.
    let ns_window = unsafe { &*ns_window.cast::<NSWindow>() };

    if CONTROLS.with(|controls| {
        if let Some(controls) = controls.borrow().as_ref() {
            ns_window.setTouchBar(Some(&controls.touch_bar));
            true
        } else {
            false
        }
    }) {
        return Ok(());
    }

    let target = MediaControlTarget::new(app.clone(), mtm);
    let asset_dir = app
        .path()
        .resource_dir()
        .map_err(|error| error.to_string())?
        .join(TOUCH_BAR_ASSET_DIR);
    let (touch_bar, play_button, like_button, images) = create_touch_bar(&target, &asset_dir, mtm)?;
    let dock_menu = create_dock_menu(&target, mtm);
    ns_window.setTouchBar(Some(&touch_bar));
    CONTROLS.with(|controls| {
        controls.replace(Some(InstalledControls {
            _target: target,
            touch_bar,
            dock_menu,
            play_button,
            like_button,
            images,
        }));
    });
    install_dock_menu_provider(mtm)
}

pub fn update_player_state(app: &AppHandle, payload: serde_json::Value) -> Result<(), String> {
    let payload: PlayerStatePayload =
        serde_json::from_value(payload).map_err(|error| error.to_string())?;
    let state = PlayerState {
        playing: payload.playing,
        liked: payload.liked_current_track,
    };
    app.run_on_main_thread(move || {
        CONTROLS.with(|controls| {
            if let Some(controls) = controls.borrow().as_ref() {
                controls.apply_player_state(state);
            }
        });
    })
    .map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use objc2::ClassType;

    #[test]
    fn actions_map_to_renderer_events() {
        assert_eq!(
            action_event(MediaAction::RouteBack),
            ("routerGo", ActionPayload::Text("back"))
        );
        assert_eq!(
            action_event(MediaAction::RouteForward),
            ("routerGo", ActionPayload::Text("forward"))
        );
        assert_eq!(
            [
                MediaAction::Search,
                MediaAction::Previous,
                MediaAction::Play,
                MediaAction::Next,
                MediaAction::Like,
                MediaAction::NextUp,
            ]
            .map(action_event),
            [
                ("search", ActionPayload::None),
                ("previous", ActionPayload::None),
                ("play", ActionPayload::None),
                ("next", ActionPayload::None),
                ("like", ActionPayload::None),
                ("nextUp", ActionPayload::None),
            ]
        );
    }

    #[test]
    fn actions_map_to_existing_touch_bar_assets() {
        assert_eq!(action_asset(MediaAction::RouteBack), "page_prev.png");
        assert_eq!(action_asset(MediaAction::RouteForward), "page_next.png");
        assert_eq!(action_asset(MediaAction::Search), "search.png");
        assert_eq!(action_asset(MediaAction::Previous), "backward.png");
        assert_eq!(action_asset(MediaAction::Play), "play.png");
        assert_eq!(action_asset(MediaAction::Next), "forward.png");
        assert_eq!(action_asset(MediaAction::Like), "like.png");
        assert_eq!(action_asset(MediaAction::NextUp), "next_up.png");
    }

    #[test]
    fn touch_bar_identifiers_are_unique() {
        let identifiers = [
            MediaAction::RouteBack,
            MediaAction::RouteForward,
            MediaAction::Search,
            MediaAction::Previous,
            MediaAction::Play,
            MediaAction::Next,
            MediaAction::Like,
            MediaAction::NextUp,
        ]
        .map(action_identifier);
        let unique = identifiers
            .into_iter()
            .collect::<std::collections::HashSet<_>>();
        assert_eq!(unique.len(), identifiers.len());
    }

    #[test]
    fn player_state_selects_playback_and_like_assets() {
        assert_eq!(
            state_assets(PlayerState {
                playing: false,
                liked: false,
            }),
            ("play.png", "like.png")
        );
        assert_eq!(
            state_assets(PlayerState {
                playing: true,
                liked: true,
            }),
            ("pause.png", "like_fill.png")
        );
    }

    #[test]
    fn dock_menu_callback_uses_object_return_encoding() {
        let class =
            dock_delegate_subclass(c"YPMDockMenuApplicationDelegateTest", NSObject::class())
                .expect("register test delegate");
        let method = class
            .instance_method(sel!(applicationDockMenu:))
            .expect("Dock menu callback");
        assert_eq!(&*method.return_type(), c"@");
        assert_eq!(method.arguments_count(), 3);
    }
}
