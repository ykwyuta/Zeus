//! Versioned Zeus run-unit checkpoints. Host runtimes must provide their own
//! transaction/restart protocol; never silently save only half of such a run.
use super::*;
use serde::{Deserialize, Serialize};
use std::io::Write;

const ARCHIVE_MAGIC: &[u8] = b"ZEUSCKP1\n";

fn checkpoint_entries(bytes: &[u8]) -> RResult<Vec<(u64, &[u8])>> {
    if bytes.is_empty() {
        return Ok(vec![]);
    }
    if !bytes.starts_with(ARCHIVE_MAGIC) {
        return Ok(vec![(1, bytes)]);
    }
    let mut entries = vec![];
    let mut position = ARCHIVE_MAGIC.len();
    let mut previous = 0u64;
    while position < bytes.len() {
        let header = bytes
            .get(position..position + 16)
            .ok_or_else(|| RunError("truncated checkpoint archive header".into()))?;
        let id = u64::from_be_bytes(header[..8].try_into().unwrap());
        let size = usize::try_from(u64::from_be_bytes(header[8..].try_into().unwrap()))
            .map_err(|_| RunError("checkpoint archive entry too large".into()))?;
        if id
            != previous
                .checked_add(1)
                .ok_or_else(|| RunError("checkpoint identifier overflow".into()))?
        {
            return err("invalid checkpoint archive identifier");
        }
        position += 16;
        let end = position
            .checked_add(size)
            .ok_or_else(|| RunError("checkpoint archive size overflow".into()))?;
        let image = bytes
            .get(position..end)
            .ok_or_else(|| RunError("truncated checkpoint archive entry".into()))?;
        entries.push((id, image));
        previous = id;
        position = end;
    }
    Ok(entries)
}

fn decode_image(bytes: &[u8]) -> RResult<Image> {
    let mut decoder = serde_json::Deserializer::from_slice(bytes);
    decoder.disable_recursion_limit();
    let image = Image::deserialize(&mut decoder)
        .map_err(|e| RunError(format!("invalid RERUN checkpoint: {e}")))?;
    decoder
        .end()
        .map_err(|e| RunError(format!("invalid RERUN checkpoint: {e}")))?;
    Ok(image)
}

#[derive(Serialize, Deserialize)]
struct Image {
    #[serde(default)]
    recycled_file_states: Vec<usize>,
    #[serde(default)]
    recycled_program_storage: Vec<(usize, usize)>,
    #[serde(default = "default_range_checking")]
    range_checking: bool,
    opened: Vec<(String, zeus_ds::format::RecordFormat, u32)>,
    opened_buffers: Vec<(String, u32, u32)>,
    sort_areas: Vec<Vec<usize>>,
    ccsid: u32,
    #[serde(default)]
    codepage_override: bool,
    search_dirs: Vec<PathBuf>,
    version: u32,
    state: RunState,
    mem: Vec<u8>,
    instances: Vec<Instance>,
    files: Vec<FileState>,
    storage: crate::hlasm::DynamicStorage,
    conditions: le::handlers::Conditions,
    externals: HashMap<String, (usize, u32)>,
    ext_files: HashMap<String, usize>,
    by_name: HashMap<String, usize>,
    entries: HashMap<u64, String>,
    #[serde(default)]
    entry_interfaces: HashMap<u64, zir::CallInterface>,
    dd: zeus_ds::dd::DdTable,
    switches: HashMap<String, bool>,
    tape_positions: HashMap<PathBuf, usize>,
    stdin: Option<std::collections::VecDeque<String>>,
    random: u64,
    return_code: i32,
    size_error: bool,
    io_cond: bool,
    last_status: [u8; 2],
    last_open_mode: Option<zir::OpenMode>,
    call_exception: bool,
    call_value: Option<Vec<u8>>,
    overflow: bool,
    step: StepInfo,
    control_blocks: bool,
    debug: bool,
    suppress_printing: bool,
    sort_records: Vec<Vec<u8>>,
    #[serde(default)]
    active_sort_controls: Option<super::sort::Controls>,
    sort_return: Option<(Vec<Vec<u8>>, usize)>,
}

