//! Read-only desktop inline workspace row from T3 Code v0.1.11.
use crate::{
    empty_workspace::MUTED,
    settings_icons::icon,
    typography,
    ui::{Button, ButtonStyle, ButtonVariant},
};
use incular::{
    prelude::*,
    widgets::{BackdropFilter, RawTooltip, RawTooltipTriggerMode, internal::ExplicitSemantics},
};
use std::{
    cell::RefCell,
    collections::BTreeMap,
    io,
    path::{Path, PathBuf},
    rc::Rc,
};
use uuid::Uuid;

const WIDTH: f32 = 280.0;
const EDGE: f32 = 12.0;
const CLEARANCE: f32 = 32.0;
const MINIMUM_CHAT: f32 = 640.0;
const MINIMUM_HEIGHT: f32 = 160.0;
const FIND_INSET: f32 = 48.0;
const GLASS: Color = Color::rgba(255, 255, 255, 213);
const BORDER: Color = Color::rgba(24, 24, 27, 26);
pub const TOGGLE: &str = "Toggle thread details panel";

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Layout {
    pub width: f32,
    pub right: f32,
    pub top: f32,
    pub lane_right: f32,
}

pub fn inline_layout(width: f32, height: f32, padding: f32, find_open: bool) -> Option<Layout> {
    let top = EDGE + if find_open { FIND_INSET } else { 0.0 };
    let lane_right = width - WIDTH - EDGE - CLEARANCE;
    (lane_right - padding >= MINIMUM_CHAT && height - top - EDGE >= MINIMUM_HEIGHT).then_some(
        Layout {
            width: WIDTH,
            right: EDGE,
            top,
            lane_right,
        },
    )
}

#[derive(Clone)]
pub struct Visibility {
    // A profile stores only exceptions to the default-open inline state.
    // Read an individual marker once when its thread is first shown so no
    // directory scan, project access or persistence mutation happens in layout.
    profile: Option<PathBuf>,
    hydrated: Rc<RefCell<BTreeMap<Uuid, bool>>>,
    overrides: Signal<BTreeMap<Uuid, bool>>,
    details_focus: FocusNode,
    content_focus: FocusNode,
}

impl Default for Visibility {
    fn default() -> Self {
        Self::new(None)
    }
}

impl Visibility {
    pub fn new(profile: Option<PathBuf>) -> Self {
        let content_focus = FocusNode::new();
        content_focus.set_skip_traversal(true);
        Self {
            profile,
            hydrated: Rc::new(RefCell::new(BTreeMap::new())),
            overrides: Signal::new(BTreeMap::new()),
            details_focus: FocusNode::new(),
            content_focus,
        }
    }

    pub fn is_open(&self, thread: Uuid) -> bool {
        if let Some(open) = self.overrides.get().get(&thread) {
            return *open;
        }
        *self.hydrated.borrow_mut().entry(thread).or_insert_with(|| {
            !self.profile.as_deref().is_some_and(|profile| {
                inline_closed(profile, &closed_marker(thread)).unwrap_or(false)
            })
        })
    }

    fn toggle(&self, thread: Uuid) {
        let open = !self.is_open(thread);
        if let Some(profile) = &self.profile
            && let Err(error) = save_closed(profile, &closed_marker(thread), !open)
        {
            // A state directory that cannot be written should not break the
            // working in-session toggle. Never write any project as a fallback.
            eprintln!("Butler could not save inline thread details: {error}");
        }
        if open {
            self.details_focus.request_focus();
        } else if self.details_focus.has_focus() {
            self.content_focus.request_focus();
        }
        let mut overrides = self.overrides.get();
        overrides.insert(thread, open);
        self.overrides.set(overrides);
    }

    pub fn content(&self, child: impl Into<Widget>) -> Widget {
        Focus::new(child).node(self.content_focus.clone()).into()
    }
}

fn closed_marker(thread: Uuid) -> String {
    format!("thread-details-inline-closed-{thread}")
}

