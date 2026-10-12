//! Requests through an API: the operation of the method and path, its
//! request mapping into the asset's data, the call of the program or
//! transaction, and the response mapping its condition chooses.

use serde_json::{Map, Value};

use crate::api::{self, Api, Target};
use crate::dai;
use crate::expr::Env;

/// Runs programs and transactions for the gateway.
pub trait Backend {
    /// Links to a CICS program with a COMMAREA; the COMMAREA it returns,
    /// or the abend code.
    fn link(&self, transid: &str, program: &str, commarea: Vec<u8>) -> Result<Vec<u8>, String>;
    /// Sends an input message (one segment, with LL ZZ) to an IMS
    /// transaction; the segments of the output message, or an error.
    fn transaction(&self, code: &str, segment: Vec<u8>) -> Result<Vec<Vec<u8>>, String>;
}

/// An HTTP request.
#[derive(Debug, Default, Clone)]
pub struct Request {
    pub method: String,
    pub path: String,
    pub query: Vec<(String, String)>,
    pub headers: Vec<(String, String)>,
    pub body: Option<Value>,
}

/// An HTTP response: status and JSON body.
#[derive(Debug, Clone, PartialEq)]
pub struct Response {
    pub status: u16,
    pub body: Value,
}

fn error(status: u16, code: &str, message: &str) -> Response {
    let mut m = Map::new();
    m.insert("code".into(), Value::String(code.into()));
    m.insert("message".into(), Value::String(message.into()));
    Response {
        status,
        body: Value::Object(m),
    }
}

/// Handles a request.
pub fn handle(api: &Api, backend: &dyn Backend, req: &Request) -> Response {
    let path = if api.base.is_empty() {
        Some(req.path.as_str())
    } else {
        req.path.strip_prefix(&api.base)
    };
    let Some(path) = path else {
        return error(404, "NOT_FOUND", &format!("{} is not in the API", req.path));
    };
    let Some((op, params)) = api.route(&req.method, path) else {
        return error(
            404,
            "NOT_FOUND",
            &format!("no operation {} {}", req.method, req.path),
        );
    };
    let Some(asset) = api.assets.get(&op.asset) else {
        return error(
            500,
            "CONFIGURATION",
            &format!("z/OS asset {} is not defined", op.asset),
        );
    };
    // the request's parts as the mappings see them
    let query: Map<String, Value> = req
        .query
        .iter()
        .map(|(k, v)| (k.clone(), Value::String(v.clone())))
        .collect();
    let headers: Map<String, Value> = req
        .headers
        .iter()
        .map(|(k, v)| (k.to_ascii_lowercase(), Value::String(v.clone())))
        .collect();
    let body = req.body.clone().unwrap_or(Value::Null);
    let mut env = Env::default();
    env.vars
        .insert("pathParameters".into(), Value::Object(params.clone()));
    env.vars
        .insert("queryParameters".into(), Value::Object(query.clone()));
    env.vars
        .insert("headers".into(), Value::Object(headers.clone()));
    env.vars.insert("body".into(), body.clone());
    env.vars.insert("requestBody".into(), body.clone());
    let mut request = Map::new();
    request.insert("pathParameters".into(), Value::Object(params));
    request.insert("queryParameters".into(), Value::Object(query));
    request.insert("headers".into(), Value::Object(headers));
    request.insert("body".into(), body);
    env.context = Value::Object(request.clone());
    let mapped = match api::apply(&op.request, &env) {
        Ok(v) => v,
        Err(e) => return error(400, "BAD_REQUEST", &e),
    };
    // the call
    let response = match &asset.target {
        Target::Cics { program, transid } => {
            let mut data = dai::encode(
                &asset.request,
                mapped.get("commarea").unwrap_or(&Value::Null),
            );
            // the COMMAREA has room for the larger of the request and the
            // response
            let room = dai::size(&asset.request).max(dai::size(&asset.response));
            if data.len() < room {
                data.resize(room, zeus_cobol::ebcdic::SPACE);
            }
            match backend.link(transid, program, data) {
                Ok(back) => {
                    let mut v = dai::decode(&asset.response, &back);
                    dai::clean(&mut v);
                    let mut m = Map::new();
                    m.insert("commarea".into(), v);
                    Value::Object(m)
                }
                Err(code) => abend(&code),
            }
        }
        Target::Ims { transaction } => {
            let mut data = dai::encode(
                &asset.request,
                mapped.get("message").unwrap_or(&Value::Null),
            );
            // LL: the length of the segment
            if data.len() >= 2 && data[0] == 0 && data[1] == 0 {
                let n = (data.len() as u16).to_be_bytes();
                data[..2].copy_from_slice(&n);
            }
            match backend.transaction(transaction, data) {
                Ok(segs) => {
                    let first = segs.into_iter().next().unwrap_or_default();
                    let mut v = dai::decode(&asset.response, &first);
                    dai::clean(&mut v);
                    let mut m = Map::new();
                    m.insert("message".into(), v);
                    Value::Object(m)
                }
                Err(e) => abend(&e),
            }
        }
    };
    env.vars.insert("zosAssetResponse".into(), response);
    env.vars.insert("apiRequest".into(), Value::Object(request));
    for (cond, status, mapping) in &op.responses {
        let Some(c) = cond else { continue };
        match api::condition(c, &env) {
            Ok(true) => {}
            Ok(false) => continue,
            Err(e) => return error(500, "MAPPING", &e),
        }
        return match api::apply(mapping, &env) {
            Ok(v) => Response {
                status: *status,
                body: v.get("body").cloned().unwrap_or(Value::Object(Map::new())),
            },
            Err(e) => error(500, "MAPPING", &e),
        };
    }
    error(500, "NO_RESPONSE", "no response mapping applies")
}

fn abend(code: &str) -> Value {
    let mut m = Map::new();
    m.insert("abendCode".into(), Value::String(code.into()));
    Value::Object(m)
}
