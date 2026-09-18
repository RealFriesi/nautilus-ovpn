fn is_comment(line: &str) -> bool {
    let trimmed = line.trim_start();
    trimmed.starts_with('#') || trimmed.starts_with(';')
}

fn first_token(line: &str) -> Option<&str> {
    line.split_whitespace().next()
}

fn second_token(line: &str) -> Option<&str> {
    line.split_whitespace().nth(1)
}

pub(super) fn requires_auth_user_pass_prompt(content: &str) -> bool {
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
            if !trimmed.ends_with('>') || !trimmed.contains('/') {
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

pub(super) fn patch_legacy_provider(content: &str) -> String {
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
            if !trimmed.ends_with('>') || !trimmed.contains('/') {
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
        assert_eq!(patch_legacy_provider(input), input);
    }

    #[test]
    fn ignores_commented_directives() {
        let input = "# providers legacy default\nclient\n";
        assert!(patch_legacy_provider(input)
            .starts_with("providers legacy default\ntls-cert-profile legacy\n"));
    }

    #[test]
    fn only_injects_the_missing_one() {
        let input = "providers legacy default\nclient\n";
        assert_eq!(
            patch_legacy_provider(input),
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
