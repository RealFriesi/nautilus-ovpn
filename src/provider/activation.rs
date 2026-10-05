use std::thread;

use futures_util::StreamExt;
use rhai::Dynamic;
use tokio::sync::mpsc;

use crate::scripting::HookContext;
use crate::ui::{PromptSpec, UiBridge};
use crate::{keyring, scripting, vpn};

pub(super) fn activate_connection(uri: String) {
    crate::log(format!("starting OpenVPN D-Bus session for {uri}"));
    let ui = match UiBridge::global() {
        Ok(ui) => ui,
        Err(error) => {
            crate::log_err(error);
            return;
        }
    };

    thread::spawn(move || {
        let runtime = match tokio::runtime::Runtime::new() {
            Ok(runtime) => runtime,
            Err(error) => {
                crate::log_err(format!("failed to start Tokio runtime: {error}"));
                return;
            }
        };
        if let Err(error) = runtime.block_on(start_session(&uri, ui)) {
            crate::log_err(error);
        }
    });
}

async fn start_session(uri: &str, ui: UiBridge) -> Result<(), String> {
    let profile = vpn::load_profile(uri)?;
    let client = vpn::Client::connect_system_bus().await?;
    let session = client
        .import_and_create_session(&profile.display_name, &profile.payload)
        .await?;

    let mut context = HookContext::default();
    context.config.insert(
        "profile_name".to_string(),
        Dynamic::from(profile.display_name.clone()),
    );
    context.dbus_payload.insert(
        "session_path".to_string(),
        Dynamic::from(session.path().to_string()),
    );
    context.dbus_payload.insert(
        "connect_arguments".to_string(),
        Dynamic::from(Vec::<Dynamic>::new()),
    );

    let session_id = format!("{}-{}", profile.profile_hash, uuid::Uuid::new_v4());
    match run_session(
        &session,
        &session_id,
        &profile.display_name,
        &profile.profile_hash,
        &ui,
        &mut context,
    )
    .await
    {
        Ok(()) => {
            if let Some(script) = scripting::load_hook("post-disconnect.rhai")? {
                scripting::run_post_disconnect(&script, &mut context)?;
            }
            Ok(())
        }
        Err(error) => {
            let _ = session.disconnect().await;
            Err(error)
        }
    }
}

async fn run_session(
    session: &vpn::VpnSession<'_>,
    session_id: &str,
    profile_name: &str,
    profile_hash: &str,
    ui: &UiBridge,
    context: &mut HookContext,
) -> Result<(), String> {
    let mut attention_events = session
        .receive_attention_required()
        .await
        .map_err(|error| {
            format!("failed to subscribe to OpenVPN authentication events: {error}")
        })?;
    let mut status_events = session
        .receive_status_change()
        .await
        .map_err(|error| format!("failed to subscribe to OpenVPN status events: {error}"))?;

    loop {
        match session.ready().await {
            Ok(()) => break,
            Err(ready_error) => {
                let inputs = session.requested_inputs().await?;
                if inputs.is_empty() {
                    return Err(ready_error);
                }
                provide_pending_inputs(session, &inputs, profile_name, profile_hash, ui).await?;
            }
        }
    }

    if let Some(script) = scripting::load_hook("pre-connect.rhai")? {
        scripting::run_pre_connect(&script, context)?;
    }

    session.connect().await?;
    let (disconnect_sender, mut disconnect_receiver) = mpsc::unbounded_channel();
    ui.show_status(
        session_id.to_string(),
        profile_name.to_string(),
        crate::i18n::translate("Connecting"),
        disconnect_sender,
    )?;

    let mut attention_open = true;
    let mut disconnect_requested = false;
    loop {
        tokio::select! {
            event = attention_events.next(), if attention_open => {
                match event {
                    Some(message) => {
                        let (_, _, message) = message.body().deserialize::<(u32, u32, String)>()
                            .map_err(|error| format!("invalid OpenVPN attention signal: {error}"))?;
                        ui.update_status(session_id.to_string(), message);
                        let inputs = session.requested_inputs().await?;
                        provide_pending_inputs(session, &inputs, profile_name, profile_hash, ui).await?;
                    }
                    None => attention_open = false,
                }
            }
            event = status_events.next() => {
                match event {
                    Some(message) => {
                        let (major, minor, message) = message.body().deserialize::<(u32, u32, String)>()
                            .map_err(|error| format!("invalid OpenVPN status signal: {error}"))?;
                        ui.update_status(session_id.to_string(), message);
                        if vpn::is_session_end_status(major, minor) {
                            ui.close_status(session_id.to_string());
                            return Ok(());
                        }
                    }
                    None => {
                        ui.close_status(session_id.to_string());
                        return Ok(());
                    }
                }
            }
            disconnect = disconnect_receiver.recv(), if !disconnect_requested => {
                disconnect_requested = true;
                if disconnect.is_some() {
                    session.disconnect().await?;
                    ui.update_status(session_id.to_string(), crate::i18n::translate("Disconnecting"));
                }
            }
        }
    }
}

async fn provide_pending_inputs(
    session: &vpn::VpnSession<'_>,
    inputs: &[vpn::UserInputSlot],
    profile_name: &str,
    profile_hash: &str,
    ui: &UiBridge,
) -> Result<(), String> {
    for input in inputs {
        let label = if input.name.is_empty() {
            crate::i18n::translate("OpenVPN authentication")
        } else {
            input.name.clone()
        };
        let cacheable = is_keyring_candidate(input);
        let slot = format!("{}:{}:{}", input.typ, input.group, label);
        let cached = if cacheable {
            match keyring::lookup(profile_hash, &slot).await {
                Ok(Some(value)) => String::from_utf8(value).ok(),
                Ok(None) => None,
                Err(error) => {
                    crate::log_err(error);
                    None
                }
            }
        } else {
            None
        };

        let value = if let Some(value) = cached {
            value
        } else {
            let Some(reply) = ui
                .prompt(PromptSpec {
                    title: profile_name.to_string(),
                    label,
                    description: input.description.clone(),
                    hidden: input.hidden,
                    allow_save: cacheable,
                })
                .await?
            else {
                return Err("VPN activation cancelled during authentication".to_string());
            };
            if cacheable && reply.save_in_keyring {
                let item_label = format!("{profile_name} - {}", input.name);
                if let Err(error) =
                    keyring::store(profile_hash, &slot, &item_label, reply.value.as_bytes()).await
                {
                    crate::log_err(error);
                }
            }
            reply.value
        };

        session.provide_input(input, &value).await?;
    }
    Ok(())
}

fn is_keyring_candidate(input: &vpn::UserInputSlot) -> bool {
    input.typ == 1 && matches!(input.group, 1..=3)
}

#[cfg(test)]
mod tests {
    use super::is_keyring_candidate;
    use crate::vpn::UserInputSlot;

    #[test]
    fn only_persistent_credential_groups_are_keyring_candidates() {
        let make_input = |group| UserInputSlot {
            typ: 1,
            group,
            id: 1,
            name: "input".to_string(),
            description: String::new(),
            hidden: true,
        };

        assert!(is_keyring_candidate(&make_input(1)));
        assert!(is_keyring_candidate(&make_input(2)));
        assert!(is_keyring_candidate(&make_input(3)));
        assert!(!is_keyring_candidate(&make_input(5)));
        assert!(!is_keyring_candidate(&UserInputSlot {
            typ: 2,
            ..make_input(1)
        }));
    }
}
