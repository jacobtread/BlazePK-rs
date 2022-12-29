use std::{collections::HashMap, future::Future, marker::PhantomData, pin::Pin};

use crate::{
    codec::Decodable,
    error::DecodeResult,
    packet::{IntoResponse, Packet, PacketComponents},
};

/// Router for routing packets based on their component and command values
///
/// `C` is the packet component to use as the routing key
/// `S` is additional state provided to the handle function when handling
/// routing. This is likely a session
pub struct Router<C = (), S = ()> {
    routes: HashMap<C, Box<dyn Route<S>>>,
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
    S: Send + 'static,
{
    /// Adds a new route that doesn't require state to be provided
    ///
    /// `component` The route component
    /// `route`     The route function
    pub fn route<R, Req, Res>(mut self, component: C, route: R) -> Self
    where
        R: FnRoute<Req, Res>,
        Req: 'static,
        Res: 'static,
    {
        self.routes.insert(
            component,
            Box::new(FnRouteWrapper {
                inner: route,
                _marker: PhantomData,
            }),
        );
        self
    }

    /// Adds a new route that requires state to be provided
    ///
    /// `component` The route component
    /// `route`     The route function
    pub fn route_stateful<R, Req, Res>(mut self, component: C, route: R) -> Self
    where
        R: FnRouteStateful<Req, Res, S>,
        Req: 'static,
        Res: 'static,
    {
        self.routes.insert(
            component,
            Box::new(StateFnRouteWrapper {
                inner: route,
                _marker: PhantomData,
            }),
        );
        self
    }

    /// Handles the routing for the provided packet with
    /// the provided handle state. Will return the response
    /// packet or a decoding error for failed req decodes.
    /// Will return None if there were no routes available to
    /// handle the component
    ///
    /// `state`  The additional handle state
    /// `packet` The packet to handle routing
    pub async fn handle(&self, state: S, packet: Packet) -> Option<DecodeResult<Packet>> {
        let component = C::from_header(&packet.header);
        let route = self.routes.get(&component)?;
        let response = route.handle(state, packet).await;
        Some(response)
    }
}

/// Future type for route implementations which is a pinned box of a future
/// where the output is a BlazeResult with a packet
type RouteFuture = Pin<Box<dyn Future<Output = DecodeResult<Packet>>>>;

/// Route implementation which handles an incoming packet along with the
trait Route<S> {
    /// Route handle function takes in the state and the packet to handle
    /// returning a future which resolves to the response
    ///
    /// `state`  The additional state provided to this route
    /// `packet` The packet to handle
    fn handle(&self, state: S, packet: Packet) -> RouteFuture;
}

/// Trait implementation for function based routing
pub trait FnRoute<Req, Res>: Clone + Send + Sized + 'static {
    fn handle(self, packet: Packet) -> RouteFuture;
}

/// Wrapper for function routes that allow them to implement the
/// route trait to handle routes using the underlying route fn
struct FnRouteWrapper<I, Req, Res> {
    /// The inner function router
    inner: I,
    /// Phantom data storage for the request and res types
    _marker: PhantomData<(Req, Res)>,
}

/// Trait implementation for function based routing where state
/// is provided to the route function
pub trait FnRouteStateful<Req, Res, S>: Clone + Send + Sized + 'static {
    fn handle(self, state: S, packet: Packet) -> RouteFuture;
}

/// Wrapper for function routes that allow them to implement the
/// route trait to handle routes using the underlying route fn
/// which needs state
struct StateFnRouteWrapper<I, Req, Res> {
    /// The inner function router
    inner: I,
    /// Phantom data storage for the request and res types
    _marker: PhantomData<(Req, Res)>,
}

impl<I, Req, Res, S> Route<S> for StateFnRouteWrapper<I, Req, Res>
where
    I: FnRouteStateful<Req, Res, S>,
    S: Send + 'static,
{
    fn handle(&self, state: S, packet: Packet) -> RouteFuture {
        let handler = self.inner.clone();
        Box::pin(handler.handle(state, packet))
    }
}

/// Handling for function routes that require state and take
/// in a request value
impl<F, Fut, Req, Res, S> FnRouteStateful<Req, Res, S> for F
where
    F: FnOnce(S, Req) -> Fut + Clone + Send + 'static,
    Fut: Future<Output = Res> + Send + 'static,
    Req: Decodable + Send + 'static,
    Res: IntoResponse,
    S: Send + 'static,
{
    fn handle(self, state: S, packet: Packet) -> RouteFuture {
        Box::pin(async move {
            let req: Req = packet.decode()?;
            let res: Res = self(state, req).await;
            Ok(res.into_response(packet))
        })
    }
}

/// Handling for function routes that require state but don't
/// require request
impl<F, Fut, Res, S> FnRouteStateful<(), Res, S> for F
where
    F: FnOnce(S) -> Fut + Clone + Send + 'static,
    Fut: Future<Output = Res> + Send + 'static,
    Res: IntoResponse,
    S: Send + 'static,
{
    fn handle(self, state: S, packet: Packet) -> RouteFuture {
        Box::pin(async move {
            let res: Res = self(state).await;
            Ok(res.into_response(packet))
        })
    }
}

impl<I, Req, Res, S> Route<S> for FnRouteWrapper<I, Req, Res>
where
    I: FnRoute<Req, Res>,
{
    fn handle(&self, _state: S, packet: Packet) -> RouteFuture {
        let handler = self.inner.clone();
        Box::pin(handler.handle(packet))
    }
}

/// Handling for function routes that require state and take
/// in a request value
impl<F, Fut, Req, Res> FnRoute<Req, Res> for F
where
    F: FnOnce(Req) -> Fut + Clone + Send + 'static,
    Fut: Future<Output = Res> + Send + 'static,
    Req: Decodable + Send + 'static,
    Res: IntoResponse,
{
    fn handle(self, packet: Packet) -> RouteFuture {
        Box::pin(async move {
            let req: Req = packet.decode()?;
            let res: Res = self(req).await;
            Ok(res.into_response(packet))
        })
    }
}

/// Handling for function routes that require state but don't
/// require request
impl<F, Fut, Res> FnRoute<(), Res> for F
where
    F: FnOnce() -> Fut + Clone + Send + 'static,
    Fut: Future<Output = Res> + Send + 'static,
    Res: IntoResponse,
{
    fn handle(self, packet: Packet) -> RouteFuture {
        Box::pin(async move {
            let res: Res = self().await;
            Ok(res.into_response(packet))
        })
    }
}
