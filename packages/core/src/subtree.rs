/*
This is a WIP module

Subtrees allow the virtualdom to split up the mutation stream into smaller chunks which can be directed to different parts of the dom.
It's core to implementing multiwindow desktop support, portals, and alternative inline renderers like react-three-fiber.

The primary idea is to give each renderer a linear element tree managed by Dioxus to maximize performance and minimize memory usage.
This can't be done if two renderers need to share the same native tree.
With subtrees, we have an entirely different slab of elements

*/
use std::{rc::{Rc}, cell::{Cell, RefCell, RefMut}, any::Any};
use slab::Slab;
use crate::{innerlude::{ElementRef}, ElementId, Event, AttributeValue, ScopeId, Scope, ScopeState, VirtualDom};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SubtreeId(pub usize);

/// A collection of elements confined to a single scope under a chunk of the tree
///
/// All elements in this collection are guaranteed to be in the same scope and share the same numbering
///
/// This unit can be multithreaded
/// Whenever multiple subtrees are present, we can perform **parallel diffing**
pub struct Subtree {
    pub id: usize,
    // namespace: Cow<'static, str>,
    pub(crate) elements: Slab<ElementRef>,
    pub dom: Rc<RefCell<VirtualDom>>
}

impl Subtree {
    pub fn none(dom: Rc<RefCell<VirtualDom>>) -> Self {
        Self {
            id: 0,
            elements: Slab::new(),
            dom
        }
    }

    pub fn new(id: SubtreeId, dom: Rc<RefCell<VirtualDom>>) -> Self {
        Self {
            id: id.0,
            elements: Slab::new(),
            dom
        }
    }

    pub fn get_dom<'a>(&'a self) -> RefMut<VirtualDom> {
        self.dom.borrow_mut()
    }

    /// Call a listener inside the VirtualDom with data from outside the VirtualDom.
    ///
    /// This method will identify the appropriate element. The data must match up with the listener delcared. Note that
    /// this method does not give any indication as to the success of the listener call. If the listener is not found,
    /// nothing will happen.
    ///
    /// It is up to the listeners themselves to mark nodes as dirty.
    ///
    /// If you have multiple events, you can call this method multiple times before calling "render_with_deadline"
    pub fn handle_event(
        &mut self,
        name: &str,
        data: Rc<dyn Any>,
        element: ElementId,
        bubbles: bool,
    ) {
        /*
        ------------------------
        The algorithm works by walking through the list of dynamic attributes, checking their paths, and breaking when
        we find the target path.

        With the target path, we try and move up to the parent until there is no parent.
        Due to how bubbling works, we call the listeners before walking to the parent.

        If we wanted to do capturing, then we would accumulate all the listeners and call them in reverse order.
        ----------------------

        For a visual demonstration, here we present a tree on the left and whether or not a listener is collected on the
        right.

        |           <-- yes (is ascendant)
        | | |       <-- no  (is not direct ascendant)
        | |         <-- yes (is ascendant)
        | | | | |   <--- target element, break early, don't check other listeners
        | | |       <-- no, broke early
        |           <-- no, broke early
        */
        let mut parent_path = self.elements.get(element.0);
        let mut listeners = vec![];

        // We will clone this later. The data itself is wrapped in RC to be used in callbacks if required
        let uievent = Event {
            propagates: Rc::new(Cell::new(bubbles)),
            data,
        };

        // Loop through each dynamic attribute in this template before moving up to the template's parent.
        while let Some(el_ref) = parent_path {
            // safety: we maintain references of all vnodes in the element slab
            let template = unsafe { el_ref.template.unwrap().as_ref() };
            let node_template = template.template.get();
            let target_path = el_ref.path;

            for (idx, attr) in template.dynamic_attrs.iter().enumerate() {
                let this_path = node_template.attr_paths[idx];

                // Remove the "on" prefix if it exists, TODO, we should remove this and settle on one
                if attr.name.trim_start_matches("on") == name
                    && target_path.is_decendant(&this_path)
                {
                    listeners.push(&attr.value);

                    // Break if the event doesn't bubble anyways
                    if !bubbles {
                        break;
                    }

                    // Break if this is the exact target element.
                    // This means we won't call two listeners with the same name on the same element. This should be
                    // documented, or be rejected from the rsx! macro outright
                    if target_path == this_path {
                        break;
                    }
                }
            }

            // Now that we've accumulated all the parent attributes for the target element, call them in reverse order
            // We check the bubble state between each call to see if the event has been stopped from bubbling
            for listener in listeners.drain(..).rev() {
                if let AttributeValue::Listener(listener) = listener {
                    if let Some(cb) = listener.borrow_mut().as_deref_mut() {
                        cb(uievent.clone());
                    }

                    if !uievent.propagates.get() {
                        return;
                    }
                }
            }

            parent_path = template.parent.and_then(|id| self.elements.get(id.0));
        }
    }
}