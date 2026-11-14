use crate::{config::Credentials, state::Reason};
use serde::Deserialize;
use serde_json::{Value, json, value::RawValue};
use std::collections::BTreeMap;

pub enum Incoming {
    Response {
        id: String,
        result: Result<Value, Reason>,
    },
    Notification,
}

pub fn login(id: &str, credentials: &Credentials) -> Value {
    json!({"jsonrpc":"2.0", "id":id, "method":"auth.login_ex", "params":[{
        "mechanism":"API_KEY_PLAIN", "username":credentials.username, "api_key":credentials.key,
        "login_options":{"user_info":false}
    }]})
}

// Closed enum: no API caller can supply an arbitrary method name or arguments.
#[derive(Clone, Copy)]
pub enum Method {
    PoolQuery,
    SystemInfo,
}
impl Method {
    pub fn request(self, id: &str) -> Value {
        match self {
            Self::PoolQuery => {
                json!({"jsonrpc":"2.0", "id":id, "method":"pool.query", "params":[]})
            }
            Self::SystemInfo => {
                json!({"jsonrpc":"2.0", "id":id, "method":"system.info", "params":[]})
            }
        }
    }
}

pub fn decode(text: &str) -> Result<Incoming, Reason> {
    decode_with(text, |raw| {
        serde_json::from_str(raw).map_err(|_| Reason::Protocol)
    })
}

// Pool replies are the only data responses admitted on an authenticated M2 session.
pub(super) fn decode_pool_response(text: &str) -> Result<Incoming, Reason> {
    decode_with(text, crate::collectors::pools::bounded_result)
}

fn decode_with(
    text: &str,
    result_decoder: impl FnOnce(&str) -> Result<Value, Reason>,
) -> Result<Incoming, Reason> {
    let object: BTreeMap<String, &RawValue> =
        serde_json::from_str(text).map_err(|_| Reason::Protocol)?;
    let string_field = |key: &str| {
        object
            .get(key)
            .and_then(|raw| serde_json::from_str::<String>(raw.get()).ok())
    };
    if string_field("jsonrpc").as_deref() != Some("2.0") {
        return Err(Reason::Protocol);
    }
    if object.contains_key("id") {
        let id = string_field("id").ok_or(Reason::Protocol)?;
        if object.contains_key("method")
            || object.contains_key("result") == object.contains_key("error")
        {
            return Err(Reason::Protocol);
        }
        let result = if let Some(raw) = object.get("error") {
            let error: Value = serde_json::from_str(raw.get()).map_err(|_| Reason::Protocol)?;
            if error.get("message").and_then(Value::as_str).is_none() {
                return Err(Reason::Protocol);
            }
            let code = error
                .get("code")
                .and_then(Value::as_i64)
                .ok_or(Reason::Protocol)?;
            let errno = error
                .get("data")
                .and_then(|d| d.get("error"))
                .and_then(Value::as_i64);
            Err(match (code, errno) {
                (-32601, _) | (_, Some(38)) => Reason::Unsupported,
                (_, Some(1 | 13)) => Reason::Permission,
                _ => Reason::Api,
            })
        } else {
            result_decoder(object.get("result").ok_or(Reason::Protocol)?.get())
        };
        Ok(Incoming::Response { id, result })
    } else if string_field("method").is_some()
        && !object.contains_key("result")
        && !object.contains_key("error")
    {
        if object.get("params").is_some_and(|params| {
            !matches!(
                params.get().trim_start().as_bytes().first(),
                Some(b'{' | b'[')
            )
        }) {
            return Err(Reason::Protocol);
        }
        Ok(Incoming::Notification)
    } else {
        Err(Reason::Protocol)
    }
}

#[derive(Deserialize)]
struct AuthResult {
    response_type: String,
}

pub fn authenticate(value: Value) -> Result<(), Reason> {
    let result: AuthResult = serde_json::from_value(value).map_err(|_| Reason::Protocol)?;
    match result.response_type.as_str() {
        "SUCCESS" => Ok(()),
        "AUTH_ERR" => Err(Reason::Auth),
        "EXPIRED" => Err(Reason::Expired),
        "OTP_REQUIRED" => Err(Reason::Otp),
        "REDIRECT" => Err(Reason::Redirect),
        _ => Err(Reason::Protocol),
    }
}