fn invalid_marker() -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, "invalid inline details marker")
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
mod persistence {
    use super::*;
    use rustix::fs::{self, AtFlags, FileType, Mode, OFlags};
    use rustix::io::Errno;
    use std::fs::File;

    fn profile_directory(profile: &Path) -> io::Result<File> {
        // Bind relative marker operations to the same directory handle for
        // both metadata checks and mutations. The configured profile itself
        // must be a real directory, not a symlink substituted at its leaf.
        Ok(fs::open(
            profile,
            OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::empty(),
        )?
        .into())
    }

    fn has_marker(profile: &File, name: &str) -> io::Result<bool> {
        match fs::statat(profile, name, AtFlags::SYMLINK_NOFOLLOW) {
            Ok(stat)
                if FileType::from_raw_mode(stat.st_mode) == FileType::RegularFile
                    && stat.st_size == 0 =>
            {
                Ok(true)
            }
            Ok(_) => Err(invalid_marker()),
            Err(Errno::NOENT) => Ok(false),
            Err(error) => Err(error.into()),
        }
    }

    pub(super) fn inline_closed(profile: &Path, name: &str) -> io::Result<bool> {
        has_marker(&profile_directory(profile)?, name)
    }

    pub(super) fn save_closed(profile: &Path, name: &str, closed: bool) -> io::Result<()> {
        let profile = profile_directory(profile)?;
        if closed {
            // An exclusive, empty file commits the entire marker in one
            // operation; unrelated threads never share an overwritten record.
            match fs::openat(
                &profile,
                name,
                OFlags::WRONLY | OFlags::CREATE | OFlags::EXCL | OFlags::NOFOLLOW | OFlags::CLOEXEC,
                Mode::RUSR | Mode::WUSR,
            ) {
                Ok(marker) => fs::fsync(marker)?,
                Err(Errno::EXIST) if has_marker(&profile, name)? => return Ok(()),
                Err(error) => return Err(error.into()),
            }
        } else {
            if !has_marker(&profile, name)? {
                return Ok(());
            }
            match fs::unlinkat(&profile, name, AtFlags::empty()) {
                Ok(()) => {}
                Err(Errno::NOENT) => return Ok(()),
                Err(error) => return Err(error.into()),
            }
        }
        fs::fsync(profile)?;
        Ok(())
    }
}

#[cfg(not(any(target_os = "linux", target_os = "macos")))]
mod persistence {
    use super::*;
    use std::fs::{self, OpenOptions};

    fn path(profile: &Path, name: &str) -> io::Result<PathBuf> {
        if !fs::symlink_metadata(profile)?.is_dir() {
            return Err(io::Error::new(
                io::ErrorKind::NotADirectory,
                "invalid profile directory",
            ));
        }
        Ok(profile.join(name))
    }

    fn has_marker(marker: &Path) -> io::Result<bool> {
        match fs::symlink_metadata(marker) {
            Ok(metadata) if metadata.is_file() && metadata.len() == 0 => Ok(true),
            Ok(_) => Err(invalid_marker()),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(false),
            Err(error) => Err(error),
        }
    }

    pub(super) fn inline_closed(profile: &Path, name: &str) -> io::Result<bool> {
        has_marker(&path(profile, name)?)
    }

    pub(super) fn save_closed(profile: &Path, name: &str, closed: bool) -> io::Result<()> {
        let marker = path(profile, name)?;
        if closed {
            match OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&marker)
            {
                Ok(file) => file.sync_all(),
                Err(error)
                    if error.kind() == io::ErrorKind::AlreadyExists && has_marker(&marker)? =>
                {
                    Ok(())
                }
                Err(error) => Err(error),
            }
        } else if has_marker(&marker)? {
            match fs::remove_file(marker) {
                Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
                result => result,
            }
        } else {
            Ok(())
        }
    }
}

use persistence::{inline_closed, save_closed};

#[derive(Clone)]
pub struct LocalDetails {
    pub thread: Uuid,
    pub machine: String,
    pub path: PathBuf,
    pub visibility: Visibility,
    pub canvas_height: f32,
    pub inline_open: bool,
}

