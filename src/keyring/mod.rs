use oo7::{Keyring, Secret};

const APPLICATION_ATTRIBUTE: &str = "nautilus-ovpn";

pub fn profile_hash(contents: &[u8]) -> String {
    format!("xxh3-128:{:032x}", xxhash_rust::xxh3::xxh3_128(contents))
}

pub async fn lookup(profile: &str, slot: &str) -> Result<Option<Vec<u8>>, String> {
    let keyring = Keyring::new()
        .await
        .map_err(|error| format!("failed to open the system keyring: {error}"))?;
    keyring
        .unlock()
        .await
        .map_err(|error| format!("failed to unlock the system keyring: {error}"))?;
    let attributes = vec![
        ("application", APPLICATION_ATTRIBUTE),
        ("profile-hash", profile),
        ("slot", slot),
    ];
    let items = keyring
        .search_items(&attributes)
        .await
        .map_err(|error| format!("failed to search the system keyring: {error}"))?;
    let Some(item) = items.first() else {
        return Ok(None);
    };
    let secret = item
        .secret()
        .await
        .map_err(|error| format!("failed to read a system keyring item: {error}"))?;
    Ok(Some(secret.as_bytes().to_vec()))
}

pub async fn store(profile: &str, slot: &str, label: &str, secret: &[u8]) -> Result<(), String> {
    let keyring = Keyring::new()
        .await
        .map_err(|error| format!("failed to open the system keyring: {error}"))?;
    keyring
        .unlock()
        .await
        .map_err(|error| format!("failed to unlock the system keyring: {error}"))?;
    let attributes = vec![
        ("application", APPLICATION_ATTRIBUTE),
        ("profile-hash", profile),
        ("slot", slot),
    ];
    keyring
        .create_item(label, &attributes, Secret::blob(secret), true)
        .await
        .map_err(|error| format!("failed to store a system keyring item: {error}"))
}

#[cfg(test)]
mod tests {
    use super::profile_hash;

    #[test]
    fn profile_hash_is_stable_and_content_sensitive() {
        assert_eq!(profile_hash(b"profile"), profile_hash(b"profile"));
        assert_ne!(profile_hash(b"profile"), profile_hash(b"changed profile"));
        assert!(profile_hash(b"profile").starts_with("xxh3-128:"));
    }
}
