//! Owner-scoped project icon lookup. The immediate fallback never blocks a frame.
use incular::{image::ImageHandle, prelude::*};
use std::{
    cell::RefCell,
    path::{Path, PathBuf},
    rc::Rc,
};

#[derive(Default)]
struct State {
    request: Option<(PathBuf, u64)>,
    busy: bool,
    settled: bool,
    icon: Option<ImageHandle>,
}

#[derive(Clone)]
pub struct Lookup {
    inner: Rc<RefCell<State>>,
    changed: Signal<u64>,
}

impl Default for Lookup {
    fn default() -> Self {
        Self {
            inner: Default::default(),
            changed: Signal::new(0),
        }
    }
}

impl Lookup {
    /// Re-resolve the approved active project on startup or explicit project
    /// activation, never read discovery paths while building a widget.
    pub fn poll(
        &self,
        cx: &incular::runtime::BuildContext,
        root: Option<&Path>,
        revision: u64,
    ) -> Option<ImageHandle> {
        self.changed.get();
        let request = root.map(|root| (root.to_owned(), revision));
        let mut state = self.inner.borrow_mut();
        if state.request != request {
            state.request = request.clone();
            state.settled = false;
            state.icon = None;
        }
        if let Some(request) = request
            && !state.busy
            && !state.settled
        {
            state.busy = true;
            let root = request.0.clone();
            let lookup = self.clone();
            let _ = cx.spawn_blocking(
                move || crate::project_favicon::bitmap(&root),
                move |result, _| {
                    let mut state = lookup.inner.borrow_mut();
                    state.busy = false;
                    if state.request.as_ref() == Some(&request) {
                        state.icon = result.ok().flatten();
                        state.settled = true;
                    }
                    drop(state);
                    lookup.changed.set(lookup.changed.get().wrapping_add(1));
                },
            );
        }
        state.icon.clone()
    }
}

/// The image and the source's automatic monogram use the same decoration size.
pub fn project_mark(title: &str, favicon: Option<&ImageHandle>, size: f32) -> Widget {
    match favicon {
        Some(image) => Widget::from(
            Image::new(image.clone())
                .width(size)
                .height(size)
                .fit(BoxFit::Contain),
        )
        .exclude_semantics(),
        None => crate::project_identity::widget(title, size, false),
    }
}
