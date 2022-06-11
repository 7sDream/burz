use std::{
    cmp::Reverse,
    collections::{BinaryHeap, HashSet},
};

use crate::ws::event::EventData;

#[derive(Debug, Default)]
pub(crate) struct EventBuffer {
    exits: HashSet<u64>,
    buffer: BinaryHeap<Reverse<EventData>>,
}

#[derive(Debug)]
pub(crate) struct EventsCanBeSend<'a> {
    sn: u64,
    buffer: &'a mut EventBuffer,
}

impl Iterator for EventsCanBeSend<'_> {
    type Item = EventData;

    fn next(&mut self) -> Option<Self::Item> {
        let item = self.buffer.peek()?;
        if item.sn == self.sn + 1 {
            self.sn += 1;
            return Some(self.buffer.pop().unwrap());
        }
        None
    }
}

impl EventBuffer {
    pub fn put(&mut self, sn: u64, item: EventData) {
        if item.sn <= sn || self.exits.contains(&item.sn) {
            log::trace!("Duplicated event {} received, drop it", item.sn);
            return;
        }
        self.exits.insert(item.sn);
        self.buffer.push(Reverse(item));
    }

    pub fn peek(&self) -> Option<&EventData> {
        Some(&self.buffer.peek()?.0)
    }

    pub fn pop(&mut self) -> Option<EventData> {
        let item = self.buffer.pop()?;
        self.exits.remove(&item.0.sn);
        Some(item.0)
    }

    pub fn events_can_be_sent(&mut self, sn: u64) -> EventsCanBeSend {
        EventsCanBeSend { sn, buffer: self }
    }
}
