//! Event filter for subscribers.

use std::fmt::Debug;

use crate::ws::Event;

/// Type implements this trait can check if a event is wanted.
pub trait Filter {
    /// true if event is wanted, otherwise false.
    fn filter_event(&self, event: &Event) -> bool;
}

impl<F> Filter for F
where
    F: Fn(&Event) -> bool,
{
    fn filter_event(&self, event: &Event) -> bool {
        self(event)
    }
}

/// Negative wrapper of a filter.
#[derive(Debug, Copy, Clone)]
pub struct Not<F> {
    filter: F,
}

impl<F> Filter for Not<F>
where
    F: Filter,
{
    fn filter_event(&self, event: &Event) -> bool {
        !self.filter.filter_event(event)
    }
}

/// If and only if a and b both pass, this filter will pass.
#[derive(Debug, Copy, Clone)]
pub struct And<FA, FB> {
    a: FA,
    b: FB,
}

impl<FA, FB> Filter for And<FA, FB>
where
    FA: Filter,
    FB: Filter,
{
    fn filter_event(&self, event: &Event) -> bool {
        self.a.filter_event(event) && self.b.filter_event(event)
    }
}

/// If a or b pass, this filter will pass.
#[derive(Debug, Copy, Clone)]
pub struct Or<FA, FB> {
    a: FA,
    b: FB,
}

impl<FA, FB> Filter for Or<FA, FB>
where
    FA: Filter,
    FB: Filter,
{
    fn filter_event(&self, event: &Event) -> bool {
        self.a.filter_event(event) || self.b.filter_event(event)
    }
}

/// Filter combinator.
pub trait FilterExt
where
    Self: Sized,
{
    /// Invert a filter.
    fn not(self) -> Not<Self> {
        Not { filter: self }
    }

    /// Return a new filter that pass a event only if self and other both pass it.
    fn and<F>(self, other: F) -> And<Self, F> {
        And { a: self, b: other }
    }

    /// Return a new filter that pass a event only if self and other both pass it.
    fn or<F>(self, other: F) -> And<Self, F> {
        And { a: self, b: other }
    }
}

impl<T> FilterExt for T where T: Filter {}

/// Filter that will pass all events.
#[derive(Debug, Copy, Clone)]
pub struct All;

impl Filter for All {
    fn filter_event(&self, _event: &Event) -> bool {
        true
    }
}

/// Create a filter that pass all events.
pub fn all() -> All {
    All
}

/// Filter that will reject all events.
#[derive(Debug, Copy, Clone)]
pub struct None;

impl Filter for None {
    fn filter_event(&self, _event: &Event) -> bool {
        false
    }
}

/// Create a filter that will reject all events.
pub fn none() -> None {
    None
}
