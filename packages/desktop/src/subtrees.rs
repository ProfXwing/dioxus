#![allow(non_snake_case)]
use dioxus::prelude::*;
use dioxus_core::{Element, Scope, Subtree};

#[inline_props]
pub fn WindowRenderer<'a>(cx: Scope<'a>, children: Element<'a>) -> Element {
    println!("WindowRenderer");
    let dom = cx.dom.clone();
    let subtree_id = dom.borrow_mut().generate_subtree();

    // if let Some(webview) = cx.consume_context::<Webview>() {
    // } else {
    // }
    crate::launch(dom, subtree_id);
    // }

    cx.render(rsx!(children))
}

pub fn launch(component: Component) {
    let mut dom = VirtualDom::new(component);
    dom.borrow_mut().get_subtree(SubtreeId(0)).unwrap();
}
