mod payload;
mod prompt;
mod secret_service;

use std::path::Path;

pub struct VpnCredentials {
    pub username: String,
    pub password: String,
    pub private_key_password: String,
    pub legacy_auth: bool,
}

const AUTO_CONNECT_DELAY_SECONDS: i32 = 5;

pub async fn get_credentials(
    config_uri: &str,
    config_path: &Path,
    requires_user_pass: bool,
) -> Result<Option<VpnCredentials>, String> {
    let stored_credentials = secret_service::lookup(config_uri).await?;

    let Some(prompt) = prompt::prompt_for_credentials(
        config_path,
        stored_credentials.as_ref(),
        AUTO_CONNECT_DELAY_SECONDS,
        requires_user_pass,
    )?
    else {
        return Ok(None);
    };

    if prompt.save {
        secret_service::store(config_uri, config_path, &prompt.credentials).await?;
    }

    Ok(Some(prompt.credentials))
}
