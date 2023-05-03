#![allow(non_snake_case)]
use std::{rc::Weak, cell::RefCell};

use dioxus::prelude::*;
use dioxus_core::{Element, Scope, SubtreeId, Subtree};

#[inline_props]
pub fn WindowRenderer<'a>(cx: Scope<'a>, dom: &'a mut VirtualDom, children: Element<'a>) -> Element {
    let subtree_id = dom.generate_subtree();

    // if let Some(webview) = cx.consume_context::<Webview>() {
    // } else {
    crate::launch(*dom, subtree_id);
    // }

    cx.render(rsx!(children))
}

pub fn launch(component: Component) {
    let mut dom = VirtualDom::new(component);
}

pub fn create_webview(subtree: &Subtree, dom: &VirtualDom) {

}
