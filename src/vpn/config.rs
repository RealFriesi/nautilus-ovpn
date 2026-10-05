use std::path::{Component, Path};

use base64::Engine as _;
use gio::prelude::*;

const FILE_DIRECTIVES: &[&str] = &[
    "ca",
    "cert",
    "key",
    "dh",
    "extra-certs",
    "tls-auth",
    "tls-crypt",
    "tls-crypt-v2",
    "pkcs12",
    "crl-verify",
    "secret",
];

pub struct PreparedProfile {
    pub display_name: String,
    pub profile_hash: String,
    pub payload: String,
}

pub fn load_profile(uri: &str) -> Result<PreparedProfile, String> {
    let source = gio::File::for_uri(uri);
    let filename = source
        .basename()
        .ok_or_else(|| "could not determine selected OpenVPN profile name".to_string())?;
    let parent = source
        .parent()
        .ok_or_else(|| "selected OpenVPN profile has no parent directory".to_string())?;
    let source_bytes = source
        .load_contents(gio::Cancellable::NONE)
        .map_err(|error| format!("failed to read selected OpenVPN profile: {error}"))?
        .0;
    let source_contents = std::str::from_utf8(source_bytes.as_ref())
        .map_err(|error| format!("selected OpenVPN profile is not valid UTF-8: {error}"))?;
    let display_name = profile_display_name(source.path().as_deref(), &filename.to_string_lossy());
    let payload = inline_referenced_files(source_contents, &parent)?;

    Ok(PreparedProfile {
        display_name,
        profile_hash: crate::keyring::profile_hash(source_bytes.as_ref()),
        payload,
    })
}

fn profile_display_name(source_path: Option<&Path>, source_name: &str) -> String {
    let fallback = Path::new(source_name)
        .file_stem()
        .and_then(|name| name.to_str())
        .unwrap_or(source_name);
    source_path
        .and_then(Path::parent)
        .filter(|parent| *parent != Path::new("/"))
        .and_then(Path::file_name)
        .and_then(|name| name.to_str())
        .filter(|name| !name.is_empty())
        .unwrap_or(fallback)
        .to_string()
}

fn inline_referenced_files(contents: &str, parent: &gio::File) -> Result<String, String> {
    let mut output = String::new();
    let mut in_inline_block = false;

    for raw_line in contents.lines() {
        let trimmed = raw_line.trim();
        if in_inline_block {
            output.push_str(raw_line);
            output.push('\n');
            if trimmed.starts_with("</") {
                in_inline_block = false;
            }
            continue;
        }
        if trimmed.is_empty() || is_comment(raw_line) {
            output.push_str(raw_line);
            output.push('\n');
            continue;
        }
        if trimmed.starts_with('<') && !trimmed.starts_with("</") {
            in_inline_block = !trimmed.contains("</");
            output.push_str(raw_line);
            output.push('\n');
            continue;
        }

        let tokens = words(trimmed)?;
        let Some(directive) = tokens.first().map(String::as_str) else {
            output.push_str(raw_line);
            output.push('\n');
            continue;
        };
        let indent = &raw_line[..raw_line.len() - raw_line.trim_start().len()];

        if matches!(directive, "auth-user-pass" | "askpass") {
            if tokens
                .get(1)
                .is_some_and(|argument| !argument.starts_with('#') && !argument.starts_with(';'))
            {
                output.push_str(indent);
                output.push_str(directive);
                output.push('\n');
            } else {
                output.push_str(raw_line);
                output.push('\n');
            }
            continue;
        }
        if !FILE_DIRECTIVES.contains(&directive) {
            output.push_str(raw_line);
            output.push('\n');
            continue;
        }

        let Some(filename) = tokens.get(1) else {
            output.push_str(raw_line);
            output.push('\n');
            continue;
        };
        if filename.starts_with('#') || filename.starts_with(';') {
            output.push_str(raw_line);
            output.push('\n');
            continue;
        }
        if directive == "crl-verify" && tokens.get(2).is_some_and(|option| option == "dir") {
            return Err("crl-verify directory mode cannot be inlined".to_string());
        }

        let path = safe_relative_path(filename)?;
        let file = parent.resolve_relative_path(path);
        let bytes = file
            .load_contents(gio::Cancellable::NONE)
            .map_err(|error| format!("failed to read profile companion {path:?}: {error}"))?
            .0;
        let data = if directive == "pkcs12" {
            base64::engine::general_purpose::STANDARD.encode(bytes.as_ref())
        } else {
            std::str::from_utf8(bytes.as_ref())
                .map_err(|error| format!("profile companion {path:?} is not valid UTF-8: {error}"))?
                .to_string()
        };

        output.push_str(indent);
        output.push('<');
        output.push_str(directive);
        output.push_str(">\n");
        output.push_str(data.trim_end_matches('\n'));
        output.push_str("\n</");
        output.push_str(directive);
        output.push_str(">\n");

        if directive == "tls-auth" {
            if let Some(direction) = tokens.get(2).filter(|value| *value == "0" || *value == "1") {
                output.push_str(indent);
                output.push_str("key-direction ");
                output.push_str(direction);
                output.push('\n');
            }
        }
    }

    Ok(output)
}

