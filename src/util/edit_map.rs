//! Helpers to deal with several changes in events, batching them together.
//!
//! Preferably, changes should be kept to a minumum.
//! Sometimes, it’s needed to change the list of events, because parsing can be
//! messy, and it helps to expose a cleaner interface of events to the compiler
//! and other users.
//! It can also help to merge many adjacent similar events.
//! And, in other cases, it’s needed to parse subcontent: pass some events
//! through another tokenizer and inject the result.

use crate::tokenizer::Event;

/// Shift `previous` and `next` links according to `jumps`.
///
/// This fixes links in case there are events removed or added between them.
fn shift_links(events: &mut [Event], jumps: &[(usize, usize, usize)]) {
    let map = |before| {
        // To do: this theoretically gets slow, investigate how to improve it.
        let mut jump_index = 0;
        let mut remove = 0;
        let mut add = 0;

        while jump_index < jumps.len() {
            if jumps[jump_index].0 > before {
                break;
            }

            (_, remove, add) = jumps[jump_index];
            jump_index += 1;
        }

        before + add - remove
    };

    let mut index = 0;

    while index < events.len() {
        if let Some(link) = &mut events[index].link {
            link.previous = link.previous.map(map);
            link.next = link.next.map(map);
        }

        index += 1;
    }
}

/// Make it easy to insert and remove things while being performant and keeping
/// links in check.
#[derive(Debug)]
pub struct EditMap {
    /// Whether this map was consumed already.
    consumed: bool,
    /// Record of changes.
    map: Vec<(usize, usize, Vec<Event>)>,
}

impl EditMap {
    /// Create a new edit map.
    pub fn new() -> EditMap {
        EditMap {
            consumed: false,
            map: vec![],
        }
    }
    /// Create an edit: a remove and/or add at a certain place.
    pub fn add(&mut self, index: usize, remove: usize, add: Vec<Event>) {
        add_impl(self, index, remove, add, false);
    }
    /// Create an edit: but insert `add` before existing additions.
    pub fn add_before(&mut self, index: usize, remove: usize, add: Vec<Event>) {
        add_impl(self, index, remove, add, true);
    }
    /// Done, change the events.
    pub fn consume(&mut self, events: &mut Vec<Event>) {
        self.map
            .sort_unstable_by(|a, b| a.0.partial_cmp(&b.0).unwrap());

        assert!(!self.consumed, "cannot consume after consuming");
        self.consumed = true;

        // Calculate jumps: where items in the current list move to.
        let mut jumps = Vec::with_capacity(self.map.len());
        let mut index = 0;
        let mut add_acc = 0;
        let mut remove_acc = 0;
        while index < self.map.len() {
            let (at, remove, add) = &self.map[index];
            add_acc += add.len();
            remove_acc += remove;
            jumps.push((*at, remove_acc, add_acc));
            index += 1;
        }

        let len_before = events.len();
        let mut index = self.map.len();
        let mut vecs: Vec<Vec<Event>> = Vec::with_capacity(index * 2 + 1);
        while index > 0 {
            index -= 1;
            let (at, remove, _) = self.map[index];
            let mut keep = events.split_off(at + remove);
            shift_links(&mut keep, &jumps);
            vecs.push(keep);
            vecs.push(self.map[index].2.split_off(0));
            events.truncate(at);
        }
        shift_links(events, &jumps);
        vecs.push(events.split_off(0));

        events.reserve(len_before + add_acc - remove_acc);

        while let Some(mut slice) = vecs.pop() {
            events.append(&mut slice);
        }
    }
}

/// Create an edit.
fn add_impl(edit_map: &mut EditMap, at: usize, remove: usize, mut add: Vec<Event>, before: bool) {
    assert!(!edit_map.consumed, "cannot add after consuming");
    let mut index = 0;

    if remove == 0 && add.is_empty() {
        return;
    }

    while index < edit_map.map.len() {
        if edit_map.map[index].0 == at {
            edit_map.map[index].1 += remove;

            if before {
                add.append(&mut edit_map.map[index].2);
                edit_map.map[index].2 = add;
            } else {
                edit_map.map[index].2.append(&mut add);
            }

            return;
        }

        index += 1;
    }

    edit_map.map.push((at, remove, add));
}
