use std::path::Path;
use std::sync::mpsc;
use std::sync::OnceLock;

use gtk::glib;
use gtk::prelude::*;

use super::VpnCredentials;

pub(super) struct CredentialPrompt {
    pub(super) credentials: VpnCredentials,
    pub(super) save: bool,
}

type PromptResult = Result<Option<CredentialPrompt>, String>;

// Nautilus already runs a GTK4 main loop; gtk_init() is safe to call again but only once here.
fn gtk_init_result() -> Result<(), String> {
    static RESULT: OnceLock<Result<(), String>> = OnceLock::new();
    RESULT
        .get_or_init(|| gtk::init().map_err(|e| format!("failed to initialize GTK: {e}")))
        .clone()
}

pub(super) fn prompt_for_credentials(config_path: &Path) -> PromptResult {
    gtk_init_result()?;

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
    glib::MainContext::default().invoke(move || show_dialog(title, tx));

    rx.recv()
        .map_err(|e| format!("credential dialog channel closed unexpectedly: {e}"))?
}

fn show_dialog(title: String, tx: mpsc::SyncSender<PromptResult>) {
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
    content.append(&username_entry);

    let password_label = gtk::Label::new(Some("Passwort"));
    password_label.set_halign(gtk::Align::Start);
    content.append(&password_label);
    let password_entry = gtk::PasswordEntry::new();
    password_entry.set_show_peek_icon(true);
    content.append(&password_entry);

    let save_check = gtk::CheckButton::with_label("Anmeldedaten speichern");
    content.append(&save_check);

    let button_box = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    button_box.set_halign(gtk::Align::End);
    let cancel_button = gtk::Button::with_label("Abbrechen");
    let connect_button = gtk::Button::with_label("Verbinden");
    connect_button.add_css_class("suggested-action");
    button_box.append(&cancel_button);
    button_box.append(&connect_button);
    content.append(&button_box);

    window.set_child(Some(&content));
    window.set_default_widget(Some(&connect_button));
    username_entry.set_activates_default(true);
    password_entry.set_activates_default(true);

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
