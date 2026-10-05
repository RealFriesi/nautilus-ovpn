use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::OnceLock;
use std::time::Duration;

use gtk::prelude::*;
use tokio::sync::{mpsc, oneshot};

#[derive(Clone)]
pub struct UiBridge {
    sender: mpsc::UnboundedSender<UiRequest>,
}

pub struct PromptSpec {
    pub title: String,
    pub label: String,
    pub description: String,
    pub hidden: bool,
    pub allow_save: bool,
}

pub struct PromptReply {
    pub value: String,
    pub save_in_keyring: bool,
}

enum UiRequest {
    Prompt {
        spec: PromptSpec,
        reply: oneshot::Sender<Option<PromptReply>>,
    },
    ShowStatus {
        id: String,
        title: String,
        message: String,
        disconnect: mpsc::UnboundedSender<()>,
    },
    UpdateStatus {
        id: String,
        message: String,
    },
    CloseStatus {
        id: String,
    },
}

static UI_BRIDGE: OnceLock<UiBridge> = OnceLock::new();

impl UiBridge {
    pub fn global() -> Result<Self, String> {
        UI_BRIDGE
            .get()
            .cloned()
            .ok_or_else(|| "GTK UI bridge has not been initialized".to_string())
    }

    pub async fn prompt(&self, spec: PromptSpec) -> Result<Option<PromptReply>, String> {
        let (reply, response) = oneshot::channel();
        self.sender
            .send(UiRequest::Prompt { spec, reply })
            .map_err(|_| "GTK UI request channel is closed".to_string())?;
        response
            .await
            .map_err(|_| "GTK prompt was closed unexpectedly".to_string())
    }

    pub fn show_status(
        &self,
        id: String,
        title: String,
        message: String,
        disconnect: mpsc::UnboundedSender<()>,
    ) -> Result<(), String> {
        self.sender
            .send(UiRequest::ShowStatus {
                id,
                title,
                message,
                disconnect,
            })
            .map_err(|_| "GTK UI request channel is closed".to_string())
    }

    pub fn update_status(&self, id: String, message: String) {
        let _ = self.sender.send(UiRequest::UpdateStatus { id, message });
    }

    pub fn close_status(&self, id: String) {
        let _ = self.sender.send(UiRequest::CloseStatus { id });
    }
}

pub(crate) fn initialize() {
    if UI_BRIDGE.get().is_some() {
        return;
    }

    let (sender, mut receiver) = mpsc::unbounded_channel();
    if UI_BRIDGE.set(UiBridge { sender }).is_err() {
        return;
    }

    let status_windows: Rc<RefCell<HashMap<String, (gtk::Window, gtk::Label, Rc<Cell<bool>>)>>> =
        Rc::default();
    glib::timeout_add_local(Duration::from_millis(40), move || {
        while let Ok(request) = receiver.try_recv() {
            dispatch(request, &status_windows);
        }
        glib::ControlFlow::Continue
    });
}

fn dispatch(
    request: UiRequest,
    status_windows: &Rc<RefCell<HashMap<String, (gtk::Window, gtk::Label, Rc<Cell<bool>>)>>>,
) {
    match request {
        UiRequest::Prompt { spec, reply } => show_prompt(spec, reply),
        UiRequest::ShowStatus {
            id,
            title,
            message,
            disconnect,
        } => show_status(id, title, message, disconnect, status_windows),
        UiRequest::UpdateStatus { id, message } => {
            if let Some((_, label, _)) = status_windows.borrow().get(&id) {
                label.set_text(&message);
            }
        }
        UiRequest::CloseStatus { id } => {
            if let Some((window, _, closing)) = status_windows.borrow_mut().remove(&id) {
                closing.set(true);
                window.close();
            }
        }
    }
}

