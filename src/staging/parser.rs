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

#[cfg(test)]
mod tests {
    use super::*;

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
