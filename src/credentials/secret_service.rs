use std::collections::HashMap;
use std::path::Path;

use zbus::zvariant::{OwnedObjectPath, OwnedValue, Value};
use zbus::{proxy, Connection};

use super::{payload, VpnCredentials};

#[proxy(
    interface = "org.freedesktop.Secret.Service",
    default_service = "org.freedesktop.secrets",
    default_path = "/org/freedesktop/secrets"
)]
trait SecretService {
    fn open_session(
        &self,
        algorithm: &str,
        input: Value<'_>,
    ) -> zbus::Result<(OwnedValue, OwnedObjectPath)>;

    fn search_items(
        &self,
        attributes: HashMap<&str, &str>,
    ) -> zbus::Result<(Vec<OwnedObjectPath>, Vec<OwnedObjectPath>)>;

    fn read_alias(&self, name: &str) -> zbus::Result<OwnedObjectPath>;

    fn close_session(&self, session: &OwnedObjectPath) -> zbus::Result<()>;
}

#[proxy(
    interface = "org.freedesktop.Secret.Collection",
    default_service = "org.freedesktop.secrets"
)]
trait SecretCollection {
    fn create_item(
        &self,
        properties: HashMap<&str, Value<'_>>,
        secret: (OwnedObjectPath, Vec<u8>, Vec<u8>, &str),
        replace: bool,
    ) -> zbus::Result<(OwnedObjectPath, OwnedObjectPath)>;
}

#[proxy(
    interface = "org.freedesktop.Secret.Item",
    default_service = "org.freedesktop.secrets"
)]
trait SecretItem {
    fn get_secret(
        &self,
        session: &OwnedObjectPath,
    ) -> zbus::Result<(OwnedObjectPath, Vec<u8>, Vec<u8>, String)>;
}

pub(super) async fn lookup(config_uri: &str) -> Result<Option<VpnCredentials>, String> {
    let connection = Connection::session()
        .await
        .map_err(|e| format!("failed to connect to the Secret Service session bus: {e}"))?;
    let service = SecretServiceProxy::new(&connection)
        .await
        .map_err(|e| format!("failed to create Secret Service proxy: {e}"))?;
    let (_, session) = service
        .open_session("plain", Value::from(""))
        .await
        .map_err(|e| format!("failed to open Secret Service session: {e}"))?;

    let result = lookup_in_session(&connection, &service, config_uri, &session).await;
    let _ = service.close_session(&session).await;
    result
}

async fn lookup_in_session(
    connection: &Connection,
    service: &SecretServiceProxy<'_>,
    config_uri: &str,
    session: &OwnedObjectPath,
) -> Result<Option<VpnCredentials>, String> {
    let mut attributes = HashMap::new();
    attributes.insert("application", "nautilus-ovpn");
    attributes.insert("config-uri", config_uri);

    let (unlocked, locked) = service
        .search_items(attributes)
        .await
        .map_err(|e| format!("failed to search Secret Service items: {e}"))?;
    if !locked.is_empty() || unlocked.is_empty() {
        return Ok(None);
    }

    let item = SecretItemProxy::builder(connection)
        .path(unlocked[0].clone())
        .map_err(|e| format!("invalid Secret Service item path: {e}"))?
        .build()
        .await
        .map_err(|e| format!("failed to create Secret Service item proxy: {e}"))?;
    let (_, _, value, _) = item
        .get_secret(session)
        .await
        .map_err(|e| format!("failed to read stored VPN credentials: {e}"))?;
    payload::parse_secret(&value)
}

pub(super) async fn store(
    config_uri: &str,
    config_path: &Path,
    credentials: &VpnCredentials,
) -> Result<(), String> {
    let label = format!(
        "nautilus-ovpn {}",
        config_path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("OpenVPN")
    );
    let connection = Connection::session()
        .await
        .map_err(|e| format!("failed to connect to the Secret Service session bus: {e}"))?;
    let service = SecretServiceProxy::new(&connection)
        .await
        .map_err(|e| format!("failed to create Secret Service proxy: {e}"))?;
    let (_, session) = service
        .open_session("plain", Value::from(""))
        .await
        .map_err(|e| format!("failed to open Secret Service session: {e}"))?;
    let collection = service
        .read_alias("default")
        .await
        .map_err(|e| format!("failed to read Secret Service default collection: {e}"))?;
    let collection = SecretCollectionProxy::builder(&connection)
        .path(collection)
        .map_err(|e| format!("invalid Secret Service collection path: {e}"))?
        .build()
        .await
        .map_err(|e| format!("failed to create Secret Service collection proxy: {e}"))?;

    let mut attributes = HashMap::new();
    attributes.insert("application", "nautilus-ovpn");
    attributes.insert("config-uri", config_uri);
    let mut properties = HashMap::new();
    properties.insert(
        "org.freedesktop.Secret.Item.Label",
        Value::from(label.as_str()),
    );
    properties.insert(
        "org.freedesktop.Secret.Item.Attributes",
        Value::from(attributes),
    );
    let secret = (
        session.clone(),
        Vec::new(),
        format!(
            "{}\n{}\n{}\n{}",
            credentials.username,
            credentials.password,
            credentials.private_key_password,
            credentials.legacy_auth
        )
        .into_bytes(),
        "text/plain",
    );
    collection
        .create_item(properties, secret, true)
        .await
        .map_err(|e| format!("failed to store VPN credentials: {e}"))?;
    let _ = service.close_session(&session).await;

    Ok(())
}
