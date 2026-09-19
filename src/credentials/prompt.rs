use std::path::Path;
use std::sync::mpsc;
use std::{cell::Cell, rc::Rc};

use gtk::glib;
use gtk::prelude::*;

use super::VpnCredentials;

pub(super) struct CredentialPrompt {
    pub(super) credentials: VpnCredentials,
    pub(super) save: bool,
}

type PromptResult = Result<Option<CredentialPrompt>, String>;

pub(super) fn prompt_for_credentials(
    config_path: &Path,
    stored_credentials: Option<&VpnCredentials>,
    auto_connect_delay_seconds: i32,
    requires_user_pass: bool,
) -> PromptResult {
    let title = format!(
        "VPN-Optionen für {}",
        config_path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("OpenVPN")
    );

    // The dialog must be built on the main thread; this function runs on a background
    // worker thread, so we hand off construction via the main context and block on a channel.
    let (tx, rx) = mpsc::sync_channel::<PromptResult>(1);
    let stored_credentials = stored_credentials.map(|credentials| {
        (
            credentials.username.clone(),
            credentials.password.clone(),
            credentials.private_key_password.clone(),
            credentials.legacy_auth,
        )
    });
    glib::MainContext::default().invoke(move || {
        show_dialog(
            title,
            tx,
            stored_credentials,
            auto_connect_delay_seconds,
            requires_user_pass,
        )
    });

    rx.recv()
        .map_err(|e| format!("credential dialog channel closed unexpectedly: {e}"))?
}

