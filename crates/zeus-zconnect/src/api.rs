//! An API: its OpenAPI paths, the operation directory of each path and
//! method (`operations/<path, URL encoded>/<method>`: the z/OS asset it
//! calls, the request mapping, the response mappings and the conditions
//! choosing them), and the z/OS assets (`zosAssets/<name>`: the program or
//! transaction and its data layouts).

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde_json::{Map, Value};
use yaml_rust2::{Yaml, YamlLoader};

use crate::dai::{self, Field};
use crate::expr::{self, Env};

fn load_yaml(path: &Path) -> Result<Yaml, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut docs =
        YamlLoader::load_from_str(&text).map_err(|e| format!("{}: {e}", path.display()))?;
    Ok(docs.drain(..).next().unwrap_or(Yaml::Null))
}

/// What a z/OS asset calls.
#[derive(Debug, Clone)]
pub enum Target {
    /// A CICS program with a COMMAREA, under a transaction identifier.
    Cics { program: String, transid: String },
    /// An IMS transaction.
    Ims { transaction: String },
}

#[derive(Debug, Clone)]
pub struct Asset {
    pub name: String,
    pub target: Target,
    pub request: Vec<Field>,
    pub response: Vec<Field>,
}

/// One operation: a method of a path.
#[derive(Debug, Clone)]
pub struct Operation {
    pub method: String,
    /// The path template (`/accounts/{accountId}`).
    pub path: String,
    pub asset: String,
    pub request: Yaml,
    /// Response mappings in order: condition, HTTP status, mapping.
    pub responses: Vec<(Option<Yaml>, u16, Yaml)>,
}

pub struct Api {
    /// The base path of the API (`/api`).
    pub base: String,
    pub operations: Vec<Operation>,
    pub assets: HashMap<String, Asset>,
}

fn s(y: &Yaml) -> Option<String> {
    match y {
        Yaml::String(t) => Some(t.clone()),
        Yaml::Integer(i) => Some(i.to_string()),
        Yaml::Real(r) => Some(r.clone()),
        Yaml::Boolean(b) => Some(b.to_string()),
        _ => None,
    }
}

/// `/a/{b}` as an operations directory name.
fn encode_path(p: &str) -> String {
    let mut out = String::new();
    for c in p.chars() {
        match c {
            '/' => out.push_str("%2F"),
            '{' => out.push_str("%7B"),
            '}' => out.push_str("%7D"),
            o => out.push(o),
        }
    }
    out
}

impl Api {
    /// Loads the API in `dir` (with `api/openapi.yaml`, `operations` and
    /// `zosAssets`).
    pub fn load(dir: &Path) -> Result<Api, String> {
        let openapi = load_yaml(&dir.join("api").join("openapi.yaml"))?;
        let base = openapi["servers"][0]["url"]
            .as_str()
            .unwrap_or("")
            .trim_end_matches('/')
            .to_string();
        let mut operations = Vec::new();
        if let Some(paths) = openapi["paths"].as_hash() {
            for (p, methods) in paths {
                let Some(p) = s(p) else { continue };
                let Some(methods) = methods.as_hash() else {
                    continue;
                };
                for (m, _) in methods {
                    let Some(m) = s(m) else { continue };
                    let d = dir.join("operations").join(encode_path(&p)).join(&m);
                    if !d.is_dir() {
                        continue;
                    }
                    operations.push(Self::operation(&d, &p, &m)?);
                }
            }
        }
        let mut assets = HashMap::new();
        if let Ok(rd) = std::fs::read_dir(dir.join("zosAssets")) {
            for e in rd.flatten() {
                if !e.path().join("zosAsset.yaml").is_file() {
                    continue;
                }
                let a = Self::asset(&e.path())?;
                assets.insert(a.name.clone(), a);
            }
        }
        Ok(Api {
            base,
            operations,
            assets,
        })
    }

    fn operation(d: &Path, path: &str, method: &str) -> Result<Operation, String> {
        let op = load_yaml(&d.join("operation.yaml"))?;
        let asset = s(&op["zasset"]).ok_or_else(|| format!("{}: no zasset", d.display()))?;
        let request = load_yaml(&d.join("request.yaml")).unwrap_or(Yaml::Null);
        let mut responses = Vec::new();
        let rm = load_yaml(&d.join("response_mapping.yaml"))?;
        if let Some(h) = rm.as_hash() {
            for (file, spec) in h {
                let Some(file) = s(file) else { continue };
                if file == "version" {
                    continue;
                }
                let status = spec["httpStatusCode"].as_i64().unwrap_or(200) as u16;
                let cond = match &spec["condition"] {
                    Yaml::BadValue | Yaml::Null => None,
                    c => Some(c.clone()),
                };
                let path = d.join(&file);
                let mapping = if path.is_file() {
                    load_yaml(&path)?
                } else {
                    Yaml::Null
                };
                responses.push((cond, status, mapping));
            }
        }
        Ok(Operation {
            method: method.to_ascii_uppercase(),
            path: path.to_string(),
            asset,
            request,
            responses,
        })
    }

