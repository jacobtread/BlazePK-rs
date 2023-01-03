use crate::{
    error::DecodeResult,
    packet::{FromRequest, IntoResponse, Packet, PacketComponents},
};

use std::{collections::HashMap, future::Future, marker::PhantomData, pin::Pin};

pub trait State: Send + Sync + Sized + 'static {}

pub trait Handler<'a, S, T>: Send + Sync + 'static {
    fn handle(&self, state: &'a mut S, packet: Packet) -> DecodeResult<CallableFuture<'a>>;
}

pub type CallableFuture<'a> = Pin<Box<dyn Future<Output = Packet> + Send + 'a>>;

impl<'a, S, Fun, Fut, Req, Res> Handler<'a, S, (S, Req, Res)> for Fun
where
    Fun: FnOnce(&'a mut S, Req) -> Fut + Clone + Send + Sync + 'static,
    Fut: Future<Output = Res> + Send + 'a,
    Req: FromRequest + Send + 'a,
    Res: IntoResponse + 'a,
    S: State + 'a,
{
    fn handle(&self, state: &'a mut S, packet: Packet) -> DecodeResult<CallableFuture<'a>> {
        let req: Req = FromRequest::from_request(&packet)?;
        let inner = self.clone();
        Ok(Box::pin(async move {
            let res: Res = inner(state, req).await;
            res.into_response(&packet)
        }))
    }
}

impl<'a, S, Fun, Fut, Res> Handler<'a, S, (S, Nil, Res)> for Fun
where
    Fun: FnOnce(&'a mut S) -> Fut + Clone + Send + Sync + 'static,
    Fut: Future<Output = Res> + Send + 'a,
    Res: IntoResponse + 'a,
    S: State + 'a,
{
    fn handle(&self, state: &'a mut S, packet: Packet) -> DecodeResult<CallableFuture<'a>> {
        let inner = self.clone();
        Ok(Box::pin(async move {
            let res: Res = inner(state).await;
            res.into_response(&packet)
        }))
    }
}

impl<'a, S, Fun, Fut, Req, Res> Handler<'a, S, (Nil, Req, Res)> for Fun
where
    Fun: FnOnce(Req) -> Fut + Clone + Send + Sync + 'static,
    Fut: Future<Output = Res> + Send + 'a,
    Req: FromRequest + Send + 'a,
    Res: IntoResponse + 'a,
    S: State + 'a,
{
    fn handle(&self, _state: &'a mut S, packet: Packet) -> DecodeResult<CallableFuture<'a>> {
        let req: Req = FromRequest::from_request(&packet)?;
        let inner = self.clone();
        Ok(Box::pin(async move {
            let res: Res = inner(req).await;
            res.into_response(&packet)
        }))
    }
}

impl<'a, S, Fun, Fut, Res> Handler<'a, S, (Nil, Nil, Res)> for Fun
where
    Fun: FnOnce() -> Fut + Clone + Send + Sync + 'static,
    Fut: Future<Output = Res> + Send + 'a,
    Res: IntoResponse + 'a,
    S: State + 'a,
{
    fn handle(&self, _state: &'a mut S, packet: Packet) -> DecodeResult<CallableFuture<'a>> {
        let inner = self.clone();
        Ok(Box::pin(async move {
            let res: Res = inner().await;
            res.into_response(&packet)
        }))
    }
}

pub type RouteFuture<'a> = Pin<Box<dyn Future<Output = Packet> + Send + 'a>>;

pub trait Route<'a, S>: Send + Sync {
    fn handle(&self, state: &'a mut S, packet: Packet) -> DecodeResult<RouteFuture<'a>>;
}

pub struct HandlerRoute<'a, C, S, T> {
    callable: C,
    _marker: PhantomData<fn(&'a S) -> T>,
}

impl<'a, C, S, T> Route<'a, S> for HandlerRoute<'_, C, S, T>
where
    for<'b> C: Handler<'b, S, T>,
    S: State,
{
    fn handle(&self, state: &'a mut S, packet: Packet) -> DecodeResult<RouteFuture<'a>> {
        let fut = self.callable.handle(state, packet)?;
        Ok(fut)
    }
}

pub struct Router<C: PacketComponents, S: State> {
    routes: HashMap<C, Box<dyn for<'a> Route<'a, S>>>,
}

impl<C: PacketComponents, S: State> Router<C, S> {
    pub fn new() -> Self {
        Self {
            routes: HashMap::new(),
        }
    }

    pub fn route<T>(&mut self, component: C, route: impl for<'a> Handler<'a, S, T>)
    where
        for<'a> T: 'a,
    {
        self.routes.insert(
            component,
            Box::new(HandlerRoute {
                callable: route,
                _marker: PhantomData,
            }),
        );
    }

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

pub struct Empty;
pub struct Nil;

impl FromRequest for Empty {
    fn from_request(_req: &Packet) -> DecodeResult<Self> {
        Ok(Self)
    }
}