fn show_prompt(spec: PromptSpec, reply: oneshot::Sender<Option<PromptReply>>) {
    let window = gtk::Window::builder()
        .title(&spec.title)
        .modal(true)
        .resizable(false)
        .default_width(380)
        .build();
    let content = gtk::Box::new(gtk::Orientation::Vertical, 10);
    content.set_margin_top(16);
    content.set_margin_bottom(16);
    content.set_margin_start(16);
    content.set_margin_end(16);

    let label = gtk::Label::new(Some(if spec.description.is_empty() {
        &spec.label
    } else {
        &spec.description
    }));
    label.set_halign(gtk::Align::Start);
    label.set_wrap(true);
    content.append(&label);

    let entry = gtk::Entry::new();
    entry.set_visibility(!spec.hidden);
    entry.set_activates_default(true);
    content.append(&entry);

    let save = gtk::CheckButton::with_label(&crate::i18n::translate("Save in keyring"));
    save.set_visible(spec.allow_save);
    content.append(&save);

    let buttons = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    buttons.set_halign(gtk::Align::End);
    let cancel = gtk::Button::with_label(&crate::i18n::translate("Cancel"));
    let submit = gtk::Button::with_label(&crate::i18n::translate("Submit"));
    submit.add_css_class("suggested-action");
    buttons.append(&cancel);
    buttons.append(&submit);
    content.append(&buttons);

    window.set_child(Some(&content));
    window.set_default_widget(Some(&submit));

    let pending_reply = Rc::new(RefCell::new(Some(reply)));
    let reply_for_submit = pending_reply.clone();
    let entry_for_submit = entry.clone();
    let save_for_submit = save.clone();
    let window_for_submit = window.clone();
    submit.connect_clicked(move |_| {
        send_prompt_reply(
            &reply_for_submit,
            Some(PromptReply {
                value: entry_for_submit.text().to_string(),
                save_in_keyring: save_for_submit.is_active(),
            }),
        );
        window_for_submit.close();
    });

    let reply_for_cancel = pending_reply.clone();
    let window_for_cancel = window.clone();
    cancel.connect_clicked(move |_| {
        send_prompt_reply(&reply_for_cancel, None);
        window_for_cancel.close();
    });

    window.connect_close_request(move |_| {
        send_prompt_reply(&pending_reply, None);
        glib::Propagation::Proceed
    });
    window.present();
}

fn send_prompt_reply(
    pending_reply: &Rc<RefCell<Option<oneshot::Sender<Option<PromptReply>>>>>,
    response: Option<PromptReply>,
) {
    if let Some(reply) = pending_reply.borrow_mut().take() {
        let _ = reply.send(response);
    }
}

fn show_status(
    id: String,
    title: String,
    message: String,
    disconnect: mpsc::UnboundedSender<()>,
    status_windows: &Rc<RefCell<HashMap<String, (gtk::Window, gtk::Label, Rc<Cell<bool>>)>>>,
) {
    let window = gtk::Window::builder()
        .title(title)
        .modal(false)
        .resizable(false)
        .default_width(380)
        .build();
    let content = gtk::Box::new(gtk::Orientation::Vertical, 12);
    content.set_margin_top(16);
    content.set_margin_bottom(16);
    content.set_margin_start(16);
    content.set_margin_end(16);

    let label = gtk::Label::new(Some(&message));
    label.set_halign(gtk::Align::Start);
    label.set_wrap(true);
    content.append(&label);

    let disconnect_button = gtk::Button::with_label(&crate::i18n::translate("Disconnect"));
    disconnect_button.add_css_class("destructive-action");
    disconnect_button.set_halign(gtk::Align::End);
    content.append(&disconnect_button);
    window.set_child(Some(&content));

    let close_requested = Rc::new(Cell::new(false));
    let close_for_button = close_requested.clone();
    let disconnect_for_button = disconnect.clone();
    let window_for_button = window.clone();
    disconnect_button.connect_clicked(move |_| {
        if !close_for_button.replace(true) {
            let _ = disconnect_for_button.send(());
        }
        window_for_button.close();
    });
    let close_for_window = close_requested.clone();
    window.connect_close_request(move |_| {
        if !close_for_window.replace(true) {
            let _ = disconnect.send(());
        }
        glib::Propagation::Proceed
    });

    status_windows
        .borrow_mut()
        .insert(id, (window.clone(), label, close_requested));
    window.present();
}
