use futures_util::FutureExt;
use std::{
    rc::Rc,
    task::{Context, Poll},
};

use crate::{
    innerlude::{Mutation, Mutations, SuspenseContext},
    nodes::RenderReturn,
    ScopeId, TaskId, VNode, VirtualDom, subtree::Subtree,
};

use super::SuspenseId;

impl VirtualDom {
    /// Handle notifications by tasks inside the scheduler
    ///
    /// This is precise, meaning we won't poll every task, just tasks that have woken up as notified to use by the
    /// queue
    pub(crate) fn handle_task_wakeup(&mut self, id: TaskId) {
        let mut tasks = self.scheduler.tasks.borrow_mut();

        let task = match tasks.get(id.0) {
            Some(task) => task,
            // The task was removed from the scheduler, so we can just ignore it
            None => return,
        };

        let mut cx = Context::from_waker(&task.waker);

        // If the task completes...
        if task.task.borrow_mut().as_mut().poll(&mut cx).is_ready() {
            // Remove it from the scope so we dont try to double drop it when the scope dropes
            let scope = &self.scopes[task.scope];
            scope.spawned_tasks.borrow_mut().remove(&id);

            // Remove it from the scheduler
            tasks.try_remove(id.0);
        }
    }

    pub(crate) fn acquire_suspense_boundary(&self, id: ScopeId) -> Rc<SuspenseContext> {
        self.scopes[id]
            .consume_context::<Rc<SuspenseContext>>()
            .unwrap()
    }
}

impl Subtree {
    pub(crate) fn handle_suspense_wakeup(&mut self, id: SuspenseId) {
        let dom2 = self.dom.borrow();
        let leaves = dom2.scheduler.leaves.borrow_mut();
        let leaf = leaves.get(id.0).unwrap();

        let scope_id = leaf.scope_id;

        // todo: cache the waker
        let mut cx = Context::from_waker(&leaf.waker);

        // Safety: the future is always pinned to the bump arena
        let mut pinned = unsafe { std::pin::Pin::new_unchecked(&mut *leaf.task) };
        let as_pinned_mut = &mut pinned;

        // the component finished rendering and gave us nodes
        // we should attach them to that component and then render its children
        // continue rendering the tree until we hit yet another suspended component
        if let Poll::Ready(new_nodes) = as_pinned_mut.poll_unpin(&mut cx) {
            let fiber = self.get_dom().acquire_suspense_boundary(leaf.scope_id);

            let dom = self.get_dom();
            let arena = dom.scopes[scope_id].current_frame();

            let ret = arena.bump().alloc(match new_nodes {
                Some(new) => RenderReturn::Ready(new),
                None => RenderReturn::default(),
            });

            arena.node.set(ret);

            drop(arena);

            fiber.waiting_on.borrow_mut().remove(&id);

            if let RenderReturn::Ready(template) = ret {
                let mutations_ref = &mut fiber.mutations.borrow_mut();
                let mutations = &mut **mutations_ref;
                let template: &VNode = unsafe { std::mem::transmute(template) };
                let mutations: &mut Mutations = unsafe { std::mem::transmute(mutations) };

                std::mem::swap(&mut self.get_dom().mutations, mutations);

                let place_holder_id = self.get_dom().scopes[scope_id].placeholder.get().unwrap();
                self.get_dom().scope_stack.push(scope_id);

                drop(leaves);
                drop(dom2);
                drop(dom);

                let created = self.create(template);
                self.get_dom().scope_stack.pop();
                mutations.push(Mutation::ReplaceWith {
                    id: place_holder_id,
                    m: created,
                });

                for leaf in self.get_dom().collected_leaves.drain(..) {
                    fiber.waiting_on.borrow_mut().insert(leaf);
                }

                std::mem::swap(&mut self.get_dom().mutations, mutations);

                if fiber.waiting_on.borrow().is_empty() {
                    self.get_dom().finished_fibers.push(fiber.id);
                }
            }
        }
    }
}