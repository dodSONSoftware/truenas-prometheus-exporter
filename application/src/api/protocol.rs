use crate::{config::Credentials, state::Reason};
use serde::Deserialize;
use serde_json::{Value, json};

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
}
impl Method {
    pub fn request(self, id: &str) -> Value {
        match self {
            Self::PoolQuery => {
                json!({"jsonrpc":"2.0", "id":id, "method":"pool.query", "params":[]})
            }
        }
    }
}

pub fn decode(text: &str) -> Result<Incoming, Reason> {
    let value: Value = serde_json::from_str(text).map_err(|_| Reason::Protocol)?;
    let object = value.as_object().ok_or(Reason::Protocol)?;
    if object.get("jsonrpc").and_then(Value::as_str) != Some("2.0") {
        return Err(Reason::Protocol);
    }
    if let Some(id) = object.get("id") {
        let id = id.as_str().ok_or(Reason::Protocol)?.to_owned();
        if object.contains_key("method")
            || object.contains_key("result") == object.contains_key("error")
        {
            return Err(Reason::Protocol);
        }
        let result = if let Some(error) = object.get("error") {
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
            Ok(object.get("result").ok_or(Reason::Protocol)?.clone())
        };
        Ok(Incoming::Response { id, result })
    } else if object.get("method").and_then(Value::as_str).is_some()
        && !object.contains_key("result")
        && !object.contains_key("error")
    {
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
