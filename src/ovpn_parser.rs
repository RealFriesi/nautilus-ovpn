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
}
