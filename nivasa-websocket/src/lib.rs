//! # nivasa-websocket
//!
//! Nivasa framework — websocket.
//!
//! This crate currently exposes the bootstrap-facing websocket gateway,
//! adapter, lifecycle, and basic room/namespace trait surfaces. Gateway
//! macros, concrete adapters, and richer runtime hooks land in later slices.

use std::collections::{HashMap, HashSet};
use std::hash::Hash;

/// Minimal public trait for websocket gateway types.
///
/// This stays intentionally small until the richer websocket runtime lands.
/// It gives downstream crates a concrete public abstraction to implement
/// without implying any transport, room, or message-routing behavior yet.
///
/// ```rust,no_run
/// use nivasa_websocket::WebSocketGateway;
///
/// struct ChatGateway;
///
/// fn assert_gateway<T: WebSocketGateway>() {}
///
/// assert_gateway::<ChatGateway>();
/// ```
pub trait WebSocketGateway: Send + Sync + 'static {}

impl<T> WebSocketGateway for T where T: Send + Sync + 'static {}

/// Minimal public trait for pluggable websocket backends.
///
/// This stays as a marker-style abstraction until the concrete transport layer
/// lands. It gives downstream crates a stable public trait to reference
/// without implying any handshake, subscription, or room behavior yet.
///
/// ```rust,no_run
/// use nivasa_websocket::WebSocketAdapter;
///
/// struct InMemoryAdapter;
///
/// fn assert_adapter<T: WebSocketAdapter>() {}
///
/// assert_adapter::<InMemoryAdapter>();
/// ```
pub trait WebSocketAdapter: Send + Sync + 'static {}

impl<T> WebSocketAdapter for T where T: Send + Sync + 'static {}

/// Default websocket adapter backed by `tokio-tungstenite`.
#[derive(Debug, Default, Clone, Copy)]
pub struct DefaultWebSocketAdapter;

impl DefaultWebSocketAdapter {
    /// Create default adapter shell.
    pub const fn new() -> Self {
        Self
    }

    /// Return backend role used by this adapter shell.
    pub fn backend_role(&self) -> tokio_tungstenite::tungstenite::protocol::Role {
        tokio_tungstenite::tungstenite::protocol::Role::Server
    }
}

/// Gateway startup hook.
///
/// Implement on gateway type when need bootstrap callback after runtime init.
///
/// ```rust,no_run
/// use nivasa_websocket::OnGatewayInit;
///
/// struct Gateway;
///
/// impl OnGatewayInit for Gateway {
///     fn on_gateway_init(&self) {
///         // bootstrap hook
///     }
/// }
/// ```
pub trait OnGatewayInit: Send + Sync + 'static {
    /// Run once when websocket runtime starts.
    fn on_gateway_init(&self) {}
}

/// Gateway connect hook.
///
/// Use when need per-client setup after connection accepted.
///
/// ```rust,no_run
/// use nivasa_websocket::OnGatewayConnection;
///
/// struct Gateway;
/// struct Client;
///
/// impl OnGatewayConnection for Gateway {
///     type Client = Client;
///
///     fn on_gateway_connection(&self, _client: &Self::Client) {
///         // per-client hook
///     }
/// }
/// ```
pub trait OnGatewayConnection: Send + Sync + 'static {
    /// Connected client payload.
    type Client: Send + Sync + 'static;

    /// Run when client connects.
    fn on_gateway_connection(&self, _client: &Self::Client) {}
}

/// Gateway disconnect hook.
///
/// Use when need cleanup after client leaves.
///
/// ```rust,no_run
/// use nivasa_websocket::OnGatewayDisconnect;
///
/// struct Gateway;
/// struct Client;
///
/// impl OnGatewayDisconnect for Gateway {
///     type Client = Client;
///
///     fn on_gateway_disconnect(&self, _client: &Self::Client) {
///         // cleanup hook
///     }
/// }
/// ```
pub trait OnGatewayDisconnect: Send + Sync + 'static {
    /// Disconnected client payload.
    type Client: Send + Sync + 'static;

    /// Run when client disconnects.
    fn on_gateway_disconnect(&self, _client: &Self::Client) {}
}

