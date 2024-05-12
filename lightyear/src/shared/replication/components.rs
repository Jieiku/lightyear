//! Components used for replication
use bevy::ecs::entity::MapEntities;
use bevy::ecs::query::QueryFilter;
use bevy::ecs::system::SystemParam;
use bevy::prelude::{Bundle, Component, Entity, EntityMapper, Or, Query, Reflect, With};
use bevy::utils::{HashMap, HashSet};
use serde::{Deserialize, Serialize};
use tracing::trace;

use bitcode::{Decode, Encode};

use crate::channel::builder::Channel;
use crate::client::components::SyncComponent;
use crate::connection::id::ClientId;
use crate::prelude::ParentSync;
use crate::protocol::component::{ComponentKind, ComponentNetId, ComponentRegistry};
use crate::server::visibility::immediate::{ClientVisibility, VisibilityManager};
use crate::shared::replication::network_target::NetworkTarget;

/// Marker component that indicates that the entity was spawned via replication
/// (it is being replicated from a remote world)
#[derive(Component, Serialize, Deserialize, Clone, Debug, PartialEq, Reflect)]
#[component(storage = "SparseSet")]
pub struct Replicated;

/// Component inserted to each replicable entities, to detect when they are despawned
#[derive(Component, Clone, Copy)]
#[component(storage = "SparseSet")]
pub(crate) struct DespawnTracker;

/// Marker component to indicate that the entity is under the control of the local peer
#[derive(Component, Clone, Copy, PartialEq, Debug, Reflect, Serialize, Deserialize)]
#[component(storage = "SparseSet")]
pub struct Controlled;

/// Component that indicates that an entity should be replicated. Added to the entity when it is spawned
/// in the world that sends replication updates.
#[derive(Bundle, Clone, PartialEq, Debug, Reflect)]
pub struct Replicate {
    /// Which clients should this entity be replicated to
    pub replication_target: ReplicationTarget,
    /// Which client(s) control this entity?
    pub controlled_by: ControlledBy,
    /// How do we control the visibility of the entity?
    pub visibility: VisibilityMode,
    /// The replication group defines how entities are grouped (sent as a single message) for replication.
    ///
    /// After the entity is first replicated, the replication group of the entity should not be modified.
    /// (but more entities can be added to the replication group)
    // TODO: currently, if the host removes Replicate, then the entity is not removed in the remote
    //  it just keeps living but doesn't receive any updates. Should we make this configurable?
    pub group: ReplicationGroup,
    /// How should the hierarchy of the entity (parents/children) be replicated?
    pub hierarchy: ReplicateHierarchy,
    // // TODO: could it be dangerous to use component kind here? (because the value could vary between rust versions)
    // //  should be ok, because this is not networked
    // /// Lets you override the replication modalities for a specific component
    // #[reflect(ignore)]
    // pub per_component_metadata: HashMap<ComponentKind, PerComponentReplicationMetadata>,
}

#[derive(SystemParam)]
struct ReplicateSystemParam<'w, 's> {
    query: Query<
        'w,
        's,
        (
            &'static ReplicationTarget,
            &'static ControlledBy,
            &'static VisibilityMode,
            &'static ReplicationGroup,
            &'static ReplicateHierarchy,
            &'static TargetEntity,
        ),
    >,
}

/// Component that indicates which clients the entity should be replicated to.
#[derive(Component, Clone, Debug, PartialEq, Reflect)]
pub struct ReplicationTarget {
    /// Which clients should this entity be replicated to
    pub replication: NetworkTarget,
    /// Which clients should predict this entity (unused for client to server replication)
    pub prediction: NetworkTarget,
    /// Which clients should interpolate this entity (unused for client to server replication)
    pub interpolation: NetworkTarget,
}

impl Default for ReplicationTarget {
    fn default() -> Self {
        Self {
            replication: NetworkTarget::All,
            prediction: NetworkTarget::None,
            interpolation: NetworkTarget::None,
        }
    }
}