fn default_range_checking() -> bool {
    true
}

impl Machine {
    pub(super) fn validate_rerun_allocations(&self, instance: usize) -> RResult<()> {
        for rule in &self.instances[instance].code.rerun {
            self.checkpoint_path(&rule.checkpoint)?;
        }
        Ok(())
    }
    pub(super) fn rerun_sort(&mut self, cur: usize) -> RResult<()> {
        if self
            .active_sort_controls
            .as_ref()
            .is_some_and(|c| c.checkpoint)
        {
            let path = self.checkpoint_path("SORTCKPT")?;
            if !self.pending_checkpoints.contains(&path) {
                self.pending_checkpoints.push(path);
            }
            self.sort_checkpoint_pending = true;
        }
        let module = self.instances[cur].module.clone();
        let rules: Vec<_> = self
            .instances
            .iter()
            .filter(|i| Rc::ptr_eq(&i.module, &module))
            .flat_map(|i| i.code.rerun.iter().filter(|r| r.file.is_none()).cloned())
            .collect();
        for rule in rules {
            let path = self.checkpoint_path(&rule.checkpoint)?;
            if !self.pending_checkpoints.contains(&path) {
                self.pending_checkpoints.push(path);
            }
            self.sort_checkpoint_pending = true;
        }
        Ok(())
    }
    pub(super) fn rerun_volume(&mut self, cur: usize, file: u32) -> RResult<()> {
        for rule in self.instances[cur].code.rerun.clone() {
            if rule.file == Some(file) && rule.records.is_none() {
                let path = self.checkpoint_path(&rule.checkpoint)?;
                if !self.pending_checkpoints.contains(&path) {
                    self.pending_checkpoints.push(path);
                }
            }
        }
        Ok(())
    }
    pub fn checkpoint_files(&self) -> &[PathBuf] {
        &self.checkpoint_files
    }

    /// Identifiers retained in a Zeus checkpoint data set, in write order.
    pub fn checkpoint_ids(path: &std::path::Path) -> RResult<Vec<u64>> {
        let bytes = std::fs::read(path).map_err(|e| RunError(e.to_string()))?;
        let entries = checkpoint_entries(&bytes)?;
        for (_, image) in &entries {
            decode_image(image)?;
        }
        Ok(entries.into_iter().map(|(id, _)| id).collect())
    }

    fn checkpoint_path(&self, name: &str) -> RResult<PathBuf> {
        fn identity(path: &std::path::Path) -> RResult<PathBuf> {
            if let Ok(path) = path.canonicalize() {
                return Ok(path);
            }
            let absolute = if path.is_absolute() {
                path.to_path_buf()
            } else {
                std::env::current_dir()
                    .map_err(|e| RunError(e.to_string()))?
                    .join(path)
            };
            let mut normalized = PathBuf::new();
            for component in absolute.components() {
                match component {
                    std::path::Component::CurDir => (),
                    std::path::Component::ParentDir => {
                        normalized.pop();
                    }
                    other => normalized.push(other.as_os_str()),
                }
            }
            if let (Some(parent), Some(name)) = (normalized.parent(), normalized.file_name())
                && let Ok(parent) = parent.canonicalize()
            {
                return Ok(parent.join(name));
            }
            Ok(normalized)
        }
        let path = self
            .dd
            .resolve(&[name])
            .ok_or_else(|| RunError(format!("RERUN checkpoint DD {name} is not allocated")))?;
        for instance in &self.instances {
            for file in &instance.code.files {
                let other = self
                    .dd
                    .resolve(&[&file.assign, &file.name])
                    .unwrap_or_else(|| file.assign.clone().into());
                if identity(&other)? == identity(&path)? {
                    return err("RERUN checkpoint cannot share a data-file path");
                }
            }
        }
        Ok(path)
    }

