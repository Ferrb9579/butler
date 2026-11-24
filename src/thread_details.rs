//! Desktop thread workspace row and presentation from T3 Code v0.1.11.
use crate::{
    empty_workspace::MUTED,
    settings_icons::icon,
    typography,
    ui::{Button, ButtonStyle, ButtonVariant},
};
use incular::{
    core::SECONDARY_POINTER_BUTTON,
    prelude::*,
    widgets::{
        BackdropFilter, HitTestBehavior, Listener, MenuController, RawMenuAnchor,
        RawMenuAnchorGroup, RawTooltip, RawTooltipTriggerMode, TapRegionGroupId,
        TransientDismissPolicy, internal::ExplicitSemantics,
    },
};
use std::{
    cell::{Cell, RefCell},
    collections::BTreeMap,
    io,
    path::{Path, PathBuf},
    rc::Rc,
    sync::{Arc, OnceLock},
};
use uuid::Uuid;

const WIDTH: f32 = 280.0;
const EDGE: f32 = 12.0;
const CLEARANCE: f32 = 32.0;
const MINIMUM_CHAT: f32 = 640.0;
const MINIMUM_HEIGHT: f32 = 160.0;
const FIND_INSET: f32 = 48.0;
const POPOVER_VERT_PADDING: f32 = 8.0;
const POPOVER_WINDOW_RESERVE: f32 = 104.0;
const GLASS: Color = Color::rgba(255, 255, 255, 213);
const BORDER: Color = Color::rgba(24, 24, 27, 26);
pub const TOGGLE: &str = "Toggle thread details panel";

#[derive(Clone, Copy)]
enum CopyAction {
    WorkspacePath,
    BranchName,
}

impl CopyAction {
    fn label(self) -> &'static str {
        match self {
            Self::WorkspacePath => "Copy full path",
            Self::BranchName => "Copy branch name",
        }
    }

    fn key(self) -> &'static str {
        match self {
            Self::WorkspacePath => "workspace-path",
            Self::BranchName => "branch-name",
        }
    }
}

/// Connects row actions to their owning App window's real clipboard. No system
/// clipboard is opened or written while building the layout.
#[derive(Clone, Default)]
pub struct ClipboardWriter(Arc<OnceLock<UiDispatcher>>);

impl ClipboardWriter {
    pub fn bind(&self, dispatcher: UiDispatcher) {
        let _ = self.0.set(dispatcher);
    }

    fn write(&self, text: String) {
        let Some(dispatcher) = self.0.get() else {
            return;
        };
        dispatcher.dispatch(move |runtime| {
            if !runtime
                .write_clipboard(DataTransfer::plain_text(text))
                .is_ok_and(|report| report.written.contains(&TransferFormat::PlainText))
            {
                eprintln!("Butler could not copy the requested text to the clipboard.");
            }
        });
    }
}

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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    Inline,
    Popover,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Presentation {
    thread: Uuid,
    pub mode: Mode,
    revision: u64,
}

pub fn mode_for_canvas(width: f32, height: f32, find_open: bool) -> Mode {
    let padding = if width >= 720.0 { 48.0 } else { 16.0 };
    if inline_layout(width, height, padding, find_open).is_some() {
        Mode::Inline
    } else {
        Mode::Popover
    }
}

fn popover_box(anchor: Rect, overlay: Size, find_open: bool) -> Option<Rect> {
    // ThreadDetailsCard.tsx anchors below the entire header. The transparent
    // panel shell contributes 8px padding inside each edge; the painted
    // card is capped at 280px and the Find bar reserves another 48px.
    let width = WIDTH.min(anchor.size.width - EDGE * 2.0);
    let anchor_bottom = anchor.origin.y + anchor.size.height;
    let y = anchor_bottom + POPOVER_VERT_PADDING + if find_open { FIND_INSET } else { 0.0 };
    let height =
        (overlay.height - POPOVER_WINDOW_RESERVE).min(overlay.height - y - POPOVER_VERT_PADDING);
    if width <= 0.0 || width > overlay.width || height <= 0.0 {
        return None;
    }
    let x = (anchor.origin.x + anchor.size.width - EDGE - width).clamp(0.0, overlay.width - width);
    Some(Rect::from_origin_size(
        Offset::new(x, y),
        Size::new(width, height),
    ))
}