/// Component storing metadata about which clients have control over the entity
///
/// This is only used for server to client replication.
#[derive(Component, Clone, Debug, Default, PartialEq, Reflect)]
pub struct ControlledBy {
    /// Which client(s) control this entity?
    pub target: NetworkTarget,
}

impl ControlledBy {
    /// Returns true if the entity is controlled by the specified client
    pub fn targets(&self, client_id: &ClientId) -> bool {
        self.target.targets(client_id)
    }
}

/// Component to have more fine-grained control over the visibility of an entity
/// (which clients do we replicate this entity to?)
///
/// This has no effect for client to server replication.
#[derive(Component, Clone, Debug, PartialEq, Reflect)]
pub struct Visibility {
    /// Control if we do fine-grained or coarse-grained visibility
    mode: VisibilityMode,
    // TODO: should we store the visibility cache here if visibility_mode = InterestManagement?
}

/// Defines the target entity for the replication.
///
/// This can be used if you want to replicate this entity on an entity that already
/// exists in the remote world.
///
/// This component is not part of the `Replicate` bundle as this is very infrequent.
#[derive(Component, Default, Clone, Copy, Debug, PartialEq, Reflect)]
pub enum TargetEntity {
    /// Spawn a new entity on the remote peer
    #[default]
    Spawn,
    /// Instead of spawning a new entity, we will apply the replication updates
    /// to the existing remote entity
    Preexisting(Entity),
}

/// Component that defines how the hierarchy of an entity (parent/children) should be replicated
#[derive(Component, Clone, Copy, Debug, PartialEq, Reflect)]
pub struct ReplicateHierarchy {
    /// If true, recursively add `Replicate` and `ParentSync` components to all children to make sure they are replicated
    /// If false, you can still replicate hierarchies, but in a more fine-grained manner. You will have to add the `Replicate`
    /// and `ParentSync` components to the children yourself
    pub recursive: bool,
}

impl Default for ReplicateHierarchy {
    fn default() -> Self {
        Self { recursive: true }
    }
}

/// This lets you specify how to customize the replication behaviour for a given component
#[derive(Clone, Debug, PartialEq, Reflect)]
pub struct PerComponentReplicationMetadata<C> {
    /// If true, do not replicate the component. (By default, all components of this entity that are present in the
    /// [`ComponentRegistry`] will be replicated.
    disabled: bool,
    /// If true, replicate only inserts/removals of the component, not the updates.
    /// (i.e. the component will only get replicated once at spawn)
    /// This is useful for components such as `ActionState`, which should only be replicated once
    replicate_once: bool,
    /// Custom replication target for this component. We will replicate to the intersection of
    /// the entity's replication target and this target
    target: NetworkTarget,
    _marker: std::marker::PhantomData<C>,
}
impl<C> Default for PerComponentReplicationMetadata<C> {
    fn default() -> Self {
        Self {
            disabled: false,
            replicate_once: false,
            target: NetworkTarget::All,
            _marker: Default::default(),
        }
    }
}

impl Replicate {
    pub(crate) fn group_id(&self, entity: Option<Entity>) -> ReplicationGroupId {
        self.group.group_id(entity)
    }

