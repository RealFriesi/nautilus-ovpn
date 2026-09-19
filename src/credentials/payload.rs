use super::VpnCredentials;

pub(super) fn parse_secret(value: &[u8]) -> Result<Option<VpnCredentials>, String> {
    if value.is_empty() {
        return Ok(None);
    }
    let secret = String::from_utf8(value.to_vec())
        .map_err(|e| format!("stored VPN credentials are not valid UTF-8: {e}"))?;
    let mut fields = secret.trim_end_matches('\n').splitn(4, '\n');
    let Some(username) = fields.next() else {
        return Ok(None);
    };
    let Some(password) = fields.next() else {
        return Ok(None);
    };

    let private_key_password = fields.next();
    let legacy_auth = fields.next();
    if username.is_empty() != password.is_empty() || (username.is_empty() && legacy_auth.is_none())
    {
        return Ok(None);
    }

    Ok(Some(VpnCredentials {
        username: username.to_string(),
        password: password.to_string(),
        private_key_password: private_key_password.unwrap_or_default().to_string(),
        legacy_auth: legacy_auth.map(|value| value == "true").unwrap_or(false),
    }))
}

#[cfg(test)]
mod tests {
    use super::parse_secret;

    #[test]
    fn parses_username_and_password() {
        let credentials = parse_secret(b"alice\ncorrect horse battery staple\n")
            .unwrap()
            .unwrap();

        assert_eq!(credentials.username, "alice");
        assert_eq!(credentials.password, "correct horse battery staple");
        assert!(credentials.private_key_password.is_empty());
        assert!(!credentials.legacy_auth);
    }

    #[test]
    fn parses_options_without_username_or_password() {
        let credentials = parse_secret(b"\n\n\ntrue").unwrap().unwrap();

        assert!(credentials.username.is_empty());
        assert!(credentials.password.is_empty());
        assert!(credentials.private_key_password.is_empty());
        assert!(credentials.legacy_auth);
    }

    #[test]
    fn rejects_empty_or_malformed_payloads() {
        assert!(parse_secret(b"").unwrap().is_none());
        assert!(parse_secret(b"alice").unwrap().is_none());
        assert!(parse_secret(b"\nalice").unwrap().is_none());
        assert!(parse_secret(b"alice\n").unwrap().is_none());
        assert!(parse_secret(&[0xff]).is_err());
    }
}