/// In-memory room registry for one namespace.
///
/// ```rust
/// use nivasa_websocket::RoomRegistry;
///
/// let mut rooms = RoomRegistry::new("/chat");
/// assert!(rooms.join("lobby", "client-1"));
/// assert!(rooms.contains("lobby", &"client-1"));
/// ```
#[derive(Debug, Clone)]
pub struct RoomRegistry<ClientId> {
    namespace: String,
    rooms: HashMap<String, HashSet<ClientId>>,
}

impl<ClientId> RoomRegistry<ClientId>
where
    ClientId: Clone + Eq + Hash,
{
    /// Create registry for namespace path.
    pub fn new(namespace: impl Into<String>) -> Self {
        Self {
            namespace: namespace.into(),
            rooms: HashMap::new(),
        }
    }

    /// Return namespace path.
    pub fn namespace(&self) -> &str {
        &self.namespace
    }

    /// Add client to room.
    ///
    /// Returns `true` when membership was new.
    pub fn join(&mut self, room: impl Into<String>, client: ClientId) -> bool {
        self.rooms.entry(room.into()).or_default().insert(client)
    }

    /// Remove client from room.
    ///
    /// Returns `true` when membership existed.
    pub fn leave(&mut self, room: &str, client: &ClientId) -> bool {
        let Some(members) = self.rooms.get_mut(room) else {
            return false;
        };

        let removed = members.remove(client);
        if members.is_empty() {
            self.rooms.remove(room);
        }

        removed
    }

    /// Return `true` when room contains client.
    pub fn contains(&self, room: &str, client: &ClientId) -> bool {
        self.rooms
            .get(room)
            .map(|members| members.contains(client))
            .unwrap_or(false)
    }

    /// Return room members.
    pub fn members(&self, room: &str) -> Vec<ClientId> {
        let Some(members) = self.rooms.get(room) else {
            return Vec::new();
        };

        members.iter().cloned().collect()
    }

    /// Return `true` when registry has no rooms.
    pub fn is_empty(&self) -> bool {
        self.rooms.is_empty()
    }

    /// Return room count.
    pub fn room_count(&self) -> usize {
        self.rooms.len()
    }
}

impl<ClientId> Default for RoomRegistry<ClientId>
where
    ClientId: Clone + Eq + Hash,
{
    fn default() -> Self {
        Self::new("/")
    }
}

/// In-memory namespace registry for room membership.
#[derive(Debug, Clone)]
pub struct NamespaceRegistry<ClientId> {
    namespaces: HashMap<String, RoomRegistry<ClientId>>,
}

/// Scoped room-membership helper for one client and namespace.
#[derive(Debug)]
pub struct ClientRoomMembership<'a, ClientId> {
    registry: &'a mut NamespaceRegistry<ClientId>,
    namespace: String,
    client: ClientId,
}

/// In-memory client event inboxes.
///
/// ```rust
/// use nivasa_websocket::ClientEventRegistry;
///
/// let mut events = ClientEventRegistry::new();
/// let mut client = events.client("client-1");
/// assert_eq!(client.emit("message", "hello"), 1);
/// assert_eq!(events.events_for(&"client-1").len(), 1);
/// ```
#[derive(Debug, Clone)]
pub struct ClientEventRegistry<ClientId> {
    clients: HashMap<ClientId, Vec<(String, String)>>,
}

/// Client-scoped event sink for `client.emit("event", data)`.
#[derive(Debug)]
pub struct ClientEventHandle<'a, ClientId> {
    registry: &'a mut ClientEventRegistry<ClientId>,
    client: ClientId,
}