    pub(super) fn rerun_record(&mut self, cur: usize, file: u32) -> RResult<()> {
        if self.last_status != *b"00" && self.last_status != *b"02" {
            return Ok(());
        }
        let rules = self.instances[cur].code.rerun.clone();
        for (index, rule) in rules.iter().enumerate() {
            if rule.file != Some(file) {
                continue;
            }
            let Some(period) = rule.records else {
                continue;
            };
            let count = &mut self.instances[cur].rerun_counts[index];
            *count += 1;
            if count.is_multiple_of(period as u64) {
                let path = self.checkpoint_path(&rule.checkpoint)?;
                if !self.pending_checkpoints.contains(&path) {
                    self.pending_checkpoints.push(path);
                }
            }
        }
        Ok(())
    }

    pub(super) fn write_pending_checkpoints(&mut self, state: &RunState) -> RResult<()> {
        if self.pending_checkpoints.is_empty() {
            return Ok(());
        }
        if self.procedure_depth != 0 && self.active_sort_areas.is_empty() {
            return Ok(());
        }
        if self.exec_host.is_some()
            || self.linker_used
            || !self.hlasm.is_empty()
            || self.hlasm_call_depth != 0
            || self.le_conditions.frames.len() > state.stack.len() + 1
            || self.stdin_lines.is_some()
            || (!self.active_sort_areas.is_empty() && !self.sort_checkpoint_pending)
        {
            return err(
                "RERUN cannot checkpoint an active external runtime, console stream, or SORT procedure",
            );
        }
        let paths = std::mem::take(&mut self.pending_checkpoints);
        let base = std::env::current_dir().map_err(|e| RunError(e.to_string()))?;
        let absolute = |path: &PathBuf| {
            if path.is_relative() {
                base.join(path)
            } else {
                path.clone()
            }
        };
        let mut dd = self.dd.clone();
        let names: Vec<_> = self
            .instances
            .iter()
            .flat_map(|i| {
                i.code
                    .files
                    .iter()
                    .flat_map(|f| [f.assign.clone(), f.name.clone()])
                    .chain(i.code.rerun.iter().map(|r| r.checkpoint.clone()))
            })
            .collect();
        dd.anchor(&base, &names);
        for instance in &self.instances {
            for file in &instance.code.files {
                if self.step.dds.is_none() && dd.resolve(&[&file.assign, &file.name]).is_none() {
                    dd.set(&file.assign, absolute(&file.assign.clone().into()));
                }
            }
        }
        let mut files = self.file_states.clone();
        for file in &mut files {
            file.anchor_path(&base);
        }
        self.return_code = 0;
        self.store_rc(state.cur);
        let image = Image {
            recycled_file_states: self.recycled_file_states.clone(),
            recycled_program_storage: self.recycled_program_storage.clone(),
            range_checking: self.range_checking,
            opened: self.opened.clone(),
            opened_buffers: self.opened_buffers.clone(),
            sort_areas: self.active_sort_areas.clone(),
            ccsid: crate::codepage::host_ccsid(),
            codepage_override: crate::codepage::has_override(),
            search_dirs: self.search_dirs.iter().map(absolute).collect(),
            version: 1,
            state: state.clone(),
            mem: self.mem.clone(),
            instances: self.instances.clone(),
            files,
            storage: self.hlasm_storage.clone(),
            conditions: self.le_conditions.clone(),
            externals: self.externals.clone(),
            ext_files: self.ext_files.clone(),
            by_name: self.by_name.clone(),
            entries: self.entry_pointers.clone(),
            entry_interfaces: self.entry_interfaces.clone(),
            dd,
            switches: self.switches.clone(),
            tape_positions: self
                .tape_positions
                .iter()
                .map(|(path, pos)| (absolute(path), *pos))
                .collect(),
            stdin: self.stdin_buffer.clone(),
            random: self.random,
            return_code: self.return_code,
            size_error: self.size_error,
            io_cond: self.io_cond,
            last_status: self.last_status,
            last_open_mode: self.last_open_mode,
            call_exception: self.call_exception,
            call_value: self.call_value.clone(),
            overflow: self.overflow,
            step: self.step.clone(),
            control_blocks: self.control_blocks,
            debug: self.debug,
            suppress_printing: self.suppress_printing,
            sort_records: self.sort_records.clone(),
            active_sort_controls: self.active_sort_controls.clone(),
            sort_return: self.sort_return.clone(),
        };
        for path in paths {
            let multiple = self
                .instances
                .iter()
                .flat_map(|i| &i.code.rerun)
                .any(|rule| {
                    self.dd
                        .io_attributes(&[&rule.checkpoint])
                        .checkpoint_multiple
                        && self.dd.resolve(&[&rule.checkpoint]).as_ref() == Some(&path)
                });
            let mut created = false;
            let temporary =
                path.with_extension(format!("ckpt-{}-{}.tmp", std::process::id(), self.ticks));
            let result = (|| -> Result<(), String> {
                let mut file = std::fs::OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .open(&temporary)
                    .map_err(|e| e.to_string())?;
                created = true;
                if multiple {
                    let bytes = match std::fs::read(&path) {
                        Ok(bytes) => bytes,
                        Err(error) if error.kind() == std::io::ErrorKind::NotFound => vec![],
                        Err(error) => return Err(error.to_string()),
                    };
                    let entries = checkpoint_entries(&bytes).map_err(|e| e.0)?;
                    file.write_all(ARCHIVE_MAGIC).map_err(|e| e.to_string())?;
                    let mut next = 1u64;
                    for (id, bytes) in entries {
                        decode_image(bytes).map_err(|e| e.0)?;
                        file.write_all(&id.to_be_bytes())
                            .map_err(|e| e.to_string())?;
                        file.write_all(&(bytes.len() as u64).to_be_bytes())
                            .map_err(|e| e.to_string())?;
                        file.write_all(bytes).map_err(|e| e.to_string())?;
                        next = id.checked_add(1).ok_or("checkpoint identifier overflow")?;
                    }
                    let bytes = serde_json::to_vec(&image).map_err(|e| e.to_string())?;
                    file.write_all(&next.to_be_bytes())
                        .map_err(|e| e.to_string())?;
                    file.write_all(&(bytes.len() as u64).to_be_bytes())
                        .map_err(|e| e.to_string())?;
                    file.write_all(&bytes).map_err(|e| e.to_string())?;
                } else {
                    serde_json::to_writer(&mut file, &image).map_err(|e| e.to_string())?;
                }
                file.sync_all().map_err(|e| e.to_string())?;
                drop(file);
                std::fs::rename(&temporary, &path).map_err(|e| e.to_string())
            })();
            if let Err(error) = result {
                if created {
                    let _ = std::fs::remove_file(&temporary);
                }
                return err(format!("RERUN checkpoint {}: {error}", path.display()));
            }
            if !self.checkpoint_files.contains(&path) {
                self.checkpoint_files.push(path);
            }
        }
        self.sort_checkpoint_pending = false;
        Ok(())
    }

