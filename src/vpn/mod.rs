mod config;

use zbus::proxy::SignalStream;
use zbus::zvariant::OwnedObjectPath;
use zbus::{proxy, Connection, Proxy};

pub fn load_profile(uri: &str) -> Result<config::PreparedProfile, String> {
    config::load_profile(uri)
}

#[proxy(
    interface = "net.openvpn.v3.configuration",
    default_service = "net.openvpn.v3.configuration",
    default_path = "/net/openvpn/v3/configuration"
)]
trait ConfigurationManager {
    fn import(
        &self,
        name: &str,
        config_str: &str,
        single_use: bool,
        persistent: bool,
    ) -> zbus::Result<OwnedObjectPath>;
}

#[proxy(
    interface = "net.openvpn.v3.configuration",
    default_service = "net.openvpn.v3.configuration"
)]
trait Configuration {
    fn remove(&self) -> zbus::Result<()>;
}

#[proxy(
    interface = "net.openvpn.v3.sessions",
    default_service = "net.openvpn.v3.sessions",
    default_path = "/net/openvpn/v3/sessions"
)]
trait SessionManager {
    fn new_tunnel(&self, config_path: &OwnedObjectPath) -> zbus::Result<OwnedObjectPath>;
}

#[proxy(
    interface = "net.openvpn.v3.sessions",
    default_service = "net.openvpn.v3.sessions"
)]
trait Session {
    fn ready(&self) -> zbus::Result<()>;
    fn connect(&self) -> zbus::Result<()>;
    fn disconnect(&self) -> zbus::Result<()>;
    fn user_input_queue_get_type_group(&self) -> zbus::Result<Vec<(u32, u32)>>;
    fn user_input_queue_check(&self, typ: u32, group: u32) -> zbus::Result<Vec<u32>>;
    fn user_input_queue_fetch(
        &self,
        typ: u32,
        group: u32,
        id: u32,
    ) -> zbus::Result<(u32, u32, u32, String, String, bool)>;
    fn user_input_provide(&self, typ: u32, group: u32, id: u32, value: &str) -> zbus::Result<()>;
}

#[derive(Clone)]
pub struct Client {
    connection: Connection,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UserInputSlot {
    pub typ: u32,
    pub group: u32,
    pub id: u32,
    pub name: String,
    pub description: String,
    pub hidden: bool,
}

pub struct VpnSession<'a> {
    proxy: SessionProxy<'a>,
    signal_proxy: Proxy<'a>,
    path: String,
}

impl Client {
    pub async fn connect_system_bus() -> Result<Self, String> {
        let connection = Connection::system()
            .await
            .map_err(|error| format!("failed to connect to the system D-Bus: {error}"))?;
        Ok(Self { connection })
    }

    pub async fn import_and_create_session<'a>(
        &'a self,
        name: &str,
        config: &str,
    ) -> Result<VpnSession<'a>, String> {
        let manager = ConfigurationManagerProxy::new(&self.connection)
            .await
            .map_err(|error| format!("failed to create OpenVPN configuration proxy: {error}"))?;
        let config_path = manager
            .import(name, config, true, false)
            .await
            .map_err(|error| format!("failed to import temporary OpenVPN profile: {error}"))?;

        let sessions = match SessionManagerProxy::new(&self.connection).await {
            Ok(proxy) => proxy,
            Err(error) => {
                self.remove_config(&config_path).await;
                return Err(format!(
                    "failed to create OpenVPN session-manager proxy: {error}"
                ));
            }
        };
        let session_path = match sessions.new_tunnel(&config_path).await {
            Ok(path) => path,
            Err(error) => {
                self.remove_config(&config_path).await;
                return Err(format!("failed to create OpenVPN session: {error}"));
            }
        };

        let proxy = SessionProxy::builder(&self.connection)
            .path(session_path.clone())
            .map_err(|error| format!("invalid OpenVPN session path: {error}"))?
            .build()
            .await
            .map_err(|error| format!("failed to create OpenVPN session proxy: {error}"))?;
        let signal_proxy = Proxy::new(
            &self.connection,
            "net.openvpn.v3.sessions",
            session_path.clone(),
            "net.openvpn.v3.sessions",
        )
        .await
        .map_err(|error| format!("failed to create OpenVPN signal proxy: {error}"))?;

        Ok(VpnSession {
            proxy,
            signal_proxy,
            path: session_path.to_string(),
        })
    }

    async fn remove_config(&self, config_path: &OwnedObjectPath) {
        let Ok(builder) = ConfigurationProxy::builder(&self.connection).path(config_path.clone())
        else {
            return;
        };
        if let Ok(config) = builder.build().await {
            let _ = config.remove().await;
        }
    }
}