/// In-memory server broadcast inboxes.
///
/// ```rust
/// use nivasa_websocket::ServerEventRegistry;
///
/// let mut server = ServerEventRegistry::new();
/// server.connect("client-1");
/// server.connect("client-2");
/// assert_eq!(server.server().emit("notice", "hello"), 2);
/// ```
#[derive(Debug, Clone)]
pub struct ServerEventRegistry<ClientId> {
    clients: HashMap<ClientId, Vec<(String, String)>>,
}

/// Server-scoped broadcaster for connected clients.
#[derive(Debug)]
pub struct ServerEventHandle<'a, ClientId> {
    registry: &'a mut ServerEventRegistry<ClientId>,
}

/// In-memory room broadcast inboxes.
///
/// ```rust
/// use nivasa_websocket::RoomEventRegistry;
///
/// let mut rooms = RoomEventRegistry::with_namespace("/chat");
/// rooms.connect("client-1");
/// rooms.connect("client-2");
/// rooms.join("lobby", "client-1");
/// assert_eq!(rooms.to("lobby").emit("notice", "hello"), 1);
/// ```
#[derive(Debug, Clone)]
pub struct RoomEventRegistry<ClientId> {
    namespace: String,
    rooms: NamespaceRegistry<ClientId>,
    clients: HashMap<ClientId, Vec<(String, String)>>,
}

/// Room-scoped broadcaster returned by `server.to("room")`.
#[derive(Debug)]
pub struct RoomEventHandle<'a, ClientId> {
    registry: &'a mut RoomEventRegistry<ClientId>,
    room: String,
}

impl<ClientId> NamespaceRegistry<ClientId>
where
    ClientId: Clone + Eq + Hash,
{
    /// Create empty namespace registry.
    pub fn new() -> Self {
        Self::default()
    }

    /// Return client-scoped room helper for one namespace.
    pub fn client(
        &mut self,
        namespace: impl Into<String>,
        client: ClientId,
    ) -> ClientRoomMembership<'_, ClientId> {
        ClientRoomMembership {
            registry: self,
            namespace: namespace.into(),
            client,
        }
    }

    /// Add client to room inside namespace.
    ///
    /// Returns `true` when membership was new.
    pub fn join(
        &mut self,
        namespace: impl Into<String>,
        room: impl Into<String>,
        client: ClientId,
    ) -> bool {
        let namespace = namespace.into();
        self.namespaces
            .entry(namespace.clone())
            .or_insert_with(|| RoomRegistry::new(namespace))
            .join(room, client)
    }

    /// Remove client from room inside namespace.
    ///
    /// Returns `true` when membership existed.
    pub fn leave(&mut self, namespace: &str, room: &str, client: &ClientId) -> bool {
        let Some(registry) = self.namespaces.get_mut(namespace) else {
            return false;
        };

        let removed = registry.leave(room, client);
        if registry.is_empty() {
            self.namespaces.remove(namespace);
        }

        removed
    }

    /// Remove a client from every room in every namespace.
    pub fn disconnect(&mut self, client: &ClientId) -> bool {
        let mut removed_any = false;

        self.namespaces.retain(|_, registry| {
            let mut namespace_removed = false;

            registry.rooms.retain(|_, members| {
                let removed = members.remove(client);
                namespace_removed |= removed;
                !members.is_empty()
            });

            removed_any |= namespace_removed;
            !registry.is_empty()
        });

        removed_any
    }

    /// Return room members inside namespace.
    pub fn members(&self, namespace: &str, room: &str) -> Vec<ClientId> {
        self.namespaces
            .get(namespace)
            .map(|registry| registry.members(room))
            .unwrap_or_default()
    }

    /// Return `true` when namespace and room contain client.
    pub fn contains(&self, namespace: &str, room: &str, client: &ClientId) -> bool {
        self.namespaces
            .get(namespace)
            .map(|registry| registry.contains(room, client))
            .unwrap_or(false)
    }

    /// Return `true` when namespace exists.
    pub fn has_namespace(&self, namespace: &str) -> bool {
        self.namespaces.contains_key(namespace)
    }
}

