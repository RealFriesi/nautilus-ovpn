//! Minimal, dependency-free parser/patcher for `.ovpn` configuration files.
//!
//! It only performs one job: make sure the OpenSSL 3.x legacy provider
//! directives (`providers legacy default` and `tls-cert-profile legacy`) are
//! present, without touching anything else in the file - including inline
//! `<tag>...</tag>` blocks (certs, keys, tls-auth material, ...) and
//! comments (`#` / `;`).

/// Returns `true` if the trimmed line is a comment.
fn is_comment(line: &str) -> bool {
    let trimmed = line.trim_start();
    trimmed.starts_with('#') || trimmed.starts_with(';')
}

/// Returns the first whitespace-separated token of a line, if any.
fn first_token(line: &str) -> Option<&str> {
    line.trim_start().split_whitespace().next()
}

fn second_token(line: &str) -> Option<&str> {
    line.trim_start().split_whitespace().nth(1)
}

/// Returns `true` if the config asks OpenVPN to prompt for username/password.
///
/// `auth-user-pass` without a filename means credentials must be supplied at
/// activation time. If a filename is present, OpenVPN reads credentials from
/// that file instead.
pub fn requires_auth_user_pass_prompt(content: &str) -> bool {
    let mut in_inline_block = false;

    for raw_line in content.lines() {
        if is_comment(raw_line) {
            continue;
        }

        let trimmed = raw_line.trim();

        if in_inline_block {
            if trimmed.starts_with("</") {
                in_inline_block = false;
            }
            continue;
        }

        if trimmed.starts_with('<') && !trimmed.starts_with("</") {
            if !trimmed.ends_with('>') || trimmed == "<" {
                in_inline_block = true;
            } else if !trimmed.contains('/') {
                in_inline_block = true;
            }
            continue;
        }

        if first_token(trimmed) == Some("auth-user-pass") {
            return match second_token(trimmed) {
                Some(token) => token.starts_with('#') || token.starts_with(';'),
                None => true,
            };
        }
    }

    false
}

/// Patches the content of an `.ovpn` file so that the OpenSSL 3.x legacy
/// provider is enabled, unless it is already configured.
///
/// Inline blocks delimited by `<tag>` / `</tag>` are copied through
/// verbatim (including their contents) so binary-ish PEM data is never
/// inspected or mangled.
pub fn patch_legacy_provider(content: &str) -> String {
    let mut has_providers = false;
    let mut has_tls_cert_profile = false;
    let mut in_inline_block = false;

    for raw_line in content.lines() {
        if is_comment(raw_line) {
            continue;
        }

        let trimmed = raw_line.trim();

        if in_inline_block {
            if trimmed.starts_with("</") {
                in_inline_block = false;
            }
            continue;
        }

        if trimmed.starts_with('<') && !trimmed.starts_with("</") {
            // Inline blocks such as <ca>, <cert>, <key>, <tls-auth>, ...
            if !trimmed.ends_with('>') || trimmed == "<" {
                in_inline_block = true;
            } else if !trimmed.contains('/') {
                // A single-line opening tag like "<ca>" with no closing tag
                // on the same line; treat as start of a block.
                in_inline_block = true;
            }
            continue;
        }

        match first_token(trimmed) {
            Some("providers") => has_providers = true,
            Some("tls-cert-profile") => has_tls_cert_profile = true,
            _ => {}
        }
    }

    if has_providers && has_tls_cert_profile {
        return content.to_string();
    }

    let mut injected = String::new();
    if !has_providers {
        injected.push_str("providers legacy default\n");
    }
    if !has_tls_cert_profile {
        injected.push_str("tls-cert-profile legacy\n");
    }

    format!("{injected}{content}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn injects_missing_directives() {
        let input = "client\nremote vpn.example.com 1194\n<ca>\nBEGIN CERT DATA\n</ca>\n";
        let patched = patch_legacy_provider(input);
        assert!(patched.starts_with("providers legacy default\ntls-cert-profile legacy\n"));
        assert!(patched.contains("<ca>\nBEGIN CERT DATA\n</ca>"));
    }

    #[test]
    fn leaves_existing_directives_untouched() {
        let input = "providers legacy default\ntls-cert-profile legacy\nclient\n";
        let patched = patch_legacy_provider(input);
        assert_eq!(patched, input);
    }

    #[test]
    fn ignores_commented_directives() {
        let input = "# providers legacy default\nclient\n";
        let patched = patch_legacy_provider(input);
        assert!(patched.starts_with("providers legacy default\ntls-cert-profile legacy\n"));
    }

    #[test]
    fn only_injects_the_missing_one() {
        let input = "providers legacy default\nclient\n";
        let patched = patch_legacy_provider(input);
        assert_eq!(
            patched,
            "tls-cert-profile legacy\nproviders legacy default\nclient\n"
        );
    }

    #[test]
    fn detects_auth_user_pass_prompt() {
        assert!(requires_auth_user_pass_prompt("client\nauth-user-pass\n"));
        assert!(requires_auth_user_pass_prompt(
            "client\nauth-user-pass # ask\n"
        ));
    }

    #[test]
    fn ignores_auth_user_pass_file() {
        assert!(!requires_auth_user_pass_prompt(
            "client\nauth-user-pass login.txt\n"
        ));
    }

    #[test]
    fn ignores_auth_user_pass_inside_inline_blocks_and_comments() {
        let input = "# auth-user-pass\n<ca>\nauth-user-pass\n</ca>\nclient\n";
        assert!(!requires_auth_user_pass_prompt(input));
    }
}