impl VpnSession<'_> {
    pub fn path(&self) -> &str {
        &self.path
    }

    pub async fn ready(&self) -> Result<(), String> {
        self.proxy
            .ready()
            .await
            .map_err(|error| format!("OpenVPN session is not ready: {error}"))
    }

    pub async fn connect(&self) -> Result<(), String> {
        self.proxy
            .connect()
            .await
            .map_err(|error| format!("failed to start OpenVPN connection: {error}"))
    }

    pub async fn disconnect(&self) -> Result<(), String> {
        self.proxy
            .disconnect()
            .await
            .map_err(|error| format!("failed to disconnect OpenVPN session: {error}"))
    }

    pub async fn receive_attention_required(&self) -> zbus::Result<SignalStream<'_>> {
        self.signal_proxy.receive_signal("AttentionRequired").await
    }

    pub async fn receive_status_change(&self) -> zbus::Result<SignalStream<'_>> {
        self.signal_proxy.receive_signal("StatusChange").await
    }

    pub async fn requested_inputs(&self) -> Result<Vec<UserInputSlot>, String> {
        let type_groups = self
            .proxy
            .user_input_queue_get_type_group()
            .await
            .map_err(|error| format!("failed to inspect OpenVPN input requests: {error}"))?;
        let mut inputs = Vec::new();

        for (typ, group) in type_groups {
            let ids = self
                .proxy
                .user_input_queue_check(typ, group)
                .await
                .map_err(|error| format!("failed to inspect OpenVPN input queue: {error}"))?;
            for id in ids {
                let (typ, group, id, name, description, hidden) = self
                    .proxy
                    .user_input_queue_fetch(typ, group, id)
                    .await
                    .map_err(|error| format!("failed to read OpenVPN input request: {error}"))?;
                inputs.push(UserInputSlot {
                    typ,
                    group,
                    id,
                    name,
                    description,
                    hidden,
                });
            }
        }

        Ok(inputs)
    }

    pub async fn provide_input(&self, input: &UserInputSlot, value: &str) -> Result<(), String> {
        self.proxy
            .user_input_provide(input.typ, input.group, input.id, value)
            .await
            .map_err(|error| format!("failed to submit OpenVPN input: {error}"))
    }
}

pub fn is_session_end_status(major: u32, minor: u32) -> bool {
    (major == 1 && minor == 1)
        || (major == 2 && matches!(minor, 9..=11 | 16))
        || (major == 5 && matches!(minor, 29 | 30))
}

#[cfg(test)]
mod tests {
    use super::is_session_end_status;

    #[test]
    fn recognizes_session_end_statuses() {
        assert!(is_session_end_status(2, 9));
        assert!(is_session_end_status(2, 10));
        assert!(is_session_end_status(2, 11));
        assert!(is_session_end_status(2, 16));
        assert!(is_session_end_status(5, 29));
        assert!(is_session_end_status(5, 30));
        assert!(is_session_end_status(1, 1));
        assert!(!is_session_end_status(2, 7));
        assert!(!is_session_end_status(5, 28));
        assert!(!is_session_end_status(3, 19));
    }
}