impl<ClientId> Default for NamespaceRegistry<ClientId>
where
    ClientId: Clone + Eq + Hash,
{
    fn default() -> Self {
        Self {
            namespaces: HashMap::new(),
        }
    }
}

impl<ClientId> ClientEventRegistry<ClientId>
where
    ClientId: Clone + Eq + Hash,
{
    /// Create empty client event registry.
    pub fn new() -> Self {
        Self::default()
    }

    /// Return client-scoped event handle.
    pub fn client(&mut self, client: ClientId) -> ClientEventHandle<'_, ClientId> {
        ClientEventHandle {
            registry: self,
            client,
        }
    }

    /// Return recorded events for one client.
    pub fn events_for(&self, client: &ClientId) -> Vec<(String, String)> {
        self.clients.get(client).cloned().unwrap_or_default()
    }

    /// Return `true` when no client has events.
    pub fn is_empty(&self) -> bool {
        self.clients.is_empty()
    }
}

impl<ClientId> Default for ClientEventRegistry<ClientId>
where
    ClientId: Clone + Eq + Hash,
{
    fn default() -> Self {
        Self {
            clients: HashMap::new(),
        }
    }
}

impl<ClientId> ServerEventRegistry<ClientId>
where
    ClientId: Clone + Eq + Hash,
{
    /// Create empty server event registry.
    pub fn new() -> Self {
        Self::default()
    }

    /// Register connected client.
    pub fn connect(&mut self, client: ClientId) {
        self.clients.entry(client).or_default();
    }

    /// Disconnect client and drop inbox.
    pub fn disconnect(&mut self, client: &ClientId) -> bool {
        self.clients.remove(client).is_some()
    }

    /// Return server-scoped broadcast handle.
    pub fn server(&mut self) -> ServerEventHandle<'_, ClientId> {
        ServerEventHandle { registry: self }
    }

    /// Return recorded events for one connected client.
    pub fn events_for(&self, client: &ClientId) -> Vec<(String, String)> {
        self.clients.get(client).cloned().unwrap_or_default()
    }

    /// Return connected client ids.
    pub fn connected_clients(&self) -> Vec<ClientId> {
        self.clients.keys().cloned().collect()
    }
}

impl<ClientId> Default for ServerEventRegistry<ClientId>
where
    ClientId: Clone + Eq + Hash,
{
    fn default() -> Self {
        Self {
            clients: HashMap::new(),
        }
    }
}

impl<ClientId> RoomEventRegistry<ClientId>
where
    ClientId: Clone + Eq + Hash,
{
    /// Create empty room-targeted broadcast registry.
    pub fn new() -> Self {
        Self::with_namespace("/")
    }

    /// Create empty room-targeted broadcast registry for one namespace.
    pub fn with_namespace(namespace: impl Into<String>) -> Self {
        Self {
            namespace: namespace.into(),
            rooms: NamespaceRegistry::default(),
            clients: HashMap::new(),
        }
    }

    /// Return namespace path used by this room registry.
    pub fn namespace(&self) -> &str {
        &self.namespace
    }

    /// Register connected client.
    pub fn connect(&mut self, client: ClientId) {
        self.clients.entry(client).or_default();
    }

    /// Remove connected client and room memberships.
    pub fn disconnect(&mut self, client: &ClientId) -> bool {
        let removed_client = self.clients.remove(client).is_some();
        let removed_rooms = self.rooms.disconnect(client);
        removed_client || removed_rooms
    }

    /// Add connected client to room.
    pub fn join(&mut self, room: impl Into<String>, client: ClientId) -> bool {
        self.rooms.join(self.namespace.clone(), room, client)
    }

    /// Return room-scoped broadcast handle.
    pub fn to(&mut self, room: impl Into<String>) -> RoomEventHandle<'_, ClientId> {
        RoomEventHandle {
            registry: self,
            room: room.into(),
        }
    }

    /// Return recorded events for one connected client.
    pub fn events_for(&self, client: &ClientId) -> Vec<(String, String)> {
        self.clients.get(client).cloned().unwrap_or_default()
    }

    /// Return members connected to room.
    pub fn room_members(&self, room: &str) -> Vec<ClientId> {
        self.rooms.members(&self.namespace, room)
    }
}