    fn asset(d: &Path) -> Result<Asset, String> {
        let y = load_yaml(&d.join("zosAsset.yaml"))?;
        let name = d
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        let pp = &y["providerProperties"];
        let kind = y["type"].as_str().unwrap_or("");
        let target = if kind.starts_with("ims") {
            Target::Ims {
                transaction: s(&pp["transactionCode"]).unwrap_or_default(),
            }
        } else {
            Target::Cics {
                program: s(&pp["program"]).unwrap_or_default(),
                transid: s(&pp["transid"]).unwrap_or_else(|| "CSMI".into()),
            }
        };
        let layout = |f: &str| -> Result<Vec<Field>, String> {
            let p = d.join("providerFiles").join(f);
            match std::fs::read_to_string(&p) {
                Ok(t) => dai::parse(&t).map_err(|e| format!("{}: {e}", p.display())),
                Err(_) => Ok(Vec::new()),
            }
        };
        Ok(Asset {
            name,
            target,
            request: layout("request.dai")?,
            response: layout("response.dai")?,
        })
    }

    /// The operation of a method and path (without the base path), and
    /// the path parameters.
    pub fn route(&self, method: &str, path: &str) -> Option<(&Operation, Map<String, Value>)> {
        let parts: Vec<&str> = path.trim_matches('/').split('/').collect();
        for op in &self.operations {
            if op.method != method.to_ascii_uppercase() {
                continue;
            }
            let tparts: Vec<&str> = op.path.trim_matches('/').split('/').collect();
            if tparts.len() != parts.len() {
                continue;
            }
            let mut params = Map::new();
            let ok = tparts.iter().zip(&parts).all(|(t, p)| {
                if let Some(name) = t.strip_prefix('{').and_then(|t| t.strip_suffix('}')) {
                    params.insert(name.to_string(), Value::String(url_decode(p)));
                    true
                } else {
                    t == p
                }
            });
            if ok {
                return Some((op, params));
            }
        }
        None
    }
}

/// `%xx` and `+` of a URL part.
pub fn url_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < b.len() {
        match b[i] {
            b'%' if i + 2 < b.len() => match u8::from_str_radix(&s[i + 1..i + 3], 16) {
                Ok(x) => {
                    out.push(x);
                    i += 3;
                    continue;
                }
                Err(_) => out.push(b'%'),
            },
            b'+' => out.push(b' '),
            c => out.push(c),
        }
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// Applies a mapping document (`mappings:` of fields) in `env`.
pub fn apply(doc: &Yaml, env: &Env) -> Result<Value, String> {
    let mut out = Map::new();
    apply_list(&doc["mappings"], env, &mut out)?;
    Ok(Value::Object(out))
}

fn apply_list(list: &Yaml, env: &Env, out: &mut Map<String, Value>) -> Result<(), String> {
    let Some(items) = list.as_vec() else {
        return Ok(());
    };
    for item in items {
        let Some(h) = item.as_hash() else { continue };
        for (k, spec) in h {
            let Some(k) = s(k) else { continue };
            if let Some(v) = value(spec, env)? {
                out.insert(k, v);
            }
        }
    }
    Ok(())
}

/// The value of one mapped field (None: left out).
fn value(spec: &Yaml, env: &Env) -> Result<Option<Value>, String> {
    let ctx = |e: expr::Error, what: &str| format!("{what}: {e}");
    if !spec["foreach"].is_badvalue() {
        let f = &spec["foreach"];
        let input = s(&f["input"]).unwrap_or_default();
        let items = env
            .eval(&expr::parse(&input).map_err(|e| ctx(e, &input))?)
            .map_err(|e| ctx(e, &input))?;
        let items = match items {
            None => Vec::new(),
            Some(Value::Array(a)) => a,
            Some(o) => vec![o],
        };
        let mut out = Vec::new();
        for it in items {
            let mut e = env.clone();
            e.vars.insert("item".into(), it);
            let mut m = Map::new();
            apply_list(&f["mappings"], &e, &mut m)?;
            out.push(Value::Object(m));
        }
        return Ok(Some(Value::Array(out)));
    }
    if !spec["mappings"].is_badvalue() {
        let mut m = Map::new();
        apply_list(&spec["mappings"], env, &mut m)?;
        return Ok(Some(Value::Object(m)));
    }
    match &spec["expression"] {
        Yaml::String(t) => {
            return env
                .eval(&expr::parse(t).map_err(|e| ctx(e, t))?)
                .map_err(|e| ctx(e, t));
        }
        Yaml::Integer(i) => return Ok(Some(Value::from(*i))),
        Yaml::Real(r) => return Ok(r.parse::<f64>().ok().map(Value::from)),
        Yaml::Boolean(b) => return Ok(Some(Value::Bool(*b))),
        _ => {}
    }
    match &spec["template"] {
        Yaml::BadValue | Yaml::Null => Ok(None),
        t => {
            let t = s(t).unwrap_or_default();
            expr::template(env, &t).map_err(|e| ctx(e, &t))
        }
    }
}

/// Whether a response condition holds.
pub fn condition(c: &Yaml, env: &Env) -> Result<bool, String> {
    match c {
        Yaml::Boolean(b) => Ok(*b),
        Yaml::String(t) => {
            let v = env
                .eval(&expr::parse(t).map_err(|e| format!("{t}: {e}"))?)
                .map_err(|e| format!("{t}: {e}"))?;
            Ok(matches!(v, Some(Value::Bool(true))))
        }
        _ => Ok(false),
    }
}

/// The directory of an API project: `src/main` of a z/OS Connect project,
/// or the directory itself.
pub fn project_dir(p: &Path) -> PathBuf {
    let m = p.join("src").join("main");
    if m.join("api").is_dir() {
        m
    } else {
        p.to_path_buf()
    }
}
