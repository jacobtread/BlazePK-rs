use crate::{
    error::{DecodeError, DecodeResult},
    packet::{FromRequest, IntoResponse, Packet, PacketComponents},
};

use std::{
    collections::HashMap,
    future::Future,
    marker::PhantomData,
    pin::Pin,
    task::{ready, Context, Poll},
};

/// Empty type used for representing nothing used because the
/// Unit type has trait implementations that will conflict with
/// other Handler implementations
pub struct Nil;

/// Trait implemented by structures that can be provided
/// as state to a router handle function and passed into
/// the handlers
pub trait State: Send + 'static {}

/// Wrapper of the from request trait to include the Nil type
pub trait FromRequestWrapper: Sized + 'static {
    fn from_request(req: &Packet) -> DecodeResult<Self>;
}

impl FromRequestWrapper for Nil {
    fn from_request(_req: &Packet) -> DecodeResult<Self> {
        Ok(Nil)
    }
}

impl<F: FromRequest> FromRequestWrapper for F {
    fn from_request(req: &Packet) -> DecodeResult<Self> {
        F::from_request(req)
    }
}

/// Trait implemented by things that can be used to handler a packet from
/// the router and return a future to a response packet.
///
/// Implements the actual calls the underlying functions in the handle
/// function along with mapping of the request and repsonse types.
pub trait Handler<'a, S, Si, Req, Res>: Clone + Send + Sync + 'static {
    /// Handle function for calling the underlying handle logic using
    /// the proivded state and packet
    ///
    /// `state`  The state to provide
    /// `packet` The packet to handle
    fn handle(self, state: &'a mut S, req: Req) -> BoxFuture<'a, Res>;
}

/// Future which results in a response packet being produced that can
/// only live for the lifetime of 'a which is the state lifetime
type PacketFuture<'a> = BoxFuture<'a, Packet>;

/// Handler implementation for async functions that take the state as well
/// as a request type
///
/// ```
/// async fn test(state: &mut S, req: Req) -> Res {
///
/// }
/// ```
impl<'a, S, Fun, Fut, Req, Res> Handler<'a, S, S, Req, Res> for Fun
where
    Fun: FnOnce(&'a mut S, Req) -> Fut + Clone + Send + Sync + 'static,
    Fut: Future<Output = Res> + Send + 'a,
    Req: FromRequest,
    Res: IntoResponse,
    S: State,
{
    fn handle(self, state: &'a mut S, req: Req) -> BoxFuture<'a, Res> {
        Box::pin(self(state, req))
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
impl<'a, S, Fun, Fut, Res> Handler<'a, S, S, Nil, Res> for Fun
where
    Fun: FnOnce(&'a mut S) -> Fut + Clone + Send + Sync + 'static,
    Fut: Future<Output = Res> + Send + 'a,
    Res: IntoResponse,
    S: State,
{
    fn handle(self, state: &'a mut S, _: Nil) -> BoxFuture<'a, Res> {
        Box::pin(self(state))
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
impl<'a, S, Fun, Fut, Req, Res> Handler<'a, S, Nil, Req, Res> for Fun
where
    Fun: FnOnce(Req) -> Fut + Clone + Send + Sync + 'static,
    Fut: Future<Output = Res> + Send + 'a,
    Req: FromRequest,
    Res: IntoResponse,
    S: State,
{
    fn handle(self, _state: &'a mut S, req: Req) -> BoxFuture<'a, Res> {
        Box::pin(self(req))
    }
}

/// Handler implementation for async functions with no arguments
///
/// ```
/// async fn test() -> Res {
///
/// }
/// ```
impl<'a, S, Fun, Fut, Res> Handler<'a, S, Nil, Nil, Res> for Fun
where
    Fun: FnOnce() -> Fut + Clone + Send + Sync + 'static,
    Fut: Future<Output = Res> + Send + 'a,
    Res: IntoResponse,
    S: State,
{
    fn handle(self, _state: &'a mut S, _: Nil) -> BoxFuture<'a, Res> {
        Box::pin(self())
    }
}

type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

struct HandlerFuture<'a, Res> {
    fut: BoxFuture<'a, Res>,
    packet: Packet,
}

impl<'a, Res> Future for HandlerFuture<'a, Res>
where
    Res: IntoResponse,
{
    type Output = Packet;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.get_mut();
        let fut = Pin::new(&mut this.fut);
        let res = ready!(fut.poll(cx));
        let packet = res.into_response(&this.packet);
        Poll::Ready(packet)
    }
}

/// Trait for erasing the inner types of the handler routes
trait Route<'a, S>: Send + Sync {
    /// Handle function for calling the handler logic on the actual implementation
    /// producing a future that lives as long as the state
    ///
    /// `state`  The state provided
    /// `packet` The packet to handle with the route
    fn handle(
        self: Box<Self>,
        state: &'a mut S,
        packet: Packet,
    ) -> Result<PacketFuture<'a>, HandleError>;

    fn boxed_clone(&self) -> Box<dyn Route<'a, S>>;
}

/// Structure wrapping a handler to allow the handler to implement Route
struct HandlerRoute<H, Si, Req, Res> {
    /// The underlying handler
    handler: H,
    /// Marker for storing related data
    _marker: PhantomData<fn(Si, Req) -> Res>,
}

/// Route implementation for handlers wrapped by handler routes
impl<'a, H, S, Si, Req, Res> Route<'a, S> for HandlerRoute<H, Si, Req, Res>
where
    H: Handler<'a, S, Si, Req, Res>,
    Req: FromRequestWrapper,
    Res: IntoResponse,
    Si: 'static,
    S: Send + 'static,
{
    fn handle(
        self: Box<Self>,
        state: &'a mut S,
        packet: Packet,
    ) -> Result<PacketFuture<'a>, HandleError> {
        let req = match Req::from_request(&packet) {
            Ok(value) => value,
            Err(err) => return Err(HandleError::Decoding(err)),
        };
        let fut = self.handler.handle(state, req);
        Ok(Box::pin(HandlerFuture { fut, packet }))
    }

    fn boxed_clone(&self) -> Box<dyn Route<'a, S>> {
        Box::new(HandlerRoute {
            handler: self.handler.clone(),
            _marker: PhantomData,
        })
    }
}

/// Route implementation for storing components mapped to route
/// handlers
pub struct Router<C: PacketComponents, S> {
    /// The map of components to routes
    routes: HashMap<C, Box<dyn for<'a> Route<'a, S>>>,
}

impl<C, S> Router<C, S>
where
    C: PacketComponents,
    S: State,
{
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
    pub fn route<Si, Req, Res>(
        &mut self,
        component: C,
        route: impl for<'a> Handler<'a, S, Si, Req, Res>,
    ) where
        Req: FromRequestWrapper,
        Res: IntoResponse,
        Si: 'static,
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
    pub fn handle<'a>(
        &self,
        state: &'a mut S,
        packet: Packet,
    ) -> Result<PacketFuture<'a>, HandleError> {
        let component = C::from_header(&packet.header);
        let route = match self.routes.get(&component) {
            Some(value) => value.boxed_clone(),
            None => return Err(HandleError::MissingHandler(packet)),
        };
        route.handle(state, packet)
    }
}

/// Error that can occur while handling a packet
#[derive(Debug)]
pub enum HandleError {
    // There wasn't an available handler for the provided packet
    MissingHandler(Packet),

    // Decoding error while reading the packet
    Decoding(DecodeError),
}
