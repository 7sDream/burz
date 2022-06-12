//! Event subscribers.

use std::{borrow::Cow, future::Future, sync::Arc};

use crate::{
    api::{self, Client},
    ws::Event,
};

/// Subscriber can be register to bot and process event.
#[async_trait::async_trait]
pub trait Subscriber {
    /// subscriber name
    fn name(&self) -> Cow<'static, str>;
    /// callback will be execute when a bot load this subscriber
    async fn on_loaded(&mut self, client: Client);
    /// callback will be execute when a bot load this subscriber
    async fn on_event(self: Arc<Self>, event: Arc<Event>);
}

#[async_trait::async_trait]
impl<F, Fut> Subscriber for F
where
    F: Fn(Arc<Event>) -> Fut + Send + Sync,
    Fut: Future<Output = ()> + Send,
{
    fn name(&self) -> Cow<'static, str> {
        "Anonymous FnMut Subscriber".into()
    }

    async fn on_loaded(&mut self, _client: api::Client) {}

    async fn on_event(self: Arc<Self>, event: Arc<Event>) {
        self(event).await
    }
}
