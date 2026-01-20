// File: ./src/gui/view/focusable.rs
use iced::advanced::layout;
use iced::advanced::renderer;
use iced::advanced::widget::{self, Operation, Tree};
use iced::advanced::{Clipboard, Layout, Shell, Widget};
use iced::event::Event;
use iced::mouse;
use iced::{Element, Length, Rectangle, Size, Vector};
use once_cell::sync::Lazy;
use std::collections::HashMap;
use std::sync::Mutex;

/// Global registry mapping a textual representation of a widget `Id` -> its last-known bounds.
/// We store the `Id` as a `String` (via Debug) because the concrete `Id` type may not be hashable
/// in all versions; a string key is stable and sufficient for lookup here.
static FOCUS_BOUNDS: Lazy<Mutex<HashMap<String, Rectangle>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));

/// Register the bounds for a focusable widget id.
pub fn register_focus_bounds(id: &widget::Id, rect: Rectangle) {
    let key = format!("{:?}", id);
    let mut map = FOCUS_BOUNDS.lock().unwrap();
    map.insert(key, rect);
}

/// Retrieve the last registered bounds for a focusable widget id.
pub fn get_focus_bounds(id: &widget::Id) -> Option<Rectangle> {
    let key = format!("{:?}", id);
    let map = FOCUS_BOUNDS.lock().unwrap();
    map.get(&key).cloned()
}

/// Retrieve a clone of the entire focus bounds registry.
/// This allows callers to inspect all known focusable widget bounds at once.
///
/// Note: returns a shallow clone of the internal HashMap. The Rectangle values are copied.
pub fn get_all_focus_bounds() -> HashMap<String, Rectangle> {
    let map = FOCUS_BOUNDS.lock().unwrap();
    map.clone()
}

pub struct Focusable<'a, Message, Theme, Renderer> {
    content: Element<'a, Message, Theme, Renderer>,
    id: Option<widget::Id>,
}

impl<'a, Message, Theme, Renderer> Focusable<'a, Message, Theme, Renderer> {
    pub fn new(content: impl Into<Element<'a, Message, Theme, Renderer>>) -> Self {
        Self {
            content: content.into(),
            id: None,
        }
    }

    pub fn id(mut self, id: impl Into<widget::Id>) -> Self {
        self.id = Some(id.into());
        self
    }
}

pub fn focusable<'a, Message, Theme, Renderer>(
    content: impl Into<Element<'a, Message, Theme, Renderer>>,
) -> Focusable<'a, Message, Theme, Renderer>
where
    Renderer: iced::advanced::Renderer,
{
    Focusable::new(content)
}

impl<'a, Message, Theme, Renderer> Widget<Message, Theme, Renderer>
    for Focusable<'a, Message, Theme, Renderer>
where
    Renderer: iced::advanced::Renderer,
{
    fn size(&self) -> Size<Length> {
        self.content.as_widget().size()
    }

    fn layout(
        &mut self,
        tree: &mut Tree,
        renderer: &Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        self.content
            .as_widget_mut()
            .layout(&mut tree.children[0], renderer, limits)
    }

    fn draw(
        &self,
        tree: &Tree,
        renderer: &mut Renderer,
        theme: &Theme,
        style: &renderer::Style,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
    ) {
        self.content.as_widget().draw(
            &tree.children[0],
            renderer,
            theme,
            style,
            layout,
            cursor,
            viewport,
        )
    }

    fn children(&self) -> Vec<Tree> {
        vec![Tree::new(&self.content)]
    }

    fn diff(&self, tree: &mut Tree) {
        tree.diff_children(std::slice::from_ref(&self.content))
    }

    fn operate(
        &mut self,
        tree: &mut Tree,
        layout: Layout<'_>,
        renderer: &Renderer,
        operation: &mut dyn Operation,
    ) {
        if let Some(id) = &self.id {
            let state = tree.state.downcast_mut::<State>();

            // Register the bounds for this focusable so other code can inspect them later
            // (e.g. for testing, diagnostics, or advanced bring-into-view heuristics).
            register_focus_bounds(id, layout.bounds());

            // Signature based on compiler error: (id, bounds, state)
            // Arg 1: Option<&Id>
            // Arg 2: Rectangle
            // Arg 3: &mut dyn Focusable
            operation.focusable(Some(id), layout.bounds(), state);
        }

        self.content
            .as_widget_mut()
            .operate(&mut tree.children[0], layout, renderer, operation);
    }

    fn tag(&self) -> widget::tree::Tag {
        widget::tree::Tag::of::<State>()
    }

    fn state(&self) -> widget::tree::State {
        widget::tree::State::new(State::default())
    }

    fn update(
        &mut self,
        tree: &mut Tree,
        event: &Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        renderer: &Renderer,
        clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, Message>,
        viewport: &Rectangle,
    ) {
        self.content.as_widget_mut().update(
            &mut tree.children[0],
            event,
            layout,
            cursor,
            renderer,
            clipboard,
            shell,
            viewport,
        )
    }

    fn mouse_interaction(
        &self,
        tree: &Tree,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
        renderer: &Renderer,
    ) -> mouse::Interaction {
        self.content.as_widget().mouse_interaction(
            &tree.children[0],
            layout,
            cursor,
            viewport,
            renderer,
        )
    }

    fn overlay<'b>(
        &'b mut self,
        tree: &'b mut Tree,
        layout: Layout<'b>,
        renderer: &Renderer,
        viewport: &Rectangle,
        translation: Vector,
    ) -> Option<iced::advanced::overlay::Element<'b, Message, Theme, Renderer>> {
        self.content.as_widget_mut().overlay(
            &mut tree.children[0],
            layout,
            renderer,
            viewport,
            translation,
        )
    }
}

#[derive(Debug, Default, Clone)]
struct State {
    is_focused: bool,
}

impl widget::operation::Focusable for State {
    fn is_focused(&self) -> bool {
        self.is_focused
    }

    fn focus(&mut self) {
        self.is_focused = true;
    }

    fn unfocus(&mut self) {
        self.is_focused = false;
    }
}

impl<'a, Message, Theme, Renderer> From<Focusable<'a, Message, Theme, Renderer>>
    for Element<'a, Message, Theme, Renderer>
where
    Message: 'a,
    Theme: 'a,
    Renderer: iced::advanced::Renderer + 'a,
{
    fn from(focusable: Focusable<'a, Message, Theme, Renderer>) -> Self {
        Element::new(focusable)
    }
}
