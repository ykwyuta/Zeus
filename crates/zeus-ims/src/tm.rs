//! Local, durable IMS TM message queue.  The lock is held only while a queue
//! operation runs, so producers and message-driven programs can run together.
use std::collections::HashMap;
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::Path;
use std::sync::Mutex;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

const MAGIC: &[u8; 8] = b"IMSTM003";
const MAX_SEGMENT: usize = 32767;
const MAX_MESSAGE: usize = 1_048_576;
static TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(1);
static PROCESS_OWNER: OnceLock<u64> = OnceLock::new();
static LEASES: OnceLock<Mutex<HashMap<(std::path::PathBuf, u64), File>>> = OnceLock::new();

fn leases() -> &'static Mutex<HashMap<(std::path::PathBuf, u64), File>> {
    LEASES.get_or_init(|| Mutex::new(HashMap::new()))
}

/// An owner id of this process for leased TM messages.
pub fn process_owner() -> u64 {
    *PROCESS_OWNER.get_or_init(|| {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let token = (nanos as u64) ^ ((nanos >> 64) as u64) ^ ((std::process::id() as u64) << 32);
        token.max(1)
    })
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Message {
    pub id: u64,
    pub destination: [u8; 8],
    pub source: [u8; 8],
    pub data: Vec<u8>,
    owner: u64,
}

#[derive(Default)]
struct State {
    next_id: u64,
    messages: Vec<Message>,
}

pub struct Queue {
    directory: std::path::PathBuf,
}

fn take<const N: usize>(input: &mut &[u8]) -> Result<[u8; N], String> {
    let data = input.get(..N).ok_or("truncated IMS TM queue")?;
    *input = &input[N..];
    Ok(data.try_into().unwrap())
}

fn checksum(data: &[u8]) -> u32 {
    data.iter().fold(0x811c9dc5_u32, |hash, byte| {
        (hash ^ u32::from(*byte)).wrapping_mul(0x01000193)
    })
}

fn parse(mut data: &[u8]) -> Result<State, String> {
    if data.is_empty() {
        return Ok(State {
            next_id: 1,
            messages: Vec::new(),
        });
    }
    if &take::<8>(&mut data)? != MAGIC {
        return Err("invalid IMS TM queue magic".into());
    }
    let next_id = u64::from_be_bytes(take(&mut data)?);
    let count = u32::from_be_bytes(take(&mut data)?) as usize;
    let expected_checksum = u32::from_be_bytes(take(&mut data)?);
    if checksum(data) != expected_checksum {
        return Err("IMS TM queue checksum mismatch".into());
    }
    if count > 1_000_000 || count > data.len() / 40 {
        return Err("IMS TM queue is too large".into());
    }
    let mut messages = Vec::with_capacity(count);
    for _ in 0..count {
        let id = u64::from_be_bytes(take(&mut data)?);
        let destination = take(&mut data)?;
        let source = take(&mut data)?;
        let owner = u64::from_be_bytes(take(&mut data)?);
        let len = u32::from_be_bytes(take(&mut data)?) as usize;
        if !(4..=MAX_MESSAGE).contains(&len) {
            return Err("invalid IMS TM message length".into());
        }
        let body = data.get(..len).ok_or("truncated IMS TM message")?;
        validate_message(body)?;
        messages.push(Message {
            id,
            destination,
            source,
            owner,
            data: body.to_vec(),
        });
        data = &data[len..];
    }
    if !data.is_empty() {
        return Err("trailing IMS TM queue bytes".into());
    }
    Ok(State { next_id, messages })
}

fn serialize(state: &State) -> Result<Vec<u8>, String> {
    let count = u32::try_from(state.messages.len()).map_err(|_| "IMS TM queue is too large")?;
    let mut payload = Vec::new();
    for message in &state.messages {
        payload.extend_from_slice(&message.id.to_be_bytes());
        payload.extend_from_slice(&message.destination);
        payload.extend_from_slice(&message.source);
        payload.extend_from_slice(&message.owner.to_be_bytes());
        payload.extend_from_slice(&(message.data.len() as u32).to_be_bytes());
        payload.extend_from_slice(&message.data);
    }
    let mut bytes = Vec::with_capacity(24 + payload.len());
    bytes.extend_from_slice(MAGIC);
    bytes.extend_from_slice(&state.next_id.to_be_bytes());
    bytes.extend_from_slice(&count.to_be_bytes());
    bytes.extend_from_slice(&checksum(&payload).to_be_bytes());
    bytes.extend_from_slice(&payload);
    Ok(bytes)
}

fn validate_message(data: &[u8]) -> Result<(), String> {
    if data.is_empty() || data.len() > MAX_MESSAGE {
        return Err("invalid IMS TM message size".into());
    }
    let mut rest = data;
    while !rest.is_empty() {
        if rest.len() < 4 {
            return Err("truncated IMS TM segment header".into());
        }
        let length = u16::from_be_bytes([rest[0], rest[1]]) as usize;
        if !(4..=MAX_SEGMENT).contains(&length) || length > rest.len() {
            return Err("invalid IMS TM segment LL".into());
        }
        rest = &rest[length..];
    }
    Ok(())
}

#[cfg(not(windows))]
fn replace_queue_file(temp: &Path, path: &Path, directory: &Path) -> Result<(), String> {
    fs::rename(temp, path).map_err(|e| format!("replace IMS TM queue {}: {e}", path.display()))?;
    File::open(directory)
        .and_then(|file| file.sync_all())
        .map_err(|e| format!("sync IMS TM queue directory {}: {e}", directory.display()))?;
    Ok(())
}

#[cfg(windows)]
fn replace_queue_file(temp: &Path, path: &Path, _directory: &Path) -> Result<(), String> {
    // Windows cannot fsync a directory through std::fs::File. This safe wrapper
    // uses MoveFileExW(REPLACE_EXISTING | WRITE_THROUGH); the temporary file's
    // contents have already been synchronized, and both paths are in the same
    // directory. Do not turn a failed durable replacement into a successful commit.
    atomicwrites::replace_atomic(temp, path)
        .map_err(|e| format!("replace IMS TM queue {}: {e}", path.display()))
}

impl Queue {
    pub fn open(directory: impl AsRef<Path>) -> Result<Self, String> {
        fs::create_dir_all(directory.as_ref()).map_err(|e| e.to_string())?;
        let directory = fs::canonicalize(directory).map_err(|e| e.to_string())?;
        fs::create_dir_all(directory.join("leases")).map_err(|e| e.to_string())?;
        Ok(Self { directory })
    }

    fn transaction<T>(
        &self,
        update: impl FnOnce(&mut State) -> Result<(T, bool), String>,
    ) -> Result<T, String> {
        let lock_path = self.directory.join("queue.lock");
        let lock = OpenOptions::new()
            .create(true)
            .truncate(false)
            .write(true)
            .open(lock_path)
            .map_err(|e| e.to_string())?;
        lock.lock().map_err(|e| e.to_string())?;
        let path = self.directory.join("queue.dat");
        let mut contents = Vec::new();
        match File::open(&path) {
            Ok(mut file) => file.read_to_end(&mut contents).map_err(|e| e.to_string())?,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => 0,
            Err(e) => return Err(e.to_string()),
        };
        let mut state = parse(&contents)?;
        let (result, changed) = update(&mut state)?;
        if changed {
            let temp = self.directory.join(format!(
                "queue.{}.{}.tmp",
                std::process::id(),
                TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed)
            ));
            let write = (|| -> Result<(), String> {
                let mut file = OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .open(&temp)
                    .map_err(|e| e.to_string())?;
                file.write_all(&serialize(&state)?)
                    .map_err(|e| e.to_string())?;
                file.sync_all().map_err(|e| e.to_string())?;
                drop(file);
                replace_queue_file(&temp, &path, &self.directory)?;
                Ok(())
            })();
            if write.is_err() {
                let _ = fs::remove_file(temp);
            }
            write?;
        }
        Ok(result)
    }

    pub fn enqueue(
        &self,
        destination: [u8; 8],
        source: [u8; 8],
        data: &[u8],
    ) -> Result<u64, String> {
        validate_message(data)?;
        self.transaction(|state| {
            let id = state.next_id;
            state.next_id = id.checked_add(1).ok_or("IMS TM message ID overflow")?;
            state.messages.push(Message {
                id,
                destination,
                source,
                data: data.to_vec(),
                owner: 0,
            });
            Ok((id, true))
        })
    }

    pub fn get(&self, destination: [u8; 8], owner: u64) -> Result<Option<Message>, String> {
        if owner == 0 {
            return Err("IMS TM owner cannot be zero".into());
        }
        let leased = self.transaction(|state| {
            for message in &mut state.messages {
                if message.destination != destination {
                    continue;
                }
                let lock_path = self
                    .directory
                    .join("leases")
                    .join(format!("{}.lock", message.id));
                let file = OpenOptions::new()
                    .create(true)
                    .truncate(false)
                    .write(true)
                    .open(lock_path)
                    .map_err(|e| e.to_string())?;
                if file.try_lock().is_err() {
                    continue;
                }
                // An owner left in queue.dat without a held OS lock belonged to
                // a crashed process. Reclaim it immediately.
                message.owner = owner;
                return Ok((Some((message.clone(), file)), true));
            }
            Ok((None, false))
        })?;
        if let Some((message, file)) = leased {
            leases()
                .lock()
                .map_err(|e| e.to_string())?
                .insert((self.directory.clone(), message.id), file);
            Ok(Some(message))
        } else {
            Ok(None)
        }
    }

    pub fn complete(
        &self,
        owner: u64,
        inputs: &[u64],
        outputs: &[([u8; 8], [u8; 8], Vec<u8>)],
    ) -> Result<(), String> {
        self.transaction(|state| {
            for id in inputs {
                if !state
                    .messages
                    .iter()
                    .any(|m| m.id == *id && m.owner == owner)
                {
                    return Err(format!("IMS TM input {id} is not leased by this task"));
                }
            }
            for (_, _, data) in outputs {
                validate_message(data)?;
            }
            state.messages.retain(|m| !inputs.contains(&m.id));
            for (destination, source, data) in outputs {
                let id = state.next_id;
                state.next_id = id.checked_add(1).ok_or("IMS TM message ID overflow")?;
                state.messages.push(Message {
                    id,
                    destination: *destination,
                    source: *source,
                    data: data.clone(),
                    owner: 0,
                });
            }
            Ok(((), !inputs.is_empty() || !outputs.is_empty()))
        })?;
        let mut held = leases().lock().map_err(|e| e.to_string())?;
        for id in inputs {
            held.remove(&(self.directory.clone(), *id));
        }
        Ok(())
    }

    pub fn rollback(&self, owner: u64, inputs: &[u64]) -> Result<(), String> {
        self.transaction(|state| {
            let mut changed = false;
            for message in &mut state.messages {
                if inputs.contains(&message.id) && message.owner == owner {
                    message.owner = 0;
                    changed = true;
                }
            }
            Ok(((), changed))
        })?;
        let mut held = leases().lock().map_err(|e| e.to_string())?;
        for id in inputs {
            held.remove(&(self.directory.clone(), *id));
        }
        Ok(())
    }

    pub fn pending(&self, destination: [u8; 8]) -> Result<Vec<Message>, String> {
        self.transaction(|state| {
            Ok((
                state
                    .messages
                    .iter()
                    .filter(|m| m.destination == destination)
                    .cloned()
                    .collect(),
                false,
            ))
        })
    }
}