#[derive(Clone)]
pub struct Visibility {
    // A profile stores only exceptions to the default-open inline state.
    // Read an individual marker once when its thread is first shown so no
    // directory scan, project access or persistence mutation happens in layout.
    profile: Option<PathBuf>,
    hydrated: Rc<RefCell<BTreeMap<Uuid, bool>>>,
    overrides: Signal<BTreeMap<Uuid, bool>>,
    // A popup belongs to one contiguous presentation, not the thread's
    // durable inline setting. Rebuilding after resize or navigation expires
    // the previous token without a reactive write from inside layout.
    current: Rc<Cell<Option<Presentation>>>,
    revision: Rc<Cell<u64>>,
    popover: Signal<Option<Presentation>>,
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
            current: Rc::new(Cell::new(None)),
            revision: Rc::new(Cell::new(0)),
            popover: Signal::new(None),
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

    pub fn present(&self, selection: Option<(Uuid, Mode)>) -> Option<Presentation> {
        let current = self.current.get();
        if current.map(|value| (value.thread, value.mode)) == selection {
            return current;
        }
        let revision = self.revision.get().wrapping_add(1);
        self.revision.set(revision);
        let next = selection.map(|(thread, mode)| Presentation {
            thread,
            mode,
            revision,
        });
        self.current.set(next);
        next
    }

    pub fn popover_open(&self, presentation: Presentation) -> bool {
        presentation.mode == Mode::Popover
            && self.current.get() == Some(presentation)
            && self.popover.get() == Some(presentation)
    }

    fn toggle_presentation(&self, presentation: Presentation) {
        if self.current.get() != Some(presentation) {
            return;
        }
        if presentation.mode == Mode::Inline {
            self.toggle(presentation.thread);
        } else if self.popover_open(presentation) {
            self.close_popover(presentation);
        } else {
            self.details_focus.request_focus();
            self.popover.set(Some(presentation));
        }
    }

    fn close_popover(&self, presentation: Presentation) {
        if !self.popover_open(presentation) {
            return;
        }
        if self.details_focus.has_focus() {
            self.content_focus.request_focus();
        }
        self.popover.set(None);
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
    pub presentation: Presentation,
    pub popover_open: bool,
    pub inline_open: bool,
    pub clipboard: ClipboardWriter,
    pub branch: Option<String>,
}

impl LocalDetails {
    pub fn toggle(&self) {
        self.visibility.toggle_presentation(self.presentation);
    }

    pub fn toggle_button(&self) -> Widget {
        let open = match self.presentation.mode {
            Mode::Inline => self.inline_open,
            Mode::Popover => self.popover_open,
        };
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
        let button = Widget::from(
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
            .enable_tap_to_dismiss(false),
        );
        self.popover_region(button)
    }

    fn group(&self) -> TapRegionGroupId {
        TapRegionGroupId::named(format!("butler-thread-details-{}", self.thread))
    }

    pub fn popover_region(&self, child: Widget) -> Widget {
        if self.presentation.mode == Mode::Popover {
            TapRegion::new(child).group_id(self.group()).into()
        } else {
            child
        }
    }

    pub fn header(&self, header: Widget, find_open: bool) -> Widget {
        let detail = self.clone();
        let dismiss = self.visibility.clone();
        let presentation = self.presentation;
        OverlayPortal::new(header)
            .role(TransientRole::Popover)
            .show(self.popover_open)
            .dismiss_policy(TransientDismissPolicy {
                outside_pointer: false, // handled by the shared Find/trigger/card group
                escape: true,
                focus_loss: false, // generic FocusLost cannot exempt the sibling Find bar
                parent_deactivation: true,
            })
            .on_dismiss(move |_| dismiss.close_popover(presentation))
            .overlay_child_layout_builder(move |_, layout| {
                let Some(bounds) = popover_box(layout.anchor_rect, layout.overlay_size, find_open)
                else {
                    return SizedBox::shrink().into();
                };
                let close = detail.visibility.clone();
                let row = TapRegion::new(detail.card_with_height(bounds.size.height, true))
                    .group_id(detail.group())
                    .on_tap_outside(move |_| close.close_popover(presentation));
                Stack::new([Positioned::new(row)
                    .left(bounds.origin.x)
                    .top(bounds.origin.y)
                    .width(bounds.size.width)])
                .into()
            })
            .into()
    }