    /// Returns true if the entity is controlled by the specified client
    pub fn is_controlled_by(&self, client_id: &ClientId) -> bool {
        self.controlled_by.targets(client_id)
    }
    //
    // /// Returns true if we don't want to replicate the component
    // pub fn is_disabled<C: Component>(&self) -> bool {
    //     let kind = ComponentKind::of::<C>();
    //     self.per_component_metadata
    //         .get(&kind)
    //         .is_some_and(|metadata| metadata.disabled)
    // }
    //
    // /// If true, the component will be replicated only once, when the entity is spawned.
    // /// We do not replicate component updates
    // pub fn is_replicate_once<C: Component>(&self) -> bool {
    //     let kind = ComponentKind::of::<C>();
    //     self.per_component_metadata
    //         .get(&kind)
    //         .is_some_and(|metadata| metadata.replicate_once)
    // }
    //
    // /// Replication target for this specific component
    // /// This will be the intersection of the provided `entity_target`, and the `target` of the component
    // /// if it exists
    // pub fn target<C: Component>(&self, entity_target: NetworkTarget) -> NetworkTarget {
    //     let kind = ComponentKind::of::<C>();
    //     match self.per_component_metadata.get(&kind) {
    //         None => entity_target,
    //         Some(metadata) => {
    //             let target = metadata.target.clone();
    //             trace!(
    //                 ?kind,
    //                 "replication target override for component {:?}: {target:?}",
    //                 std::any::type_name::<C>()
    //             );
    //             target
    //         }
    //     }
    // }
    //
    // /// Disable the replication of a component for this entity
    // pub fn disable_component<C: Component>(&mut self) {
    //     let kind = ComponentKind::of::<C>();
    //     self.per_component_metadata
    //         .entry(kind)
    //         .or_default()
    //         .disabled = true;
    // }
    //
    // /// Enable the replication of a component for this entity
    // pub fn enable_component<C: Component>(&mut self) {
    //     let kind = ComponentKind::of::<C>();
    //     self.per_component_metadata
    //         .entry(kind)
    //         .or_default()
    //         .disabled = false;
    //     // if we are back at the default, remove the entry
    //     if self.per_component_metadata.get(&kind).unwrap()
    //         == &PerComponentReplicationMetadata::default()
    //     {
    //         self.per_component_metadata.remove(&kind);
    //     }
    // }
    //
    // pub fn enable_replicate_once<C: Component>(&mut self) {
    //     let kind = ComponentKind::of::<C>();
    //     self.per_component_metadata
    //         .entry(kind)
    //         .or_default()
    //         .replicate_once = true;
    // }
    //
    // pub fn disable_replicate_once<C: Component>(&mut self) {
    //     let kind = ComponentKind::of::<C>();
    //     self.per_component_metadata
    //         .entry(kind)
    //         .or_default()
    //         .replicate_once = false;
    //     // if we are back at the default, remove the entry
    //     if self.per_component_metadata.get(&kind).unwrap()
    //         == &PerComponentReplicationMetadata::default()
    //     {
    //         self.per_component_metadata.remove(&kind);
    //     }
    // }
    //
    // pub fn add_target<C: Component>(&mut self, target: NetworkTarget) {
    //     let kind = ComponentKind::of::<C>();
    //     self.per_component_metadata.entry(kind).or_default().target = target;
    //     // if we are back at the default, remove the entry
    //     if self.per_component_metadata.get(&kind).unwrap()
    //         == &PerComponentReplicationMetadata::default()
    //     {
    //         self.per_component_metadata.remove(&kind);
    //     }
    // }
}

#[derive(Debug, Default, Copy, Clone, PartialEq, Reflect)]
pub enum ReplicationGroupIdBuilder {
    // the group id is the entity id
    #[default]
    FromEntity,
    // choose a different group id
    // note: it must not be the same as any entity id!
    // TODO: how can i generate one that doesn't conflict with an existing entity? maybe take u32 as input, and apply generation = u32::MAX - 1?
    //  or reserver some entities on the sender world?
    Group(u64),
}

/// Component to specify the replication group of an entity
///
/// If multiple entities are part of the same replication group, they will be sent together in the same message.
/// It is guaranteed that these entities will be updated at the same time on the remote world.
#[derive(Component, Debug, Copy, Clone, PartialEq, Reflect)]
pub struct ReplicationGroup {
    id_builder: ReplicationGroupIdBuilder,
    /// the priority of the accumulation group
    /// (priority will get reset to this value every time a message gets sent successfully)
    base_priority: f32,
}

impl Default for ReplicationGroup {
    fn default() -> Self {
        Self {
            id_builder: ReplicationGroupIdBuilder::FromEntity,
            base_priority: 1.0,
        }
    }
}

impl ReplicationGroup {
    pub const fn new_from_entity() -> Self {
        Self {
            id_builder: ReplicationGroupIdBuilder::FromEntity,
            base_priority: 1.0,
        }
    }