impl<ClientId> Default for RoomEventRegistry<ClientId>
where
    ClientId: Clone + Eq + Hash,
{
    fn default() -> Self {
        Self::with_namespace("/")
    }
}

impl<'a, ClientId> ClientRoomMembership<'a, ClientId>
where
    ClientId: Clone + Eq + Hash,
{
    /// Add scoped client to room in scoped namespace.
    pub fn join(&mut self, room: impl Into<String>) -> bool {
        self.registry
            .join(self.namespace.clone(), room, self.client.clone())
    }

    /// Remove scoped client from room in scoped namespace.
    pub fn leave(&mut self, room: &str) -> bool {
        self.registry.leave(&self.namespace, room, &self.client)
    }

    /// Return scoped namespace path.
    pub fn namespace(&self) -> &str {
        &self.namespace
    }

    /// Return scoped client identifier.
    pub fn client(&self) -> &ClientId {
        &self.client
    }
}

impl<'a, ClientId> ClientEventHandle<'a, ClientId>
where
    ClientId: Clone + Eq + Hash,
{
    /// Emit one event for scoped client.
    pub fn emit(&mut self, event: impl Into<String>, data: impl Into<String>) -> usize {
        let entry = self
            .registry
            .clients
            .entry(self.client.clone())
            .or_default();
        entry.push((event.into(), data.into()));
        entry.len()
    }

    /// Return scoped client identifier.
    pub fn client(&self) -> &ClientId {
        &self.client
    }
}

impl<'a, ClientId> ServerEventHandle<'a, ClientId>
where
    ClientId: Clone + Eq + Hash,
{
    /// Broadcast one event to every connected client.
    pub fn emit(&mut self, event: impl Into<String>, data: impl Into<String>) -> usize {
        let event = event.into();
        let data = data.into();
        let mut delivered = 0;

        for inbox in self.registry.clients.values_mut() {
            inbox.push((event.clone(), data.clone()));
            delivered += 1;
        }

        delivered
    }
}

impl<'a, ClientId> RoomEventHandle<'a, ClientId>
where
    ClientId: Clone + Eq + Hash,
{
    /// Emit one event to clients in scoped room.
    pub fn emit(&mut self, event: impl Into<String>, data: impl Into<String>) -> usize {
        let event = event.into();
        let data = data.into();
        let members = self
            .registry
            .rooms
            .members(&self.registry.namespace, &self.room);
        let mut delivered = 0;

        for client in members {
            if let Some(inbox) = self.registry.clients.get_mut(&client) {
                inbox.push((event.clone(), data.clone()));
                delivered += 1;
            }
        }

        delivered
    }
}

