//! Bevy events that will be emitted upon receiving network messages

use std::marker::PhantomData;

use bevy::prelude::{Component, Entity, Event};

use crate::packet::message::Message;
use crate::prelude::ClientId;

/// This event is emitted whenever we receive a message from the remote
#[derive(Event, Debug)]
pub struct MessageEvent<M: Message> {
    pub message: M,
    // TODO: this is not ideal. Should we have PeerId that is either ClientId or Server?
    /// The client that sent the message.
    /// If the server sent the message, we will just put ClientId::Local(0) here
    pub from: ClientId,
}

impl<M: Message> MessageEvent<M> {
    pub fn new(message: M, from: ClientId) -> Self {
        Self { message, from }
    }

    pub fn message(&self) -> &M {
        &self.message
    }

    pub fn from(&self) -> ClientId {
        self.from
    }
}

#[derive(Event)]
/// Event emitted on server every time we receive an event
pub struct InputEvent<I: crate::inputs::native::UserAction, Ctx = ()> {
    input: Option<I>,
    from: Ctx,
}

impl<I: crate::inputs::native::UserAction, Ctx: Copy> InputEvent<I, Ctx> {
    pub fn new(input: Option<I>, from: Ctx) -> Self {
        Self { input, from }
    }

    pub fn input(&self) -> &Option<I> {
        &self.input
    }

    pub fn from(&self) -> Ctx {
        self.from
    }
}

#[derive(Event)]
/// Event emitted whenever we spawn an entity from the remote world
pub struct EntitySpawnEvent<Ctx = ()> {
    entity: Entity,
    context: Ctx,
}

impl<Ctx> EntitySpawnEvent<Ctx> {
    pub fn new(entity: Entity, context: Ctx) -> Self {
        Self { entity, context }
    }

    pub fn entity(&self) -> Entity {
        self.entity
    }

    pub fn context(&self) -> &Ctx {
        &self.context
    }
}

/// Event emitted whenever we despawn an entity from the remote world
#[derive(Event)]
pub struct EntityDespawnEvent<Ctx = ()> {
    entity: Entity,
    context: Ctx,
}

impl<Ctx> EntityDespawnEvent<Ctx> {
    pub fn new(entity: Entity, context: Ctx) -> Self {
        Self { entity, context }
    }

    pub fn entity(&self) -> Entity {
        self.entity
    }

    pub fn context(&self) -> &Ctx {
        &self.context
    }
}

/// Event emitted whenever we update a component from the remote world
#[derive(Event)]
pub struct ComponentUpdateEvent<C: Component, Ctx = ()> {
    entity: Entity,
    context: Ctx,

    _marker: PhantomData<C>,
}

impl<C: Component, Ctx> ComponentUpdateEvent<C, Ctx> {
    pub fn new(entity: Entity, context: Ctx) -> Self {
        Self {
            entity,
            context,
            _marker: PhantomData,
        }
    }

    pub fn entity(&self) -> Entity {
        self.entity
    }

    pub fn context(&self) -> &Ctx {
        &self.context
    }
}

/// Event emitted whenever we insert a component from the remote world
#[derive(Event, Debug)]
pub struct ComponentInsertEvent<C: Component, Ctx = ()> {
    entity: Entity,
    context: Ctx,

    _marker: PhantomData<C>,
}

impl<C: Component, Ctx> ComponentInsertEvent<C, Ctx> {
    pub fn new(entity: Entity, context: Ctx) -> Self {
        Self {
            entity,
            context,
            _marker: PhantomData,
        }
    }

    pub fn entity(&self) -> Entity {
        self.entity
    }

    pub fn context(&self) -> &Ctx {
        &self.context
    }
}

/// Event emitted whenever we remove a component from the remote world
#[derive(Event)]
pub struct ComponentRemoveEvent<C: Component, Ctx = ()> {
    entity: Entity,
    context: Ctx,

    _marker: PhantomData<C>,
}

impl<C: Component, Ctx> ComponentRemoveEvent<C, Ctx> {
    pub fn new(entity: Entity, context: Ctx) -> Self {
        Self {
            entity,
            context,
            _marker: PhantomData,
        }
    }

    pub fn entity(&self) -> Entity {
        self.entity
    }

    pub fn context(&self) -> &Ctx {
        &self.context
    }
}
