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

/// Empty type used to represent the format of handler
/// that is provided state
///
/// This type is just used to prevent implementation conflicts
/// between stateful and stateless handlers
pub struct FormatA;

/// Empty type used to represent the format of handler
/// that is not provided state
///
/// This type is just used to prevent implementation conflicts
/// between stateful and stateless handlers
pub struct FormatB;

/// Wrapper over the [FromRequest] type to support the unit type
/// to differentiate
pub trait FromRequestInternal: Sized + 'static {
    fn from_request(req: &Packet) -> DecodeResult<Self>;
}

/// Unit type implementation for handlers that don't take a req type
impl FromRequestInternal for () {
    fn from_request(_req: &Packet) -> DecodeResult<Self> {
        Ok(())
    }
}

/// Implementation for normal [FromRequest] implementations
impl<F: FromRequest> FromRequestInternal for F {
    fn from_request(req: &Packet) -> DecodeResult<Self> {
        F::from_request(req)
    }
}

/// Pin boxed future type that is Send and lives for 'a
type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

/// Trait implemented by handlers which can provided a boxed future
/// to a response type which can be turned into a response
///
/// `State`  The type of state provided to the handler
/// `Format` The format of the handler function (FormatA, FormatB)
/// `Req`    The request value type for the handler
/// `Res`    The response type for the handler
pub trait Handler<'a, State, Format, Req, Res>: Clone + Send + Sync + 'static {
    /// Handle function for calling the underlying handle logic using
    /// the proivded state and packet
    ///
    /// `state`  The state to provide
    /// `packet` The packet to handle
    fn handle(self, state: &'a mut State, req: Req) -> BoxFuture<'a, Res>;
}

/// Future which results in a response packet being produced that can
/// only live for the lifetime of 'a which is the state lifetime
type PacketFuture<'a> = BoxFuture<'a, Packet>;

/// Handler implementation for async functions that take the state as well
/// as a request type
///
/// ```
/// struct State;
/// struct Req;
/// struct Res;
///
/// async fn test(state: &mut State, req: Req) -> Res {
///     Res {}
/// }
/// ```
impl<'a, State, Fn, Fut, Req, Res> Handler<'a, State, FormatA, Req, Res> for Fn
where
    Fn: FnOnce(&'a mut State, Req) -> Fut + Clone + Send + Sync + 'static,
    Fut: Future<Output = Res> + Send + 'a,
    Req: FromRequest,
    Res: IntoResponse,
    State: Send + 'static,
{
    fn handle(self, state: &'a mut State, req: Req) -> BoxFuture<'a, Res> {
        Box::pin(self(state, req))
    }
}

/// Handler implementation for async functions that take the request type
/// without any state
///
/// ```
/// struct Req;
/// struct Res;
///
/// async fn test(req: Req) -> Res {
///     Res {}
/// }
/// ```
impl<State, Fn, Fut, Req, Res> Handler<'_, State, FormatB, Req, Res> for Fn
where
    Fn: FnOnce(Req) -> Fut + Clone + Send + Sync + 'static,
    Fut: Future<Output = Res> + Send + 'static,
    Req: FromRequest,
    Res: IntoResponse,
    State: Send + 'static,
{
    fn handle(self, _state: &mut State, req: Req) -> BoxFuture<'static, Res> {
        Box::pin(self(req))
    }
}

/// Handler implementation for async functions that take the state with no
/// request type
///
/// ```
/// struct State;
/// struct Res;
///
/// async fn test(state: &mut State) -> Res {
///     Res {}
/// }
/// ```
impl<'a, State, Fn, Fut, Res> Handler<'a, State, FormatA, (), Res> for Fn
where
    Fn: FnOnce(&'a mut State) -> Fut + Clone + Send + Sync + 'static,
    Fut: Future<Output = Res> + Send + 'a,
    Res: IntoResponse,
    State: Send + 'static,
{
    fn handle(self, state: &'a mut State, _: ()) -> BoxFuture<'a, Res> {
        Box::pin(self(state))
    }
}

/// Handler implementation for async functions with no arguments
///
/// ```
/// struct Res;
///
/// async fn test() -> Res {
///     Res {}
/// }
/// ```
impl<State, Fn, Fut, Res> Handler<'_, State, FormatB, (), Res> for Fn
where
    Fn: FnOnce() -> Fut + Clone + Send + Sync + 'static,
    Fut: Future<Output = Res> + Send + 'static,
    Res: IntoResponse,
    State: Send + 'static,
{
    fn handle(self, _state: &mut State, _: ()) -> BoxFuture<'static, Res> {
        Box::pin(self())
    }
}

/// Future wrapper that wraps a future from a handler in order
/// to poll the underlying future and then transform the future
/// result into the response packet
///
/// 'a:   The lifetime of the session
/// `Res` The response type for the handler
struct HandlerFuture<'a, Res> {
    /// The future from the hanlder
    fut: BoxFuture<'a, Res>,
    /// The packet the handler is responding to
    packet: Packet,
}

impl<'a, Res> Future for HandlerFuture<'a, Res>
where
    Res: IntoResponse,
{
    type Output = Packet;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.get_mut();
        // Poll the underlying future
        let fut = Pin::new(&mut this.fut);
        let res = ready!(fut.poll(cx));
        // Transform the result
        let packet = res.into_response(&this.packet);
        Poll::Ready(packet)
    }
}

/// Trait for erasing the inner types of the handler routes
///
///
trait Route<S>: Send + Sync {
    /// Handle function for calling the handler logic on the actual implementation
    /// producing a future that lives as long as the state
    ///
    /// `state`  The state provided
    /// `packet` The packet to handle with the route
    fn handle(
        self: Box<Self>,
        state: &mut S,
        packet: Packet,
    ) -> Result<PacketFuture<'_>, HandleError>;

