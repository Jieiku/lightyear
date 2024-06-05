//! Defines server-specific configuration options
use bevy::prelude::{Reflect, Resource};
use governor::Quota;
use nonzero_ext::nonzero;
use std::sync::Arc;

use crate::connection::netcode::{Key, PRIVATE_KEY_BYTES};
use crate::connection::server::{
    ConnectionRequestHandler, DefaultConnectionRequestHandler, NetConfig,
};
use crate::shared::config::SharedConfig;
use crate::shared::ping::manager::PingConfig;

#[derive(Debug, Clone)]
pub struct NetcodeConfig {
    pub num_disconnect_packets: usize,
    pub keep_alive_send_rate: f64,
    /// Set the duration (in seconds) after which the server disconnects a client if they don't hear from them.
    /// This is valid for tokens generated by the server.
    /// The default is 3 seconds. A negative value means no timeout.
    pub client_timeout_secs: i32,
    pub protocol_id: u64,
    pub private_key: Key,
    /// A closure that will be used to accept or reject incoming connections
    pub connection_request_handler: Arc<dyn ConnectionRequestHandler>,
}

impl Default for NetcodeConfig {
    fn default() -> Self {
        Self {
            num_disconnect_packets: 10,
            keep_alive_send_rate: 1.0 / 10.0,
            client_timeout_secs: 3,
            protocol_id: 0,
            private_key: [0; PRIVATE_KEY_BYTES],
            connection_request_handler: Arc::new(DefaultConnectionRequestHandler),
        }
    }
}

impl NetcodeConfig {
    pub fn with_protocol_id(mut self, protocol_id: u64) -> Self {
        self.protocol_id = protocol_id;
        self
    }
    pub fn with_key(mut self, key: Key) -> Self {
        self.private_key = key;
        self
    }

    pub fn with_client_timeout_secs(mut self, client_timeout_secs: i32) -> Self {
        self.client_timeout_secs = client_timeout_secs;
        self
    }
}

/// Configuration related to sending packets
#[derive(Clone, Debug)]
pub struct PacketConfig {
    /// After how many multiples of RTT do we consider a packet to be lost?
    ///
    /// The default is 1.5; i.e. after 1.5 times the round trip time, we consider a packet lost if
    /// we haven't received an ACK for it.
    pub nack_rtt_multiple: f32,
    /// Number of bytes per second that can be sent to each client
    pub per_client_send_bandwidth_cap: Quota,
    /// If false, there is no bandwidth cap and all messages are sent as soon as possible
    pub bandwidth_cap_enabled: bool,
}

impl Default for PacketConfig {
    fn default() -> Self {
        Self {
            nack_rtt_multiple: 1.5,
            // 56 KB/s bandwidth cap
            per_client_send_bandwidth_cap: Quota::per_second(nonzero!(56000u32)),
            bandwidth_cap_enabled: false,
        }
    }
}

impl PacketConfig {
    pub fn with_send_bandwidth_cap(mut self, send_bandwidth_cap: Quota) -> Self {
        self.per_client_send_bandwidth_cap = send_bandwidth_cap;
        self
    }

    pub fn with_send_bandwidth_bytes_per_second_cap(mut self, send_bandwidth_cap: u32) -> Self {
        let cap = send_bandwidth_cap.try_into().unwrap();
        self.per_client_send_bandwidth_cap = Quota::per_second(cap).allow_burst(cap);
        self
    }

    pub fn enable_bandwidth_cap(mut self) -> Self {
        self.bandwidth_cap_enabled = true;
        self
    }
}

#[derive(Clone, Debug, Default, Reflect)]
pub struct ReplicationConfig {
    /// By default, we will send all component updates since the last time we sent an update for a given entity.
    /// E.g. if the component was updated at tick 3; we will send the update at tick 3, and then at tick 4,
    /// we won't be sending anything since the component wasn't updated after that.
    ///
    /// This helps save bandwidth, but can cause the client to have delayed eventual consistency in the
    /// case of packet loss.
    ///
    /// If this is set to true, we will instead send all updates since the last time we received an ACK from the client.
    /// E.g. if the component was updated at tick 3; we will send the update at tick 3, and then at tick 4,
    /// we will send the update again even if the component wasn't updated, because we still haven't
    /// received an ACK from the client.
    pub send_updates_since_last_ack: bool,
}

/// Configuration for the server plugin.
///
/// The [`ServerConfig`] is a bevy Resource. You can access it in your systems using `Res<ServerConfig>`.
///
/// You can also modify it while the app is running, and the new values will be used on the next
/// time that the server is started. This can be useful to change some configuration values at runtime.
#[derive(Clone, Debug, Default, Resource)]
pub struct ServerConfig {
    pub shared: SharedConfig,
    /// The server can support multiple transport at the same time (e.g. UDP and WebTransport) so that
    /// clients can connect using the transport they prefer, and still play with each other!
    pub net: Vec<NetConfig>,
    pub packet: PacketConfig,
    pub replication: ReplicationConfig,
    pub ping: PingConfig,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::client::networking::NetworkingState;
    use crate::connection::server::DeniedReason;
    use crate::prelude::ClientId;

    use crate::tests::stepper::{BevyStepper, TEST_CLIENT_ID};
    use bevy::prelude::State;
    use std::fmt::Debug;
    use std::sync::Arc;

    #[derive(Debug, Clone)]
    struct CustomConnectionRequestHandler;

    impl ConnectionRequestHandler for CustomConnectionRequestHandler {
        fn handle_request(&self, client_id: ClientId) -> Option<DeniedReason> {
            if client_id == ClientId::Netcode(TEST_CLIENT_ID) {
                Some(DeniedReason::Custom(
                    "Test client is not allowed to connect".into(),
                ))
            } else {
                None
            }
        }
    }

    #[test]
    fn test_accept_connection_request_fn() {
        let mut stepper = BevyStepper::default();
        stepper.stop();

        // add a hook to handle incoming connection request
        for netconfig in &mut stepper.server_app.world.resource_mut::<ServerConfig>().net {
            // reject connections from the test client
            netconfig.set_connection_request_handler(Arc::new(CustomConnectionRequestHandler));
        }

        // try to connect
        stepper.start();

        // check that the client could not connect
        assert_eq!(
            stepper
                .client_app
                .world
                .resource::<State<NetworkingState>>()
                .get(),
            &NetworkingState::Disconnected
        );
    }
}