#[cfg(test)]
mod tests {
    use super::{
        ClientEventRegistry, ClientRoomMembership, DefaultWebSocketAdapter, NamespaceRegistry,
        OnGatewayConnection, OnGatewayDisconnect, OnGatewayInit, RoomEventRegistry, RoomRegistry,
        ServerEventRegistry, WebSocketAdapter, WebSocketGateway,
    };
    use std::sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    };

    struct DemoGateway;
    struct DemoAdapter;
    struct DemoLifecycleGateway {
        initialized: Arc<AtomicBool>,
        connected: Arc<AtomicBool>,
        disconnected: Arc<AtomicBool>,
    }

    #[test]
    fn concrete_types_can_implement_the_gateway_trait() {
        fn assert_gateway<T: WebSocketGateway>() {}

        assert_gateway::<DemoGateway>();
    }

    #[test]
    fn trait_objects_can_reference_gateway_values() {
        let gateway: &(dyn WebSocketGateway + Send + Sync) = &DemoGateway;
        let _ = gateway;
    }

    #[test]
    fn concrete_types_can_implement_the_adapter_trait() {
        fn assert_adapter<T: WebSocketAdapter>() {}

        assert_adapter::<DemoAdapter>();
        assert_adapter::<DefaultWebSocketAdapter>();
    }

    #[test]
    fn default_websocket_adapter_reports_tokio_tungstenite_backend_role() {
        let adapter = DefaultWebSocketAdapter::new();

        assert_eq!(
            adapter.backend_role(),
            tokio_tungstenite::tungstenite::protocol::Role::Server
        );
    }

    #[test]
    fn client_event_registry_keeps_emits_isolated_by_client() {
        let mut registry = ClientEventRegistry::new();

        {
            let mut client = registry.client("client-1");
            assert_eq!(client.client(), &"client-1");
            assert_eq!(client.emit("message", "hello"), 1);
            assert_eq!(client.emit("typing", "on"), 2);
        }

        {
            let mut other_client = registry.client("client-2");
            assert_eq!(other_client.emit("message", "other"), 1);
        }

        assert_eq!(
            registry.events_for(&"client-1"),
            vec![
                ("message".to_string(), "hello".to_string()),
                ("typing".to_string(), "on".to_string()),
            ]
        );
        assert_eq!(
            registry.events_for(&"client-2"),
            vec![("message".to_string(), "other".to_string())]
        );
        assert!(!registry
            .events_for(&"client-1")
            .contains(&("message".to_string(), "other".to_string())));
    }

    #[test]
    fn server_event_registry_broadcasts_to_all_connected_clients() {
        let mut registry = ServerEventRegistry::new();
        registry.connect("client-1");
        registry.connect("client-2");
        registry.connect("client-3");

        let delivered = {
            let mut server = registry.server();
            server.emit("notice", "hello")
        };

        assert_eq!(delivered, 3);
        assert_eq!(
            registry.events_for(&"client-1"),
            vec![("notice".to_string(), "hello".to_string())]
        );
        assert_eq!(
            registry.events_for(&"client-2"),
            vec![("notice".to_string(), "hello".to_string())]
        );
        assert_eq!(
            registry.events_for(&"client-3"),
            vec![("notice".to_string(), "hello".to_string())]
        );
        assert_eq!(registry.connected_clients().len(), 3);
    }

    #[test]
    fn room_event_registry_broadcasts_only_to_room_members() {
        let mut registry = RoomEventRegistry::new();
        registry.connect("client-1");
        registry.connect("client-2");
        registry.connect("client-3");
        registry.join("general", "client-1");
        registry.join("general", "client-3");
        registry.join("private", "client-2");

        let delivered = {
            let mut room = registry.to("general");
            room.emit("notice", "hello room")
        };

        assert_eq!(delivered, 2);
        assert_eq!(
            registry.events_for(&"client-1"),
            vec![("notice".to_string(), "hello room".to_string())]
        );
        assert_eq!(
            registry.events_for(&"client-2"),
            Vec::<(String, String)>::new()
        );
        assert_eq!(
            registry.events_for(&"client-3"),
            vec![("notice".to_string(), "hello room".to_string())]
        );
        assert_eq!(registry.room_members("general").len(), 2);
    }

    #[test]
    fn room_event_registry_tracks_namespace_specific_room_state() {
        let mut registry = RoomEventRegistry::with_namespace("/chat");

        registry.connect("client-1");
        assert!(registry.join("general", "client-1"));

        assert_eq!(registry.namespace(), "/chat");
        assert!(registry.rooms.has_namespace("/chat"));
        assert!(!registry.rooms.has_namespace("/"));
        assert!(registry.rooms.contains("/chat", "general", &"client-1"));
        assert!(!registry.rooms.contains("/", "general", &"client-1"));
    }

    #[test]
    fn room_event_registry_scopes_membership_by_namespace() {
        let mut chat = RoomEventRegistry::with_namespace("/chat");
        let mut admin = RoomEventRegistry::with_namespace("/admin");

        chat.connect("client-1");
        chat.connect("client-2");
        admin.connect("client-9");

        assert_eq!(chat.namespace(), "/chat");
        assert_eq!(admin.namespace(), "/admin");

        assert!(chat.join("general", "client-1"));
        assert!(chat.join("general", "client-2"));
        assert!(admin.join("general", "client-9"));

        let mut chat_members = chat.room_members("general");
        chat_members.sort_unstable();
        assert_eq!(
            chat_members,
            vec!["client-1".to_string(), "client-2".to_string()]
        );
        assert_eq!(admin.room_members("general"), vec!["client-9".to_string()]);

        let delivered = {
            let mut broadcast = chat.to("general");
            broadcast.emit("notice", "hello chat")
        };

        assert_eq!(delivered, 2);
        assert_eq!(
            chat.events_for(&"client-1"),
            vec![("notice".to_string(), "hello chat".to_string())]
        );
        assert_eq!(
            chat.events_for(&"client-2"),
            vec![("notice".to_string(), "hello chat".to_string())]
        );
        assert_eq!(
            admin.events_for(&"client-9"),
            Vec::<(String, String)>::new()
        );
    }

    impl OnGatewayInit for DemoLifecycleGateway {
        fn on_gateway_init(&self) {
            self.initialized.store(true, Ordering::SeqCst);
        }
    }

    impl OnGatewayConnection for DemoLifecycleGateway {
        type Client = &'static str;

        fn on_gateway_connection(&self, client: &Self::Client) {
            assert_eq!(*client, "client-1");
            self.connected.store(true, Ordering::SeqCst);
        }
    }

    impl OnGatewayDisconnect for DemoLifecycleGateway {
        type Client = &'static str;

        fn on_gateway_disconnect(&self, client: &Self::Client) {
            assert_eq!(*client, "client-1");
            self.disconnected.store(true, Ordering::SeqCst);
        }
    }

    #[test]
    fn lifecycle_traits_can_be_implemented_by_gateway_types() {
        fn assert_init<T: OnGatewayInit>() {}
        fn assert_connect<T: OnGatewayConnection<Client = &'static str>>() {}
        fn assert_disconnect<T: OnGatewayDisconnect<Client = &'static str>>() {}

        assert_init::<DemoLifecycleGateway>();
        assert_connect::<DemoLifecycleGateway>();
        assert_disconnect::<DemoLifecycleGateway>();
    }

    #[test]
    fn lifecycle_hooks_can_be_invoked_directly() {
        let gateway = DemoLifecycleGateway {
            initialized: Arc::new(AtomicBool::new(false)),
            connected: Arc::new(AtomicBool::new(false)),
            disconnected: Arc::new(AtomicBool::new(false)),
        };

        gateway.on_gateway_init();
        gateway.on_gateway_connection(&"client-1");
        gateway.on_gateway_disconnect(&"client-1");

        assert!(gateway.initialized.load(Ordering::SeqCst));
        assert!(gateway.connected.load(Ordering::SeqCst));
        assert!(gateway.disconnected.load(Ordering::SeqCst));
    }

    #[test]
    fn room_registry_tracks_membership_in_single_namespace() {
        let mut rooms = RoomRegistry::new("/chat");

        assert_eq!(rooms.namespace(), "/chat");
        assert!(rooms.join("general", "client-1"));
        assert!(!rooms.join("general", "client-1"));
        assert!(rooms.join("general", "client-2"));
        assert!(rooms.contains("general", &"client-1"));
        assert_eq!(rooms.room_count(), 1);

        let mut members = rooms.members("general");
        members.sort_unstable();
        assert_eq!(members, vec!["client-1", "client-2"]);

        assert!(rooms.leave("general", &"client-1"));
        assert!(!rooms.contains("general", &"client-1"));
        assert!(rooms.contains("general", &"client-2"));
    }

    #[test]
    fn namespace_registry_isolates_room_membership_by_namespace() {
        let mut namespaces = NamespaceRegistry::new();

        assert!(namespaces.join("/chat", "general", "client-1"));
        assert!(namespaces.join("/admin", "general", "client-2"));
        assert!(namespaces.has_namespace("/chat"));
        assert!(namespaces.has_namespace("/admin"));

        assert!(namespaces.contains("/chat", "general", &"client-1"));
        assert!(!namespaces.contains("/chat", "general", &"client-2"));
        assert!(namespaces.contains("/admin", "general", &"client-2"));

        assert!(namespaces.leave("/chat", "general", &"client-1"));
        assert!(!namespaces.has_namespace("/chat"));
        assert!(namespaces.has_namespace("/admin"));
    }

    #[test]
    fn disconnection_cleanup_removes_empty_rooms_and_namespaces_only() {
        let mut namespaces = NamespaceRegistry::new();

        assert!(namespaces.join("/chat", "general", "client-1"));
        assert!(namespaces.join("/chat", "general", "client-2"));
        assert!(namespaces.join("/chat", "ops", "client-3"));
        assert!(namespaces.join("/admin", "general", "client-9"));

        assert!(namespaces.leave("/chat", "general", &"client-1"));
        assert!(!namespaces.members("/chat", "general").is_empty());
        assert!(namespaces.has_namespace("/chat"));
        assert!(namespaces.has_namespace("/admin"));

        assert!(namespaces.leave("/chat", "general", &"client-2"));
        assert!(namespaces.members("/chat", "general").is_empty());
        assert!(namespaces.has_namespace("/chat"));
        assert!(namespaces.contains("/chat", "ops", &"client-3"));

        assert!(namespaces.leave("/chat", "ops", &"client-3"));
        assert!(!namespaces.has_namespace("/chat"));

        assert!(namespaces.has_namespace("/admin"));
        assert!(namespaces.contains("/admin", "general", &"client-9"));
    }

    #[test]
    fn room_membership_targets_only_matching_room_and_namespace() {
        let mut namespaces = NamespaceRegistry::new();

        assert!(namespaces.join("/chat", "general", "client-1"));
        assert!(namespaces.join("/chat", "general", "client-2"));
        assert!(namespaces.join("/chat", "ops", "client-3"));
        assert!(namespaces.join("/admin", "general", "client-4"));

        let mut recipients = namespaces.members("/chat", "general");
        recipients.sort_unstable();

        assert_eq!(recipients, vec!["client-1", "client-2"]);
        assert!(!recipients.contains(&"client-3"));
        assert!(!recipients.contains(&"client-4"));
        assert!(namespaces.members("/chat", "missing").is_empty());
    }

    #[test]
    fn client_room_membership_helper_joins_and_leaves_rooms() {
        let mut namespaces = NamespaceRegistry::new();

        {
            let mut client = namespaces.client("/chat", "client-1");
            assert_eq!(client.namespace(), "/chat");
            assert_eq!(client.client(), &"client-1");
            assert!(client.join("general"));
            assert!(!client.join("general"));
            assert!(client.leave("general"));
            assert!(!client.leave("general"));
        }

        assert!(!namespaces.has_namespace("/chat"));
    }

    #[test]
    fn client_room_membership_helper_targets_only_its_namespace_and_client() {
        let mut namespaces = NamespaceRegistry::new();
        namespaces.join("/admin", "general", "client-9");

        {
            let mut client = namespaces.client("/chat", "client-1");
            assert!(client.join("general"));
            assert!(client.join("ops"));
        }

        assert!(namespaces.contains("/chat", "general", &"client-1"));
        assert!(namespaces.contains("/chat", "ops", &"client-1"));
        assert!(!namespaces.contains("/chat", "general", &"client-9"));
        assert!(namespaces.contains("/admin", "general", &"client-9"));
    }

    #[test]
    fn client_room_membership_helper_type_is_publicly_constructible_from_registry() {
        fn assert_helper_type<T>(_value: &T) {}

        let mut namespaces = NamespaceRegistry::new();
        let client: ClientRoomMembership<'_, &'static str> = namespaces.client("/chat", "client-1");
        assert_helper_type(&client);
    }
}
