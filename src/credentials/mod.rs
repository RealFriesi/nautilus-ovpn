mod payload;
mod prompt;
mod secret_service;

use std::path::Path;

pub struct VpnCredentials {
    pub username: String,
    pub password: String,
}

pub async fn get_credentials(
    config_uri: &str,
    config_path: &Path,
) -> Result<Option<VpnCredentials>, String> {
    if let Some(credentials) = secret_service::lookup(config_uri).await? {
        return Ok(Some(credentials));
    }

    let Some(prompt) = prompt::prompt_for_credentials(config_path)? else {
        return Ok(None);
    };

    if prompt.save {
        secret_service::store(config_uri, config_path, &prompt.credentials).await?;
    }

    Ok(Some(prompt.credentials))
}