    pub fn card(&self) -> Widget {
        self.card_with_height(f32::INFINITY, false)
    }

    fn copy_menu(&self, row: Widget, action: CopyAction, value: String) -> Widget {
        let menu = MenuController::new();
        let close = menu.clone();
        let clipboard = self.clipboard.clone();
        let popup = RawMenuAnchor::new(menu, move |_, layout| {
            let margin = 4.0;
            let width = 144.0;
            let height = 36.0;
            let position = layout.position.unwrap_or(Offset::ZERO);
            let x = (layout.anchor_rect.origin.x + position.x).clamp(
                margin,
                (layout.overlay_size.width - width - margin).max(margin),
            );
            let y = (layout.anchor_rect.origin.y + position.y).clamp(
                margin,
                (layout.overlay_size.height - height - margin).max(margin),
            );
            let click = close.clone();
            let clipboard = clipboard.clone();
            let value = value.clone();
            let row = Button::new(action.label())
                .style(
                    ButtonStyle::new()
                        .variant(ButtonVariant::Ghost)
                        .fixed_size(Size::new(width - margin * 2.0, 28.0))
                        .alignment(Alignment::CENTER_LEFT)
                        .padding(EdgeInsets::symmetric(8.0, 0.0))
                        .border_radius(4.0)
                        .text_style(
                            typography::interface()
                                .font_size(14.0)
                                .line_height_absolute(20.0),
                        ),
                )
                .on_click(move || {
                    click.close();
                    clipboard.write(value.clone());
                });
            let panel = Container::new()
                .height(height)
                .width(width)
                .padding(EdgeInsets::all(margin))
                .decoration(
                    BoxDecoration::new()
                        .color(GLASS)
                        .border(Border::new(1.0, BORDER))
                        .border_radius(BorderRadius::circular(8.0)),
                )
                .child(row);
            let dismiss = close.clone();
            let panel = TapRegion::new(panel)
                .group_id(layout.tap_region_group_id.clone())
                .on_tap_outside(move |_| dismiss.close());
            Stack::new([Positioned::new(panel).left(x).top(y).width(width)]).into()
        })
        .child(row)
        .builder(|_, menu, row| {
            let open = menu.clone();
            Listener::new(row.expect("workspace row"))
                .behavior(HitTestBehavior::Opaque)
                .on_pointer_down(move |press| {
                    if press.button == Some(SECONDARY_POINTER_BUTTON) {
                        open.open(Some(press.local_position));
                    }
                })
                .into()
        });
        Widget::from(popup)
            .with_key(format!("{}-{}", action.key(), self.presentation.revision).as_str())
    }

    fn card_with_height(&self, max_height: f32, dialog: bool) -> Widget {
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
        // The pinned runtime dismisses every Escape-enabled portal together;
        // don't mount a nested menu that would close compact details too.
        // A non-UTF-8 path also has no exact plain-text encoding.
        let row = if !dialog && let Some(path) = self.path.to_str() {
            self.copy_menu(row, CopyAction::WorkspacePath, path.to_owned())
        } else {
            row
        };
        let (sections, content_height) = self.card_sections(row, !dialog);
        let body = Container::new()
            .width(WIDTH)
            .constraints(Constraints::new(0.0, WIDTH, 0.0, max_height))
            .decoration(
                BoxDecoration::new()
                    .color(GLASS)
                    .border(Border::new(1.0, BORDER))
                    .border_radius(BorderRadius::circular(16.0)),
            )
            .child(if dialog && max_height < content_height {
                Widget::from(SingleChildScrollView::new(sections))
            } else {
                sections
            });
        Focus::new(
            Widget::from(ClipRRect::new(16.0, BackdropFilter::blur(12.0, body))).semantics(
                ExplicitSemantics::new(if dialog {
                    SemanticRole::Dialog
                } else {
                    SemanticRole::GenericContainer
                })
                .label("Thread details"),
            ),
        )
        .node(self.visibility.details_focus.clone())
        .into()
    }