    pub const fn new_id(id: u64) -> Self {
        Self {
            id_builder: ReplicationGroupIdBuilder::Group(id),
            base_priority: 1.0,
        }
    }

    pub(crate) fn group_id(&self, entity: Option<Entity>) -> ReplicationGroupId {
        match self.id_builder {
            ReplicationGroupIdBuilder::FromEntity => {
                ReplicationGroupId(entity.expect("need to provide an entity").to_bits())
            }
            ReplicationGroupIdBuilder::Group(id) => ReplicationGroupId(id),
        }
    }

    pub(crate) fn priority(&self) -> f32 {
        self.base_priority
    }

    pub fn set_priority(mut self, priority: f32) -> Self {
        self.base_priority = priority;
        self
    }

    pub fn set_id(mut self, id: u64) -> Self {
        self.id_builder = ReplicationGroupIdBuilder::Group(id);
        self
    }
}

#[derive(
    Default,
    Clone,
    Copy,
    Debug,
    PartialEq,
    Eq,
    Hash,
    Serialize,
    Deserialize,
    Reflect,
    Encode,
    Decode,
)]
pub struct ReplicationGroupId(pub u64);

#[derive(Component, Clone, Copy, Default, Debug, PartialEq, Reflect)]
pub enum VisibilityMode {
    /// We will replicate this entity to the clients specified in the `replication_target`.
    /// On top of that, we will apply interest management logic to determine which clients should receive the entity
    ///
    /// You can use [`gain_visibility`](VisibilityManager::gain_visibility) and [`lose_visibility`](VisibilityManager::lose_visibility)
    /// to control the visibility of entities.
    /// You can also use the [`RoomManager`](crate::prelude::server::RoomManager)
    ///
    /// (the client still needs to be included in the [`NetworkTarget`], the room is simply an additional constraint)
    InterestManagement,
    /// We will replicate this entity to the client specified in the `replication_target`, without
    /// running any additional interest management logic
    #[default]
    All,
}

impl Default for Replicate {
    fn default() -> Self {
        #[allow(unused_mut)]
        let mut replicate = Self {
            replication_target: ReplicationTarget::default(),
            controlled_by: ControlledBy::default(),
            visibility: VisibilityMode::default(),
            group: ReplicationGroup::default(),
            hierarchy: ReplicateHierarchy::default(),
        };
        // // TODO: what's the point in replicating them once since they don't change?
        // //  or is it because they are removed and we don't want to replicate the removal?
        // // those metadata components should only be replicated once
        // replicate.enable_replicate_once::<ShouldBePredicted>();
        // replicate.enable_replicate_once::<ShouldBeInterpolated>();
        // cfg_if! {
        //     // the ActionState components are replicated only once when the entity is spawned
        //     // then they get updated by the user inputs, not by replication!
        //     if #[cfg(feature = "leafwing")] {
        //         use leafwing_input_manager::prelude::ActionState;
        //         replicate.enable_replicate_once::<ActionState<P::LeafwingInput1>>();
        //         replicate.enable_replicate_once::<ActionState<P::LeafwingInput2>>();
        //     }
        // }
        replicate
    }
}

/// Marker component that tells the client to spawn an Interpolated entity
#[derive(Component, Serialize, Deserialize, Clone, Debug, PartialEq, Reflect)]
#[component(storage = "SparseSet")]
pub struct ShouldBeInterpolated;

/// Indicates that an entity was pre-predicted
// NOTE: we do not map entities for this component, we want to receive the entities as is
//  because we already do the mapping at other steps
#[derive(Component, Serialize, Deserialize, Clone, Debug, Default, PartialEq, Reflect)]
#[component(storage = "SparseSet")]
pub struct PrePredicted {
    // if this is set, the predicted entity has been pre-spawned on the client
    pub(crate) client_entity: Option<Entity>,
}

/// Marker component that tells the client to spawn a Predicted entity
#[derive(Component, Serialize, Deserialize, Clone, Debug, Default, PartialEq, Reflect)]
#[component(storage = "SparseSet")]
pub struct ShouldBePredicted;
