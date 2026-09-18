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
) -> PromptResult {
    let title = format!(
        "VPN-Anmeldung für {}",
        config_path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("OpenVPN")
    );

    // The dialog must be built on the main thread; this function runs on a background
    // worker thread, so we hand off construction via the main context and block on a channel.
    let (tx, rx) = mpsc::sync_channel::<PromptResult>(1);
    let stored_credentials = stored_credentials
        .map(|credentials| (credentials.username.clone(), credentials.password.clone()));
    glib::MainContext::default()
        .invoke(move || show_dialog(title, tx, stored_credentials, auto_connect_delay_seconds));

    rx.recv()
        .map_err(|e| format!("credential dialog channel closed unexpectedly: {e}"))?
}

fn show_dialog(
    title: String,
    tx: mpsc::SyncSender<PromptResult>,
    stored_credentials: Option<(String, String)>,
    auto_connect_delay_seconds: i32,
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

    let info_label = gtk::Label::new(Some("Diese VPN-Verbindung benötigt Anmeldedaten."));
    info_label.set_halign(gtk::Align::Start);
    content.append(&info_label);

    let username_label = gtk::Label::new(Some("Benutzername"));
    username_label.set_halign(gtk::Align::Start);
    content.append(&username_label);
    let username_entry = gtk::Entry::new();
    if let Some((username, _)) = &stored_credentials {
        username_entry.set_text(username);
    }
    content.append(&username_entry);

    let password_label = gtk::Label::new(Some("Passwort"));
    password_label.set_halign(gtk::Align::Start);
    content.append(&password_label);
    let password_entry = gtk::PasswordEntry::new();
    password_entry.set_show_peek_icon(true);
    if let Some((_, password)) = &stored_credentials {
        password_entry.set_text(password);
    }
    content.append(&password_entry);

    let save_check = gtk::CheckButton::with_label("Anmeldedaten speichern");
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

    if stored_credentials.is_some() && auto_connect_delay_seconds > 0 {
        let button_for_timer = connect_button.clone();
        let credentials_changed_for_timer = credentials_changed.clone();
        let remaining = Rc::new(Cell::new(auto_connect_delay_seconds));
        let remaining_for_timer = remaining.clone();
        let tx_for_timer = tx.clone();
        let window_for_timer = window.clone();
        let username_for_timer = username_entry.clone();
        let password_for_timer = password_entry.clone();
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
        let save = save_check.is_active();

        let result = if username.is_empty() || password.is_empty() {
            Err("VPN username and password must not be empty".to_string())
        } else {
            Ok(Some(CredentialPrompt {
                credentials: VpnCredentials { username, password },
                save,
            }))
        };

        let _ = tx_connect.send(result);
        window_connect.close();
    });

    window.present();
}