    /// Resume a saved Zeus run in this Machine, retaining its output sink and
    /// registered host functions. Already emitted console output is not undone.
    pub fn restart_checkpoint(&mut self, path: &std::path::Path) -> RResult<i32> {
        self.restart_checkpoint_at(path, None)
    }

    /// Select an earlier checkid, or the latest image when no id is supplied.
    pub fn restart_checkpoint_at(
        &mut self,
        path: &std::path::Path,
        checkid: Option<u64>,
    ) -> RResult<i32> {
        if self.hlasm_call_depth != 0 || self.exec_host.is_some() || self.linker_used {
            return err("RERUN restart requires a standalone COBOL run unit");
        }
        let bytes = std::fs::read(path).map_err(|e| RunError(e.to_string()))?;
        let entries = checkpoint_entries(&bytes)?;
        let selected = match checkid {
            Some(id) => entries.iter().find(|(key, _)| *key == id),
            None => entries.last(),
        }
        .ok_or_else(|| RunError("checkpoint identifier not found".into()))?;
        let mut image = decode_image(selected.1)?;
        if image.version != 1
            || image.state.cur >= image.instances.len()
            || image
                .state
                .stack
                .iter()
                .any(|f| f.inst >= image.instances.len())
        {
            return err("unsupported or invalid RERUN checkpoint");
        }
        if !crate::codepage::set_default(image.ccsid)
            && crate::codepage::active().ccsid != image.ccsid
        {
            return err("checkpoint CCSID differs from the active runtime CCSID");
        }
        // serde recreates Rc values independently. Restore module identity so
        // nested and COMMON name resolution reuses the saved instances.
        let mut modules: HashMap<Vec<u8>, Rc<Module>> = HashMap::new();
        if image.instances.iter().any(|instance| {
            instance
                .static_calls
                .values()
                .any(|&target| image.instances.get(target).is_none_or(|i| i.retired))
        }) || image
            .by_name
            .values()
            .any(|&target| image.instances.get(target).is_none_or(|i| i.retired))
        {
            return err("invalid program binding in checkpoint");
        }
        for instance in &mut image.instances {
            if instance.module_index as usize >= instance.module.programs.len()
                || instance.static_base + instance.code.static_init.len() > image.mem.len()
                || instance.files.iter().any(|&f| f >= image.files.len())
                || instance.rerun_counts.len() != instance.code.rerun.len()
            {
                return err("invalid RERUN instance storage");
            }
            let key = crate::module_bytes(&instance.module).map_err(|e| RunError(e.to_string()))?;
            instance.module = modules
                .entry(key)
                .or_insert_with(|| instance.module.clone())
                .clone();
            instance.code =
                Rc::new(instance.module.programs[instance.module_index as usize].clone());
        }
        if image.recycled_program_storage.iter().any(|&(base, len)| {
            base.checked_add(len)
                .is_none_or(|end| end > image.mem.len())
        }) {
            return err("invalid recycled program storage in checkpoint");
        }
        if image
            .recycled_file_states
            .iter()
            .any(|&file| file >= image.files.len())
        {
            return err("invalid recycled file state in checkpoint");
        }
        self.recycled_file_states = image.recycled_file_states;
        self.recycled_program_storage = image.recycled_program_storage;
        self.mem = image.mem;
        self.range_checking = image.range_checking;
        crate::codepage::restore_override(image.codepage_override);
        self.opened = image.opened;
        self.opened_buffers = image.opened_buffers;
        self.native = None;
        self.search_dirs = image.search_dirs;
        self.instances = image.instances;
        self.file_states = image.files;
        self.hlasm_storage = image.storage;
        self.le_conditions = image.conditions;
        self.externals = image.externals;
        self.ext_files = image.ext_files;
        self.by_name = image.by_name;
        self.entry_pointers = image.entries;
        self.entry_interfaces = image.entry_interfaces;
        self.dd = image.dd;
        self.switches = image.switches;
        self.tape_positions = image.tape_positions;
        self.stdin_buffer = image.stdin;
        self.stdin_lines = None;
        self.random = image.random;
        self.return_code = image.return_code;
        self.size_error = image.size_error;
        self.io_cond = image.io_cond;
        self.last_status = image.last_status;
        self.last_open_mode = image.last_open_mode;
        self.call_exception = image.call_exception;
        self.call_value = image.call_value;
        self.overflow = image.overflow;
        self.step = image.step;
        self.control_blocks = image.control_blocks;
        self.debug = image.debug;
        self.suppress_printing = image.suppress_printing;
        self.sort_records = image.sort_records;
        self.active_sort_controls = image.active_sort_controls;
        self.sort_return = image.sort_return;
        self.pending_checkpoints.clear();
        self.active_sort_areas = image.sort_areas;
        self.sort_checkpoint_pending = false;
        self.compatibility_error = None;
        self.hlasm.clear();
        self.procedure_depth = 0;
        if let Some(coverage) = self.cov.as_mut() {
            coverage.clear();
        }
        let result = self.run_loop(&mut image.state);
        self.leave_language_frame(0);
        match result? {
            Exit::StopRun(rc) => Ok(rc),
            Exit::Returned => Ok(self.return_code),
        }
    }
}
