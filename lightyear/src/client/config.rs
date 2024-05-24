//! Defines client-specific configuration options
use bevy::ecs::reflect::ReflectResource;
use bevy::prelude::Resource;
use bevy::reflect::Reflect;
use governor::Quota;
use nonzero_ext::nonzero;

use crate::client::input::native::InputConfig;
use crate::client::interpolation::plugin::InterpolationConfig;
use crate::client::prediction::plugin::PredictionConfig;
use crate::client::sync::SyncConfig;
use crate::connection::client::NetConfig;
use crate::shared::config::{Mode, SharedConfig};
use crate::shared::ping::manager::PingConfig;

#[derive(Clone, Reflect)]
/// Config related to the netcode protocol (abstraction of a connection over raw UDP-like transport)
pub struct NetcodeConfig {
    pub num_disconnect_packets: usize,
    pub keepalive_packet_send_rate: f64,
    /// Set the duration (in seconds) after which the server disconnects a client if they don't hear from them.
    /// This is valid for tokens generated by the server.
    /// The default is 3 seconds. A negative value means no timeout.
    /// This is used when the client generates a `ConnectToken` (with `Authentication::Manual`)
    pub client_timeout_secs: i32,
    /// Set the duration in seconds after which the `ConnectToken` generated by the Client
    /// will expire. Set a negative value for the token to never expire.
    pub token_expire_secs: i32,
}

impl Default for NetcodeConfig {
    fn default() -> Self {
        Self {
            num_disconnect_packets: 10,
            keepalive_packet_send_rate: 1.0 / 10.0,
            client_timeout_secs: 3,
            token_expire_secs: 30,
        }
    }
}

impl NetcodeConfig {
    pub(crate) fn build(&self) -> crate::connection::netcode::ClientConfig<()> {
        crate::connection::netcode::ClientConfig::default()
            .num_disconnect_packets(self.num_disconnect_packets)
            .packet_send_rate(self.keepalive_packet_send_rate)
    }
}

#[derive(Clone, Reflect)]
#[reflect(from_reflect = false)]
pub struct PacketConfig {
    /// After how many multiples of RTT do we consider a packet to be lost?
    ///
    /// The default is 1.5; i.e. after 1.5 times the round trip time, we consider a packet lost if
    /// we haven't received an ACK for it.
    pub nack_rtt_multiple: f32,
    #[reflect(ignore)]
    /// Number of bytes per second that can be sent to the server
    pub send_bandwidth_cap: Quota,
    /// If false, there is no bandwidth cap and all messages are sent as soon as possible
    pub bandwidth_cap_enabled: bool,
}

impl Default for PacketConfig {
    fn default() -> Self {
        Self {
            nack_rtt_multiple: 1.5,
            // 56 KB/s bandwidth cap
            send_bandwidth_cap: Quota::per_second(nonzero!(56000u32)),
            bandwidth_cap_enabled: false,
        }
    }
}

impl PacketConfig {
    pub fn with_send_bandwidth_cap(mut self, send_bandwidth_cap: Quota) -> Self {
        self.send_bandwidth_cap = send_bandwidth_cap;
        self
    }

    pub fn with_send_bandwidth_bytes_per_second_cap(mut self, send_bandwidth_cap: u32) -> Self {
        let cap = send_bandwidth_cap.try_into().unwrap();
        self.send_bandwidth_cap = Quota::per_second(cap).allow_burst(cap);
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

/// The configuration object that lets you create a `ClientPlugin` with the desired settings.
///
/// Most of the fields are optional and have sensible defaults.
/// What is required is:
/// - a [`SharedConfig`] struct that has to be same on the client and the server, and contains some common configuration
/// - a [`NetConfig`] that species the connection type
/// ```rust,ignore
/// let config = ClientConfig {
///    shared: SharedConfig::default(),
///    net: net_config,
///    ..default()
/// };
/// let client = ClientPlugin::new(PluginConfig::new(config, protocol()));
/// ```
#[derive(Resource, Clone, Default, Reflect)]
#[reflect(Resource, from_reflect = false)]
pub struct ClientConfig {
    pub shared: SharedConfig,
    pub packet: PacketConfig,
    pub net: NetConfig,
    pub input: InputConfig,
    pub ping: PingConfig,
    pub sync: SyncConfig,
    pub replication: ReplicationConfig,
    pub prediction: PredictionConfig,
    pub interpolation: InterpolationConfig,
}
