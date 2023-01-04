use crate::{
    error::DecodeResult,
    packet::{FromRequest, IntoResponse, Packet, PacketComponents},
};

use std::{collections::HashMap, future::Future, marker::PhantomData, pin::Pin};

/// Empty type used for representing nothing used because the
/// Unit type has trait implementations that will conflict with
/// other Handler implementations
pub struct Nil;

/// Trait implemented by structures that can be provided
/// as state to a router handle function and passed into
/// the handlers
pub trait State: Send + Sync + Sized + 'static {}

/// Trait implemented by things that can be used to handler a packet from
/// the router and return a future to a response packet.
///
/// Implements the actual calls the underlying functions in the handle
/// function along with mapping of the request and repsonse types.
pub trait Handler<'a, S, T>: Send + Sync + 'static {
    /// Handle function for calling the underlying handle logic using
    /// the proivded state and packet
    ///
    /// `state`  The state to provide
    /// `packet` The packet to handle
    fn handle(&self, state: &'a mut S, packet: Packet) -> DecodeResult<PacketFuture<'a>>;
}

/// Future which results in a response packet being produced that can
/// only live for the lifetime of 'a which is the state lifetime
pub type PacketFuture<'a> = Pin<Box<dyn Future<Output = Packet> + Send + 'a>>;

/// Handler implementation for async functions that take the state as well
/// as a request type
///
/// ```
/// async fn test(state: &mut S, req: Req) -> Res {
///
/// }
/// ```
impl<'a, S, Fun, Fut, Req, Res> Handler<'a, S, (S, Req, Res)> for Fun
where
    Fun: FnOnce(&'a mut S, Req) -> Fut + Clone + Send + Sync + 'static,
    Fut: Future<Output = Res> + Send + 'a,
    Req: FromRequest + Send + 'a,
    Res: IntoResponse + 'a,
    S: State + 'a,
{
    fn handle(&self, state: &'a mut S, packet: Packet) -> DecodeResult<PacketFuture<'a>> {
        let req: Req = FromRequest::from_request(&packet)?;
        let inner = self.clone();
        Ok(Box::pin(async move {
            let res: Res = inner(state, req).await;
            res.into_response(&packet)
        }))
    }
}

/// Handler implementation for async functions that take the state with no
/// request type
///
/// ```
/// async fn test(state: &mut S) -> Res {
///
/// }
/// ```
impl<'a, S, Fun, Fut, Res> Handler<'a, S, (S, Nil, Res)> for Fun
where
    Fun: FnOnce(&'a mut S) -> Fut + Clone + Send + Sync + 'static,
    Fut: Future<Output = Res> + Send + 'a,
    Res: IntoResponse + 'a,
    S: State + 'a,
{
    fn handle(&self, state: &'a mut S, packet: Packet) -> DecodeResult<PacketFuture<'a>> {
        let inner = self.clone();
        Ok(Box::pin(async move {
            let res: Res = inner(state).await;
            res.into_response(&packet)
        }))
    }
}

/// Handler implementation for async functions that take the request type
/// without any state
///
/// ```
/// async fn test(req: Req) -> Res {
///
/// }
/// ```
impl<'a, S, Fun, Fut, Req, Res> Handler<'a, S, (Nil, Req, Res)> for Fun
where
    Fun: FnOnce(Req) -> Fut + Clone + Send + Sync + 'static,
    Fut: Future<Output = Res> + Send + 'a,
    Req: FromRequest + Send + 'a,
    Res: IntoResponse + 'a,
    S: State + 'a,
{
    fn handle(&self, _state: &'a mut S, packet: Packet) -> DecodeResult<PacketFuture<'a>> {
        let req: Req = FromRequest::from_request(&packet)?;
        let inner = self.clone();
        Ok(Box::pin(async move {
            let res: Res = inner(req).await;
            res.into_response(&packet)
        }))
    }
}

/// Handler implementation for async functions with no arguments
///
/// ```
/// async fn test() -> Res {
///
/// }
/// ```
impl<'a, S, Fun, Fut, Res> Handler<'a, S, (Nil, Nil, Res)> for Fun
where
    Fun: FnOnce() -> Fut + Clone + Send + Sync + 'static,
    Fut: Future<Output = Res> + Send + 'a,
    Res: IntoResponse + 'a,
    S: State + 'a,
{
    fn handle(&self, _state: &'a mut S, packet: Packet) -> DecodeResult<PacketFuture<'a>> {
        let inner = self.clone();
        Ok(Box::pin(async move {
            let res: Res = inner().await;
            res.into_response(&packet)
        }))
    }
}

/// Trait for erasing the inner types of the handler routes
pub trait Route<'a, S>: Send + Sync {
    /// Handle function for calling the handler logic on the actual implementation
    /// producing a future that lives as long as the state
    ///
    /// `state`  The state provided
    /// `packet` The packet to handle with the route
    fn handle(&self, state: &'a mut S, packet: Packet) -> DecodeResult<PacketFuture<'a>>;
}

/// Structure wrapping a handler to allow the handler to implement Route
pub struct HandlerRoute<'a, C, S, T> {
    /// The underlying handler
    handler: C,
    /// Marker for storing related data
    _marker: PhantomData<fn(&'a S) -> T>,
}

/// Route implementation for handlers wrapped by handler routes
impl<'a, C, S, T> Route<'a, S> for HandlerRoute<'_, C, S, T>
where
    for<'b> C: Handler<'b, S, T>,
    S: State,
{
    fn handle(&self, state: &'a mut S, packet: Packet) -> DecodeResult<PacketFuture<'a>> {
        let fut = self.handler.handle(state, packet)?;
        Ok(fut)
    }
}

/// Route implementation for storing components mapped to route
/// handlers
pub struct Router<C: PacketComponents, S: State> {
    /// The map of components to routes
    routes: HashMap<C, Box<dyn for<'a> Route<'a, S>>>,
}

impl<C: PacketComponents, S: State> Router<C, S> {
    /// Creates a new router
    pub fn new() -> Self {
        Self {
            routes: HashMap::new(),
        }
    }

    /// Adds a new route to the router where the route is something that implements
    /// the handler type with any lifetime. The value is wrapped with a HandlerRoute
    /// and stored boxed in the routes map under the component key
    ///
    /// `component` The component key for the route
    /// `route`     The actual route handler function
    pub fn route<T>(&mut self, component: C, route: impl for<'a> Handler<'a, S, T>)
    where
        for<'a> T: 'a,
    {
        self.routes.insert(
            component,
            Box::new(HandlerRoute {
                handler: route,
                _marker: PhantomData,
            }),
        );
    }

    /// Handle function takes the provided packet retrieves the component from its header
    /// and finds the matching route (Returning an empty response immediately if none match)
    /// and providing the state the route along with the packet awaiting the route future
    ///
    /// `state`  The provided state
    /// `packet` The packet to handle
    pub async fn handle<'a>(&self, state: &'a mut S, packet: Packet) -> DecodeResult<Packet> {
        let component = C::from_header(&packet.header);
        let route = match self.routes.get(&component) {
            Some(value) => value,
            None => return Ok(packet.respond_empty()),
        };
        let response = route.handle(state, packet)?.await;
        Ok(response)
    }
}