fn show_dialog(
    title: String,
    tx: mpsc::SyncSender<PromptResult>,
    stored_credentials: Option<(String, String, String, bool)>,
    auto_connect_delay_seconds: i32,
    requires_user_pass: bool,
) {
    let window = gtk::Window::builder()
        .title(title)
        .modal(true)
        .resizable(false)
        .default_width(360)
        .build();

    let content = gtk::Box::new(gtk::Orientation::Vertical, 12);
    content.set_margin_top(16);
    content.set_margin_bottom(16);
    content.set_margin_start(16);
    content.set_margin_end(16);

    let info_label = gtk::Label::new(Some(if requires_user_pass {
        "Diese VPN-Verbindung benötigt Anmeldedaten."
    } else {
        "Diese VPN-Verbindung benötigt keine Anmeldedaten."
    }));
    info_label.set_halign(gtk::Align::Start);
    content.append(&info_label);

    let username_label = gtk::Label::new(Some("Benutzername"));
    username_label.set_halign(gtk::Align::Start);
    content.append(&username_label);
    let username_entry = gtk::Entry::new();
    username_entry.set_sensitive(requires_user_pass);
    if let Some((username, _, _, _)) = &stored_credentials {
        username_entry.set_text(username);
    }
    content.append(&username_entry);

    let password_label = gtk::Label::new(Some("Passwort"));
    password_label.set_halign(gtk::Align::Start);
    content.append(&password_label);
    let password_entry = gtk::PasswordEntry::new();
    password_entry.set_show_peek_icon(true);
    password_entry.set_sensitive(requires_user_pass);
    if let Some((_, password, _, _)) = &stored_credentials {
        password_entry.set_text(password);
    }
    content.append(&password_entry);

    let private_key_password_label =
        gtk::Label::new(Some("Passwort des privaten Schlüssels (optional)"));
    private_key_password_label.set_halign(gtk::Align::Start);
    content.append(&private_key_password_label);
    let private_key_password_entry = gtk::PasswordEntry::new();
    private_key_password_entry.set_show_peek_icon(true);
    if let Some((_, _, private_key_password, _)) = &stored_credentials {
        private_key_password_entry.set_text(private_key_password);
    }
    content.append(&private_key_password_entry);

    let legacy_auth_check = gtk::CheckButton::with_label("Legacy-Authentifizierung");
    legacy_auth_check.set_active(
        stored_credentials
            .as_ref()
            .map(|(_, _, _, legacy_auth)| *legacy_auth)
            .unwrap_or(false),
    );
    content.append(&legacy_auth_check);

    let save_check = gtk::CheckButton::with_label(if requires_user_pass {
        "Anmeldedaten speichern"
    } else {
        "Einstellungen speichern"
    });
    save_check.set_active(stored_credentials.is_some());
    content.append(&save_check);

    let button_box = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    button_box.set_halign(gtk::Align::End);
    let cancel_button = gtk::Button::with_label("Abbrechen");
    let connect_label = if stored_credentials.is_some() {
        format!("Verbinden ({auto_connect_delay_seconds})")
    } else {
        "Verbinden".to_string()
    };
    let connect_button = gtk::Button::with_label(&connect_label);
    connect_button.add_css_class("suggested-action");
    button_box.append(&cancel_button);
    button_box.append(&connect_button);
    content.append(&button_box);

    window.set_child(Some(&content));
    window.set_default_widget(Some(&connect_button));
    username_entry.set_activates_default(true);
    password_entry.set_activates_default(true);

    let credentials_changed = Rc::new(Cell::new(false));
    let changed_for_username = credentials_changed.clone();
    username_entry.connect_changed(move |_| changed_for_username.set(true));
    let changed_for_password = credentials_changed.clone();
    password_entry.connect_changed(move |_| changed_for_password.set(true));
    let changed_for_private_key_password = credentials_changed.clone();
    private_key_password_entry.connect_changed(move |_| changed_for_private_key_password.set(true));
    let changed_for_legacy_auth = credentials_changed.clone();
    legacy_auth_check.connect_toggled(move |_| changed_for_legacy_auth.set(true));
    let changed_for_save = credentials_changed.clone();
    save_check.connect_toggled(move |_| changed_for_save.set(true));

    if stored_credentials.is_some() && auto_connect_delay_seconds > 0 {
        let button_for_timer = connect_button.clone();
        let credentials_changed_for_timer = credentials_changed.clone();
        let remaining = Rc::new(Cell::new(auto_connect_delay_seconds));
        let remaining_for_timer = remaining.clone();
        let tx_for_timer = tx.clone();
        let window_for_timer = window.clone();
        let username_for_timer = username_entry.clone();
        let password_for_timer = password_entry.clone();
        let private_key_password_for_timer = private_key_password_entry.clone();
        let legacy_auth_for_timer = legacy_auth_check.clone();
        let save_for_timer = save_check.clone();
        glib::timeout_add_seconds_local(1, move || {
            if credentials_changed_for_timer.get() {
                button_for_timer.set_label("Verbinden");
                return glib::ControlFlow::Break;
            }

            let next = remaining_for_timer.get() - 1;
            remaining_for_timer.set(next);
            if next <= 0 {
                let result = Ok(Some(CredentialPrompt {
                    credentials: VpnCredentials {
                        username: username_for_timer.text().trim().to_string(),
                        password: password_for_timer.text().to_string(),
                        private_key_password: private_key_password_for_timer.text().to_string(),
                        legacy_auth: legacy_auth_for_timer.is_active(),
                    },
                    save: save_for_timer.is_active(),
                }));
                let _ = tx_for_timer.send(result);
                window_for_timer.close();
                glib::ControlFlow::Break
            } else {
                button_for_timer.set_label(&format!("Verbinden ({next})"));
                glib::ControlFlow::Continue
            }
        });
    }

    let tx_cancel = tx.clone();
    let window_cancel = window.clone();
    cancel_button.connect_clicked(move |_| {
        let _ = tx_cancel.send(Ok(None));
        window_cancel.close();
    });

    let tx_close = tx.clone();
    window.connect_close_request(move |_| {
        let _ = tx_close.send(Ok(None));
        glib::Propagation::Proceed
    });

    let tx_connect = tx;
    let window_connect = window.clone();
    connect_button.connect_clicked(move |_| {
        let username = username_entry.text().trim().to_string();
        let password = password_entry.text().to_string();
        let private_key_password = private_key_password_entry.text().to_string();
        let legacy_auth = legacy_auth_check.is_active();
        let save = save_check.is_active();

        let result = if requires_user_pass && (username.is_empty() || password.is_empty()) {
            Err("VPN username and password must not be empty".to_string())
        } else {
            Ok(Some(CredentialPrompt {
                credentials: VpnCredentials {
                    username,
                    password,
                    private_key_password,
                    legacy_auth,
                },
                save,
            }))
        };

        let _ = tx_connect.send(result);
        window_connect.close();
    });

    window.present();
}
