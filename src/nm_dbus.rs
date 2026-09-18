//! NetworkManager D-Bus integration: registers a volatile, in-memory VPN
//! connection and activates it immediately.

use std::collections::HashMap;

use uuid::Uuid;
use zbus::zvariant::{ObjectPath, OwnedObjectPath, OwnedValue, Value};
use zbus::{proxy, Connection};

/// `NM_SETTINGS_ADD_CONNECTION2_FLAG_IN_MEMORY`, keeps the connection out of
/// `/etc/NetworkManager/system-connections/`.
const ADD_CONNECTION2_FLAG_IN_MEMORY: u32 = 0x2;

#[proxy(
    interface = "org.freedesktop.NetworkManager.Settings",
    default_service = "org.freedesktop.NetworkManager",
    default_path = "/org/freedesktop/NetworkManager/Settings"
)]
trait Settings {
    #[zbus(name = "AddConnection2")]
    fn add_connection2(
        &self,
        settings: HashMap<&str, HashMap<&str, Value<'_>>>,
        flags: u32,
        args: HashMap<&str, Value<'_>>,
    ) -> zbus::Result<(OwnedObjectPath, HashMap<String, OwnedValue>)>;
}

#[proxy(
    interface = "org.freedesktop.NetworkManager",
    default_service = "org.freedesktop.NetworkManager",
    default_path = "/org/freedesktop/NetworkManager"
)]
trait NetworkManager {
    fn activate_connection(
        &self,
        connection: &ObjectPath<'_>,
        device: &ObjectPath<'_>,
        specific_object: &ObjectPath<'_>,
    ) -> zbus::Result<OwnedObjectPath>;
}

/// Registers `config_path` as a volatile OpenVPN connection named
/// `VPN-<session_id>` and activates it right away.
pub async fn activate_vpn(session_id: &str, config_path: &str) -> Result<(), String> {
    let connection = Connection::system()
        .await
        .map_err(|e| format!("failed to connect to the system D-Bus: {e}"))?;

    let settings_proxy = SettingsProxy::new(&connection)
        .await
        .map_err(|e| format!("failed to create Settings proxy: {e}"))?;
    let nm_proxy = NetworkManagerProxy::new(&connection)
        .await
        .map_err(|e| format!("failed to create NetworkManager proxy: {e}"))?;

    let uuid = Uuid::new_v4().to_string();
    let connection_id = format!("VPN-{session_id}");

    let mut connection_section: HashMap<&str, Value<'_>> = HashMap::new();
    connection_section.insert("type", Value::from("vpn"));
    connection_section.insert("id", Value::from(connection_id.as_str()));
    connection_section.insert("uuid", Value::from(uuid.as_str()));
    connection_section.insert("volatile", Value::from(true));

    let mut vpn_data: HashMap<&str, &str> = HashMap::new();
    vpn_data.insert("config", config_path);

    let mut vpn_section: HashMap<&str, Value<'_>> = HashMap::new();
    vpn_section.insert(
        "service-type",
        Value::from("org.freedesktop.NetworkManager.openvpn"),
    );
    vpn_section.insert("data", Value::from(vpn_data));

    let mut settings: HashMap<&str, HashMap<&str, Value<'_>>> = HashMap::new();
    settings.insert("connection", connection_section);
    settings.insert("vpn", vpn_section);

    let (connection_path, _result) = settings_proxy
        .add_connection2(settings, ADD_CONNECTION2_FLAG_IN_MEMORY, HashMap::new())
        .await
        .map_err(|e| format!("AddConnection2 failed: {e}"))?;

    println!("[nautilus-openvpn] registered volatile connection at {connection_path}");

    let root_path =
        ObjectPath::try_from("/").map_err(|e| format!("invalid root object path: {e}"))?;

    let active_path = nm_proxy
        .activate_connection(&connection_path.as_ref(), &root_path, &root_path)
        .await
        .map_err(|e| format!("ActivateConnection failed: {e}"))?;

    println!("[nautilus-openvpn] activated connection at {active_path}");

    Ok(())
}
