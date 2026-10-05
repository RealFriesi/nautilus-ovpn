use std::collections::HashMap;
use std::{env, fs, path::PathBuf};

use rhai::{Dynamic, Engine, ImmutableString, Scope};

#[derive(Clone, Default)]
pub struct HookContext {
    pub config: HashMap<String, Dynamic>,
    pub dbus_payload: HashMap<String, Dynamic>,
}

pub fn load_hook(filename: &str) -> Result<Option<String>, String> {
    if !matches!(filename, "pre-connect.rhai" | "post-disconnect.rhai") {
        return Err("unsupported hook name".to_string());
    }
    let config_home = env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| env::var_os("HOME").map(|home| PathBuf::from(home).join(".config")));
    let Some(config_home) = config_home else {
        return Ok(None);
    };
    let path = config_home
        .join("nautilus-ovpn")
        .join("hooks")
        .join(filename);
    match fs::read_to_string(&path) {
        Ok(script) => Ok(Some(script)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(format!("failed to read Rhai hook {path:?}: {error}")),
    }
}

pub fn run_pre_connect(script: &str, context: &mut HookContext) -> Result<(), String> {
    let result = run(script, context)?;
    let should_connect = if result.is::<bool>() {
        result.as_bool().unwrap_or(false)
    } else if result.is::<i64>() {
        result.as_int().unwrap_or(1) == 0
    } else {
        true
    };

    if should_connect {
        Ok(())
    } else {
        Err("pre-connect hook rejected the connection".to_string())
    }
}

pub fn run_post_disconnect(script: &str, context: &mut HookContext) -> Result<(), String> {
    run(script, context).map(|_| ())
}

fn run(script: &str, context: &mut HookContext) -> Result<Dynamic, String> {
    let mut engine = Engine::new();
    engine.register_type_with_name::<HookContext>("HookContext");
    engine.register_fn(
        "set_config",
        |context: &mut HookContext, key: ImmutableString, value: Dynamic| {
            context.config.insert(key.to_string(), value);
        },
    );
    engine.register_fn(
        "get_config",
        |context: &mut HookContext, key: ImmutableString| {
            context
                .config
                .get(key.as_str())
                .cloned()
                .unwrap_or(Dynamic::UNIT)
        },
    );
    engine.register_fn(
        "set_dbus_payload",
        |context: &mut HookContext, key: ImmutableString, value: Dynamic| {
            context.dbus_payload.insert(key.to_string(), value);
        },
    );
    engine.register_fn(
        "get_dbus_payload",
        |context: &mut HookContext, key: ImmutableString| {
            context
                .dbus_payload
                .get(key.as_str())
                .cloned()
                .unwrap_or(Dynamic::UNIT)
        },
    );

    let mut scope = Scope::new();
    scope.push("context", context.clone());
    let result = engine
        .eval_with_scope::<Dynamic>(&mut scope, script)
        .map_err(|error| format!("Rhai hook failed: {error}"))?;
    *context = scope
        .get_value("context")
        .ok_or_else(|| "Rhai hook did not preserve its context".to_string())?;
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::{run_post_disconnect, run_pre_connect, HookContext};
    use rhai::Dynamic;

    #[test]
    fn pre_connect_allows_zero_and_context_mutation() {
        let mut context = HookContext::default();
        run_pre_connect(
            "context.set_config(\"profile\", \"demo\"); context.set_dbus_payload(\"name\", \"vpn\"); 0",
            &mut context,
        )
        .expect("zero exit code allows connection");

        assert_eq!(
            context
                .config
                .get("profile")
                .and_then(|value| value.clone().try_cast::<String>()),
            Some("demo".to_string())
        );
        assert_eq!(
            context
                .dbus_payload
                .get("name")
                .and_then(|value| value.clone().try_cast::<String>()),
            Some("vpn".to_string())
        );
    }

    #[test]
    fn pre_connect_can_read_context_values() {
        let mut context = HookContext::default();
        context
            .config
            .insert("profile".to_string(), Dynamic::from("demo"));
        run_pre_connect(
            "context.set_config(\"copy\", context.get_config(\"profile\")); 0",
            &mut context,
        )
        .expect("hook reads and copies existing context values");

        assert_eq!(
            context
                .config
                .get("copy")
                .and_then(|value| value.clone().try_cast::<String>()),
            Some("demo".to_string())
        );
    }

    #[test]
    fn pre_connect_rejects_nonzero_and_throw() {
        let mut context = HookContext::default();
        assert!(run_pre_connect("1", &mut context).is_err());
        assert!(run_pre_connect("throw \"blocked\";", &mut context).is_err());
    }

    #[test]
    fn post_disconnect_runs_and_mutates_context() {
        let mut context = HookContext::default();
        run_post_disconnect("context.set_config(\"disconnected\", true);", &mut context)
            .expect("post-disconnect hook succeeds");

        assert_eq!(
            context
                .config
                .get("disconnected")
                .and_then(|value| value.clone().try_cast::<bool>()),
            Some(true)
        );
    }
}