fn is_comment(line: &str) -> bool {
    let trimmed = line.trim_start();
    trimmed.starts_with('#') || trimmed.starts_with(';')
}

fn words(line: &str) -> Result<Vec<String>, String> {
    let mut result = Vec::new();
    let mut word = String::new();
    let mut quote = None;
    let mut escaped = false;
    let mut active = false;

    for character in line.chars() {
        if escaped {
            word.push(character);
            escaped = false;
            active = true;
        } else if character == '\\' && quote != Some('\'') {
            escaped = true;
            active = true;
        } else if quote == Some(character) {
            quote = None;
        } else if quote.is_none() && matches!(character, '\'' | '"') {
            quote = Some(character);
            active = true;
        } else if quote.is_none() && character.is_whitespace() {
            if active {
                result.push(std::mem::take(&mut word));
                active = false;
            }
        } else {
            word.push(character);
            active = true;
        }
    }

    if escaped || quote.is_some() {
        return Err("malformed quoting in OpenVPN directive".to_string());
    }
    if active {
        result.push(word);
    }
    Ok(result)
}

fn safe_relative_path(path: &str) -> Result<&Path, String> {
    let path = Path::new(path);
    if path.is_absolute()
        || path
            .components()
            .any(|component| !matches!(component, Component::Normal(_) | Component::CurDir))
    {
        return Err(format!(
            "OpenVPN dependency must be relative to the selected profile: {path:?}"
        ));
    }
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::{inline_referenced_files, profile_display_name};
    use std::fs;
    use std::path::Path;

    #[test]
    fn inlines_companions_from_source_and_requests_credentials_over_dbus() {
        let profile_dir =
            std::env::temp_dir().join(format!("nautilus-ovpn-profile-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(profile_dir.join("certs")).expect("create source directory");
        fs::write(profile_dir.join("certs/ca.pem"), "certificate\n").expect("write certificate");
        fs::write(profile_dir.join("identity.p12"), [0, 1, 2, 3]).expect("write PKCS#12");
        let parent = gio::File::for_path(&profile_dir);

        let result = inline_referenced_files(
            "ca certs/ca.pem\npkcs12 identity.p12\nauth-user-pass login.txt\naskpass key-pass.txt\n",
            &parent,
        )
        .expect("build OpenVPN 3 import payload");

        assert!(result.contains("<ca>\ncertificate\n</ca>"));
        assert!(result.contains("<pkcs12>\nAAECAw==\n</pkcs12>"));
        assert!(result.contains("auth-user-pass\n"));
        assert!(result.contains("askpass\n"));
        assert!(!result.contains("login.txt"));
        assert!(!result.contains("key-pass.txt"));
        fs::remove_dir_all(profile_dir).expect("remove source directory");
    }

    #[test]
    fn preserves_absolute_references_and_rejects_parent_traversal() {
        let result = inline_referenced_files(
            "# ca /etc/example.pem\nauth-user-pass # prompt\n<ca>\ninline\n</ca>\n",
            &gio::File::for_path("/tmp"),
        )
        .expect("preserve comments and inline blocks");
        assert!(result.contains("# ca /etc/example.pem"));
        assert!(result.contains("auth-user-pass # prompt"));
        assert!(result.contains("<ca>\ninline\n</ca>"));
        assert!(
            inline_referenced_files("ca ../outside.pem\n", &gio::File::for_path("/tmp")).is_err()
        );
    }

    #[test]
    fn profile_name_uses_the_parent_directory() {
        assert_eq!(
            profile_display_name(Some(Path::new("/vpn/work/profile.ovpn")), "profile.ovpn"),
            "work"
        );
    }
}
