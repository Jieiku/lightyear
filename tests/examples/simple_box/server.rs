use crate::protocol::*;
use crate::shared::{shared_config, shared_movement_behaviour};
use crate::{shared, KEY, PROTOCOL_ID};
use bevy::prelude::*;
use lightyear_shared::plugin::events::InputEvent;
use lightyear_shared::plugin::sets::FixedUpdateSet;
use lightyear_shared::prelude::*;
use lightyear_shared::server::{NetcodeConfig, PingConfig, Server, ServerConfig};
use lightyear_shared::{ConnectEvent, DisconnectEvent, IoConfig, TransportConfig};
use std::collections::HashMap;
use std::net::{Ipv4Addr, SocketAddr};

pub struct ServerPlugin {
    pub(crate) port: u16,
}

impl Plugin for ServerPlugin {
    fn build(&self, app: &mut App) {
        let server_addr = SocketAddr::new(Ipv4Addr::LOCALHOST.into(), self.port);
        let netcode_config = NetcodeConfig::default()
            .with_protocol_id(PROTOCOL_ID)
            .with_key(KEY);
        let config = ServerConfig {
            shared: shared_config().clone(),
            netcode: netcode_config,
            io: IoConfig::from_transport(TransportConfig::UdpSocket(server_addr)),
            ping: PingConfig::default(),
        };
        let plugin_config =
            lightyear_shared::server::PluginConfig::new(config, MyProtocol::default());
        app.add_plugins(lightyear_shared::server::Plugin::new(plugin_config));
        app.add_plugins(shared::SharedPlugin);
        app.init_resource::<Global>();
        app.add_systems(Startup, init);
        // the physics/FixedUpdates systems that consume inputs should be run in this set
        app.add_systems(FixedUpdate, movement.in_set(FixedUpdateSet::Main));
        app.add_systems(Update, (handle_connections, send_message));
    }
}

#[derive(Resource, Default)]
pub struct Global {
    pub client_id_to_entity_id: HashMap<ClientId, Entity>,
}

pub(crate) fn init(mut commands: Commands) {
    commands.spawn(Camera2dBundle::default());
    commands.spawn(TextBundle::from_section(
        "Server",
        TextStyle {
            font_size: 30.0,
            color: Color::WHITE,
            ..default()
        },
    ));
}

/// Server connection system, create a player upon connection
pub(crate) fn handle_connections(
    // TODO: give type alias to ConnectionEvents<ClientId> ? (such as ServerConnectionEvents)?
    mut connections: EventReader<ConnectEvent<ClientId>>,
    mut disconnections: EventReader<DisconnectEvent<ClientId>>,
    mut global: ResMut<Global>,
    mut commands: Commands,
) {
    for connection in connections.iter() {
        let client_id = connection.context();
        info!("New connection from client: {:?}", client_id);
        // Generate pseudo random color from client id.
        let r = ((client_id % 23) as f32) / 23.0;
        let g = ((client_id % 27) as f32) / 27.0;
        let b = ((client_id % 39) as f32) / 39.0;
        let entity = commands.spawn(PlayerBundle::new(
            *client_id,
            Vec2::ZERO,
            Color::rgb(r, g, b),
        ));
        // Add a mapping from client id to entity id
        global
            .client_id_to_entity_id
            .insert(*client_id, entity.id());
    }
    for disconnection in disconnections.iter() {
        info!("Client disconnected: {:?}", disconnection.context());
    }
}

/// Read client inputs and move players
pub(crate) fn movement(
    mut position_query: Query<&mut PlayerPosition>,
    mut input_reader: EventReader<InputEvent<Inputs, ClientId>>,
    global: Res<Global>,
    server: Res<Server<MyProtocol>>,
) {
    for input in input_reader.read() {
        let client_id = input.context();
        if input.input().is_some() {
            let input = input.input().as_ref().unwrap();
            info!("Receiving input: {:?} on tick: {:?}", input, server.tick());
            // TODO: on the server-side maintain a map from client_id to entity_id
            if let Some(player_entity) = global.client_id_to_entity_id.get(client_id) {
                if let Ok(mut position) = position_query.get_mut(*player_entity) {
                    shared_movement_behaviour(&mut position, input);
                }
            }
        }
    }
}

/// Send messages from server to clients
pub(crate) fn send_message(mut server: ResMut<Server<MyProtocol>>, input: Res<Input<KeyCode>>) {
    if input.pressed(KeyCode::M) {
        // TODO: add way to send message to all
        let message = Message1(5);
        info!("Send message: {:?}", message);
        server.broadcast_send::<Channel1, Message1>(Message1(5));
    }
}
