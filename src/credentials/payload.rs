use super::VpnCredentials;

pub(super) fn parse_secret(value: &[u8]) -> Result<Option<VpnCredentials>, String> {
    if value.is_empty() {
        return Ok(None);
    }
    let secret = String::from_utf8(value.to_vec())
        .map_err(|e| format!("stored VPN credentials are not valid UTF-8: {e}"))?;
    let Some((username, password)) = secret.trim_end_matches('\n').split_once('\n') else {
        return Ok(None);
    };

    if username.is_empty() || password.is_empty() {
        return Ok(None);
    }

    Ok(Some(VpnCredentials {
        username: username.to_string(),
        password: password.to_string(),
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