impl LocalDetails {
    pub fn toggle(&self) {
        self.visibility.toggle(self.thread);
    }

    pub fn toggle_button(&self) -> Widget {
        let open = self.inline_open;
        let next = self.clone();
        let control = Button::with_child(
            TOGGLE,
            icon("square-menu", 16.0, Color::rgba(24, 24, 27, 204)),
        )
        .style(
            ButtonStyle::new()
                .variant(ButtonVariant::Ghost)
                .fixed_size(Size::new(28.0, 28.0))
                .padding(EdgeInsets::ZERO)
                .background(if open {
                    Color::rgba(244, 244, 245, 255)
                } else {
                    Color::TRANSPARENT
                }),
        )
        .on_click(move || next.toggle());
        RawTooltip::new(
            "Toggle thread details",
            Widget::from(control).semantics(
                ExplicitSemantics::new(SemanticRole::Button)
                    .label(TOGGLE)
                    .state(SemanticState {
                        toggled: Some(open),
                        ..SemanticState::default()
                    }),
            ),
        )
        .trigger_mode(RawTooltipTriggerMode::Manual)
        .enable_tap_to_dismiss(false)
        .into()
    }

    pub fn card(&self) -> Widget {
        let path = self.path.to_string_lossy().into_owned();
        let row_text = Widget::from(
            Text::new(&self.machine)
                .style(
                    typography::interface()
                        .font_size(13.0)
                        .font_weight(FontWeight::W500)
                        .line_height_absolute(18.0)
                        .color(Color::rgba(24, 24, 27, 204)),
                )
                .max_lines(Some(1))
                .overflow(TextOverflow::Ellipsis),
        );
        let row = Container::new()
            .height(32.0)
            .padding(EdgeInsets::symmetric(10.0, 0.0))
            .alignment(Alignment::CENTER_LEFT)
            .child(
                Row::new([
                    icon("monitor", 16.0, MUTED),
                    Expanded::new(row_text).into(),
                    icon("folder", 16.0, MUTED),
                ])
                .spacing(10.0)
                .cross_axis_alignment(CrossAxisAlignment::Center),
            );
        let row = Widget::from(RawTooltip::new(&path, row)).semantics(
            ExplicitSemantics::new(SemanticRole::Text)
                .label(format!("Run context: {}, Local workspace", self.machine))
                .state(SemanticState {
                    tooltip: Some(path),
                    ..SemanticState::default()
                }),
        );
        let body = Container::new()
            .width(WIDTH)
            .padding(EdgeInsets::from_ltrb(8.0, 8.0, 8.0, 10.0))
            .decoration(
                BoxDecoration::new()
                    .color(GLASS)
                    .border(Border::new(1.0, BORDER))
                    .border_radius(BorderRadius::circular(16.0)),
            )
            .child(row);
        Focus::new(
            Widget::from(ClipRRect::new(16.0, BackdropFilter::blur(12.0, body))).semantics(
                ExplicitSemantics::new(SemanticRole::GenericContainer).label("Thread details"),
            ),
        )
        .node(self.visibility.details_focus.clone())
        .into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn desktop_layout_reserves_the_lane_and_moves_below_find() {
        let closed_find = inline_layout(1024.0, 760.0, 48.0, false).unwrap();
        assert_eq!(
            (closed_find.width, closed_find.right, closed_find.top),
            (280.0, 12.0, 12.0)
        );
        assert_eq!(closed_find.lane_right - 48.0, 652.0);
        let open_find = inline_layout(1024.0, 760.0, 48.0, true).unwrap();
        assert_eq!(open_find.top - closed_find.top, 48.0);
        assert!(inline_layout(1012.0, 184.0, 48.0, false).is_some());
        assert!(inline_layout(1011.0, 184.0, 48.0, false).is_none());
        assert!(inline_layout(1012.0, 183.0, 48.0, false).is_none());
        assert!(inline_layout(1012.0, 231.0, 48.0, true).is_none());
    }

    #[test]
    fn visibility_is_independent_per_thread() {
        let visibility = Visibility::default();
        let first = Uuid::new_v4();
        let second = Uuid::new_v4();
        assert!(visibility.is_open(first));
        visibility.toggle(first);
        assert!(!visibility.is_open(first));
        assert!(visibility.is_open(second));
        visibility.toggle(first);
        assert!(visibility.is_open(first));
    }
    struct ProfileFixture(PathBuf);

    impl ProfileFixture {
        fn new() -> Self {
            let root =
                std::env::temp_dir().join(format!("butler-inline-details-{}", Uuid::new_v4()));
            std::fs::create_dir(&root).unwrap();
            Self(root)
        }

        fn visibility(&self) -> Visibility {
            Visibility::new(Some(self.0.clone()))
        }
    }

    impl Drop for ProfileFixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn only_inline_closed_is_persisted_per_thread_and_profile() {
        let profile = ProfileFixture::new();
        let other = ProfileFixture::new();
        let first = Uuid::new_v4();
        let second = Uuid::new_v4();
        let visibility = profile.visibility();
        let concurrent = profile.visibility();

        assert!(visibility.is_open(first));
        assert!(visibility.is_open(second));
        assert_eq!(std::fs::read_dir(&profile.0).unwrap().count(), 0);
        visibility.toggle(first);
        concurrent.toggle(second);
        assert!(!visibility.is_open(first));
        assert!(!concurrent.is_open(second));
        assert!(other.visibility().is_open(first));

        let reopened = profile.visibility();
        assert!(!reopened.is_open(first));
        assert!(!reopened.is_open(second));
        reopened.toggle(first);
        assert!(reopened.is_open(first));

        let reopened_again = profile.visibility();
        assert!(reopened_again.is_open(first));
        assert!(!reopened_again.is_open(second));
        let entries = std::fs::read_dir(&profile.0)
            .unwrap()
            .map(|entry| entry.unwrap().file_name())
            .collect::<Vec<_>>();
        assert_eq!(entries, [std::ffi::OsString::from(closed_marker(second))]);
    }

    #[test]
    fn malformed_marker_and_missing_profile_cannot_erase_data_or_break_the_session_toggle() {
        let profile = ProfileFixture::new();
        let thread = Uuid::new_v4();
        let marker = profile.0.join(closed_marker(thread));
        std::fs::write(&marker, b"unrelated data").unwrap();
        let visibility = profile.visibility();
        assert!(visibility.is_open(thread));
        visibility.toggle(thread);
        assert!(!visibility.is_open(thread));
        visibility.toggle(thread);
        assert_eq!(std::fs::read(&marker).unwrap(), b"unrelated data");
        assert!(profile.visibility().is_open(thread));

        let absent = profile.0.join("missing");
        let memory_only = Visibility::new(Some(absent.clone()));
        memory_only.toggle(thread);
        assert!(!memory_only.is_open(thread));
        assert!(!absent.exists());
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    #[test]
    fn marker_or_profile_symlinks_are_not_followed() {
        let profile = ProfileFixture::new();
        let other = ProfileFixture::new();
        let thread = Uuid::new_v4();
        let marker = profile.0.join(closed_marker(thread));
        let outside = other.0.join("outside");
        std::fs::write(&outside, b"").unwrap();
        std::os::unix::fs::symlink(&outside, &marker).unwrap();
        let visibility = profile.visibility();
        assert!(
            visibility.is_open(thread),
            "a marker link is not a regular marker"
        );
        visibility.toggle(thread);
        visibility.toggle(thread);
        assert_eq!(std::fs::read(&outside).unwrap(), b"");
        assert!(std::fs::symlink_metadata(marker).unwrap().is_symlink());

        let linked_profile = profile.0.join("linked-profile");
        std::os::unix::fs::symlink(&other.0, &linked_profile).unwrap();
        let visibility = Visibility::new(Some(linked_profile));
        visibility.toggle(thread);
        assert!(!visibility.is_open(thread));
        assert!(!other.0.join(closed_marker(thread)).exists());
    }
}