    fn card_sections(&self, workspace: Widget, contextual_copy: bool) -> (Widget, f32) {
        let section = |row: Widget| -> Widget {
            Container::new()
                .padding(EdgeInsets::from_ltrb(8.0, 8.0, 8.0, 10.0))
                .child(row)
                .into()
        };
        let mut sections = vec![section(workspace)];
        if let Some(branch) = &self.branch {
            let row = Self::branch_row(branch);
            let row = if contextual_copy {
                self.copy_menu(row, CopyAction::BranchName, branch.clone())
            } else {
                row
            };
            sections.push(
                Container::new()
                    .height(1.0)
                    .color(Color::rgba(24, 24, 27, 17))
                    .into(),
            );
            sections.push(section(row));
        }
        let content_height = if sections.len() == 1 { 52.0 } else { 103.0 };
        let content = Column::new(sections)
            .main_axis_size(MainAxisSize::Min)
            .cross_axis_alignment(CrossAxisAlignment::Stretch);
        let content = if contextual_copy {
            // Secondary-clicking the other row replaces, rather than stacks,
            // the first row's context menu.
            Widget::from(RawMenuAnchorGroup::new(MenuController::new(), content))
        } else {
            content.into()
        };
        (content, content_height)
    }

    fn branch_row(branch: &str) -> Widget {
        let caption = Text::new(branch)
            .style(
                typography::interface()
                    .font_size(13.0)
                    .font_weight(FontWeight::W500)
                    .line_height_absolute(18.0)
                    .color(Color::rgba(24, 24, 27, 204)),
            )
            .max_lines(Some(1))
            .overflow(TextOverflow::Ellipsis);
        Widget::from(
            Container::new()
                .height(32.0)
                .padding(EdgeInsets::symmetric(10.0, 0.0))
                .alignment(Alignment::CENTER_LEFT)
                .child(
                    Row::new([
                        icon("git-branch", 16.0, MUTED),
                        Expanded::new(caption).into(),
                    ])
                    .spacing(10.0)
                    .cross_axis_alignment(CrossAxisAlignment::Center),
                ),
        )
        .semantics(ExplicitSemantics::new(SemanticRole::Text).label(format!("Branch: {branch}")))
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
    #[test]
    fn popup_geometry_uses_full_header_and_does_not_inherit_inline_gap() {
        let overlay = Size::new(1000.0, 800.0);
        let header = Rect::from_origin_size(Offset::new(256.0, 0.0), Size::new(744.0, 40.0));
        let gap = popover_box(header, overlay, false).unwrap();
        assert_eq!(gap.origin, Offset::new(708.0, 48.0));
        assert_eq!(gap.size.width, 280.0);
        let with_find = popover_box(header, overlay, true).unwrap();
        assert_eq!(with_find.origin, Offset::new(708.0, 96.0));
        assert_eq!(with_find.size.width, 280.0);
        assert!(popover_box(header, Size::new(1000.0, 100.0), false).is_none());
    }

    #[test]
    fn popover_never_changes_persisted_inline_and_expires_when_its_view_changes() {
        let profile = ProfileFixture::new();
        let visibility = profile.visibility();
        let first = Uuid::new_v4();
        let second = Uuid::new_v4();
        visibility.toggle(first);
        let popup = visibility.present(Some((first, Mode::Popover))).unwrap();
        assert!(!visibility.popover_open(popup));
        visibility.toggle_presentation(popup);
        assert!(visibility.popover_open(popup));
        assert!(!profile.visibility().is_open(first));

        let inline = visibility.present(Some((first, Mode::Inline))).unwrap();
        assert!(!visibility.popover_open(inline));
        let narrow_again = visibility.present(Some((first, Mode::Popover))).unwrap();
        assert!(!visibility.popover_open(narrow_again));
        visibility.toggle_presentation(popup); // stale widget callback
        assert!(!visibility.popover_open(narrow_again));
        visibility.toggle_presentation(narrow_again);
        assert!(visibility.popover_open(narrow_again));

        let another = visibility.present(Some((second, Mode::Popover))).unwrap();
        assert!(!visibility.popover_open(another));
        assert!(profile.visibility().is_open(second));
        assert_eq!(std::fs::read_dir(&profile.0).unwrap().count(), 1);
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