    /// Cloning implementation to clone self
    fn boxed_clone(&self) -> Box<dyn Route<S>>;
}

/// Route wrapper over a handler for storing the phantom type data
/// and implementing Route
struct HandlerRoute<H, Format, Req, Res> {
    /// The underlying handler
    handler: H,
    /// Marker for storing related data
    _marker: PhantomData<fn(Format, Req) -> Res>,
}

/// Route implementation for handlers wrapped by handler routes
impl<H, State, Format, Req, Res> Route<State> for HandlerRoute<H, Format, Req, Res>
where
    for<'a> H: Handler<'a, State, Format, Req, Res>,
    Req: FromRequestInternal,
    Res: IntoResponse,
    Format: 'static,
    State: Send + 'static,
{
    fn handle(
        self: Box<Self>,
        state: &mut State,
        packet: Packet,
    ) -> Result<PacketFuture<'_>, HandleError> {
        let req = match Req::from_request(&packet) {
            Ok(value) => value,
            Err(err) => return Err(HandleError::Decoding(err)),
        };
        let fut = self.handler.handle(state, req);
        Ok(Box::pin(HandlerFuture { fut, packet }))
    }

    fn boxed_clone(&self) -> Box<dyn Route<State>> {
        Box::new(HandlerRoute {
            handler: self.handler.clone(),
            _marker: PhantomData,
        })
    }
}

/// Route implementation for storing components mapped to route
/// handlers
pub struct Router<C, S> {
    /// The map of components to routes
    routes: HashMap<C, Box<dyn Route<S>>>,
}

impl<C, S> Default for Router<C, S> {
    fn default() -> Self {
        Self {
            routes: Default::default(),
        }
    }
}

impl<C, S> Router<C, S>
where
    C: PacketComponents,
    S: Send + 'static,
{
    /// Creates a new router
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds a new route to the router where the route is something that implements
    /// the handler type with any lifetime. The value is wrapped with a HandlerRoute
    /// and stored boxed in the routes map under the component key
    ///
    /// `component` The component key for the route
    /// `route`     The actual route handler function
    pub fn route<Format, Req, Res>(
        &mut self,
        component: C,
        route: impl for<'a> Handler<'a, S, Format, Req, Res>,
    ) where
        Req: FromRequestInternal,
        Res: IntoResponse,
        Format: 'static,
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
        let target = match C::from_header(&packet.header) {
            Some(value) => value,
            None => return Err(HandleError::MissingHandler(packet)),
        };
        let route = match self.routes.get(&target) {
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
