use std::any::{Any, TypeId};
use std::collections::HashMap;
use std::future::Future;
use std::marker::PhantomData;
use std::pin::Pin;
use std::sync::Arc;

use super::behavior::Behavior;
use super::handler::RequestHandler;
use super::request::Request;
use super::resolve::Resolve;

/// A future borrowing the container for `'a` (factories resolve through a
/// borrowed container, so they cannot return a `'static` future).
type Resolved<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

type HandlerFactory<R, C> =
    Arc<dyn for<'a> Fn(&'a C) -> Resolved<'a, Arc<dyn RequestHandler<R>>> + Send + Sync>;
type BehaviorFactory<R, C> =
    Arc<dyn for<'a> Fn(&'a C) -> Resolved<'a, Arc<dyn Behavior<R>>> + Send + Sync>;

struct RequestEntry<R: Request, C> {
    handler: Option<HandlerFactory<R, C>>,
    behaviors: Vec<BehaviorFactory<R, C>>,
}

impl<R: Request, C> Default for RequestEntry<R, C> {
    fn default() -> Self {
        Self {
            handler: None,
            behaviors: Vec::new(),
        }
    }
}

/// The routing table (bazario-style): which handler and which ordered pipeline
/// behaviors each request type maps to. It records *what* resolves to what;
/// *building* the instances is delegated to the container `C` via [`Resolve`].
///
/// Entries are type-erased by [`TypeId`] and recovered, fully typed, on send.
pub struct Registry<C> {
    entries: HashMap<TypeId, Box<dyn Any + Send + Sync>>,
    _resolver: PhantomData<fn() -> C>,
}

impl<C: Send + Sync + 'static> Registry<C> {
    #[must_use]
    pub fn new() -> Self {
        Self {
            entries: HashMap::new(),
            _resolver: PhantomData,
        }
    }

    /// Route requests of type `R` to handler `H`.
    pub fn handler<R, H>(&mut self) -> &mut Self
    where
        R: Request,
        H: RequestHandler<R> + 'static,
        C: Resolve<H>,
    {
        let factory: HandlerFactory<R, C> = Arc::new(|container: &C| {
            Box::pin(async move {
                let handler: Arc<H> = container
                    .resolve()
                    .await
                    .expect("handler not registered in the container");
                handler as Arc<dyn RequestHandler<R>>
            })
        });
        self.entry::<R>().handler = Some(factory);
        self
    }

    /// Append behavior `B` to the pipeline for requests of type `R`.
    pub fn behavior<R, B>(&mut self) -> &mut Self
    where
        R: Request,
        B: Behavior<R> + 'static,
        C: Resolve<B>,
    {
        let factory: BehaviorFactory<R, C> = Arc::new(|container: &C| {
            Box::pin(async move {
                let behavior: Arc<B> = container
                    .resolve()
                    .await
                    .expect("behavior not registered in the container");
                behavior as Arc<dyn Behavior<R>>
            })
        });
        self.entry::<R>().behaviors.push(factory);
        self
    }

    pub(super) async fn resolve_handler<R: Request>(
        &self,
        container: &C,
    ) -> Option<Arc<dyn RequestHandler<R>>> {
        let factory = self.lookup::<R>()?.handler.as_ref()?.clone();
        Some(factory(container).await)
    }

    pub(super) async fn resolve_behaviors<R: Request>(
        &self,
        container: &C,
    ) -> Vec<Arc<dyn Behavior<R>>> {
        let Some(entry) = self.lookup::<R>() else {
            return Vec::new();
        };
        let factories: Vec<_> = entry.behaviors.clone();
        let mut behaviors = Vec::with_capacity(factories.len());
        for factory in factories {
            behaviors.push(factory(container).await);
        }
        behaviors
    }

    fn entry<R: Request>(&mut self) -> &mut RequestEntry<R, C> {
        self.entries
            .entry(TypeId::of::<R>())
            .or_insert_with(|| Box::new(RequestEntry::<R, C>::default()))
            .downcast_mut::<RequestEntry<R, C>>()
            .expect("registry type-id collision")
    }

    fn lookup<R: Request>(&self) -> Option<&RequestEntry<R, C>> {
        self.entries
            .get(&TypeId::of::<R>())
            .and_then(|entry| entry.downcast_ref::<RequestEntry<R, C>>())
    }
}

impl<C: Send + Sync + 'static> Default for Registry<C> {
    fn default() -> Self {
        Self::new()
    }
}
