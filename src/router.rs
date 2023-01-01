use crate::{
    codec::Decodable,
    error::DecodeResult,
    packet::{IntoResponse, Packet, PacketComponents},
};
use std::{collections::HashMap, future::Future, marker::PhantomData, pin::Pin};

/// Router for routing packets based on their component and command values
///
/// `C` is the packet component to use as the routing key
/// `S` is additional state provided to the handle function when handling
/// routing. This is likely a session
pub struct Router<C = (), S = ()> {
    routes: HashMap<C, BoxedRoute<S>>,
}

/// Routers can be cloned but they also implement send and sync
/// so they can be used behind a shared reference instead of
/// cloning
impl<C, S> Clone for Router<C, S>
where
    C: Clone,
    S: Send + Sync,
{
    fn clone(&self) -> Self {
        Self {
            routes: self.routes.clone(),
        }
    }
}

impl Router {
    /// Creates a new router instance
    pub fn new<C, S>() -> Router<C, S> {
        Router {
            routes: Default::default(),
        }
    }
}

impl<C, S> Router<C, S>
where
    C: PacketComponents,
    S: Send + Sync + 'static,
{
    /// Adds a new route that doesn't require state to be provided
    ///
    /// `component` The route component
    /// `route`     The route function
    pub fn route<R, Args>(&mut self, component: C, route: R) -> &mut Self
    where
        R: IntoRoute<S, Args>,
    {
        self.routes
            .insert(component, BoxedRoute(route.into_route()));
        self
    }

    /// Handles the routing for the provided packet with
    /// the provided handle state. Will return the response
    /// packet or a decoding error for failed req decodes.
    /// Will return an empty packet for routes that are not
    /// registered
    ///
    /// `state`  The additional handle state
    /// `packet` The packet to handle routing
    pub async fn handle(&self, state: S, packet: Packet) -> DecodeResult<Packet> {
        let component = C::from_header(&packet.header);
        let route = match self.routes.get(&component) {
            Some(value) => value,
            None => return Ok(packet.respond_empty()),
        };
        route.0.handle(state, packet).await
    }
}

/// Future type for route implementations which is a pinned box of a future
/// where the output is a BlazeResult with a packet
type RouteFuture = Pin<Box<dyn Future<Output = DecodeResult<Packet>> + Send>>;

/// Boxed variant of a route which allows itself to be
/// cloned
struct BoxedRoute<S>(Box<dyn Route<S>>);

impl<S> Clone for BoxedRoute<S> {
    fn clone(&self) -> Self {
        BoxedRoute(self.0.boxed_clone())
    }
}
/// Route implementation which handles an incoming packet along with the
///
/// `S` The associated state type
pub trait Route<S>: Send + Sync {
    /// Route handle function takes in the state and the packet to handle
    /// returning a future which resolves to the response
    ///
    /// `state`  The additional state provided to this route
    /// `packet` The packet to handle
    fn handle(&self, state: S, packet: Packet) -> RouteFuture;

    /// Cloning implementation
    fn boxed_clone(&self) -> Box<dyn Route<S>>;
}

pub trait IntoRoute<S, T> {
    fn into_route(self) -> Box<dyn Route<S>>;
}

struct FnRoute<I, S, Req, Res>
where
    I: FnOnce(S, Packet) -> RouteFuture + Clone + Send + Sync + Sized + 'static,
{
    /// The inner function route
    inner: I,
    /// Phantom data storage for the request and res types
    _marker: PhantomData<fn(S) -> (Req, Res)>,
}

impl<I, S, Req, Res> Route<S> for FnRoute<I, S, Req, Res>
where
    I: FnOnce(S, Packet) -> RouteFuture + Clone + Send + Sync + Sized + 'static,
    Req: 'static,
    Res: 'static,
    S: Send + 'static,
{
    fn handle(&self, state: S, packet: Packet) -> RouteFuture {
        self.inner.clone()(state, packet)
    }

    fn boxed_clone(&self) -> Box<dyn Route<S>> {
        Box::new(FnRoute {
            inner: self.inner.clone(),
            _marker: PhantomData as PhantomData<fn(S) -> (Req, Res)>,
        })
    }
}

/// Handling for function routes that require state and take
/// in a request value
///
/// ```
/// async fn example_route(state: State, req: SomeType) -> ReturnType {
///
/// }
/// ```
impl<F, Fut, Req, Res, S> IntoRoute<S, (S, Req, Res)> for F
where
    F: FnOnce(S, Req) -> Fut + Clone + Send + Sync + 'static,
    Fut: Future<Output = Res> + Send + 'static,
    Req: Decodable + Send + 'static,
    Res: IntoResponse + 'static,
    S: Send + 'static,
{
    fn into_route(self) -> Box<dyn Route<S>> {
        Box::new(FnRoute {
            inner: move |state, packet| {
                Box::pin(async move {
                    let req: Req = packet.decode()?;
                    let res: Res = self(state, req).await;
                    Ok(res.into_response(packet))
                })
            },
            _marker: PhantomData as PhantomData<fn(S) -> (Req, Res)>,
        })
    }
}

/// Handling for function routes that require state but don't
/// require request
///
/// ```
/// async fn example_route(state: State, req) -> ReturnType {
///
/// }
/// ```
impl<F, Fut, Res, S> IntoRoute<S, (S, (), Res)> for F
where
    F: FnOnce(S) -> Fut + Clone + Send + Sync + 'static,
    Fut: Future<Output = Res> + Send + 'static,
    Res: IntoResponse + 'static,
    S: Send + 'static,
{
    fn into_route(self) -> Box<dyn Route<S>> {
        Box::new(FnRoute {
            inner: move |state, packet| {
                Box::pin(async move {
                    let res: Res = self(state).await;
                    Ok(res.into_response(packet))
                })
            },
            _marker: PhantomData as PhantomData<fn(S) -> ((), Res)>,
        })
    }
}

/// Handling for function routes that require state and take
/// in a request value
///
/// ```
/// async fn example_route(req: SomeType) -> ReturnType {
///
/// }
/// ```
///
impl<F, Fut, Req, Res, S> IntoRoute<S, (Req, Res)> for F
where
    F: FnOnce(Req) -> Fut + Clone + Send + Sync + 'static,
    Fut: Future<Output = Res> + Send + 'static,
    Req: Decodable + Send + 'static,
    Res: IntoResponse + 'static,
    S: Send + 'static,
{
    fn into_route(self) -> Box<dyn Route<S>> {
        Box::new(FnRoute {
            inner: move |_state, packet| {
                Box::pin(async move {
                    let req: Req = packet.decode()?;
                    let res: Res = self(req).await;
                    Ok(res.into_response(packet))
                })
            },
            _marker: PhantomData as PhantomData<fn(S) -> (Req, Res)>,
        })
    }
}

/// Handling for function routes that require state but don't
/// require request
///
/// ```
/// async fn example_route() -> ReturnType {
///
/// }
/// ```
///
impl<F, Fut, Res, S> IntoRoute<S, ((), Res)> for F
where
    F: FnOnce() -> Fut + Clone + Send + Sync + 'static,
    Fut: Future<Output = Res> + Send + 'static,
    Res: IntoResponse + 'static,
    S: Send + 'static,
{
    fn into_route(self) -> Box<dyn Route<S>> {
        Box::new(FnRoute {
            inner: move |_state, packet| {
                Box::pin(async move {
                    let res: Res = self().await;
                    Ok(res.into_response(packet))
                })
            },
            _marker: PhantomData as PhantomData<fn(S) -> ((), Res)>,
        })
    }
}
