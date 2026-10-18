//! COBOL static bindings and dynamic program cancellation.
use super::*;
use std::collections::HashSet;

impl Machine {
    pub(super) fn allocate_invocation_storage(
        &mut self,
        code: &ProgramCode,
    ) -> HashMap<u32, usize> {
        code.invocation_storage
            .iter()
            .map(|&index| {
                let desc = &code.descs[index as usize];
                let base = self.program_storage(
                    &code.static_init[desc.offset as usize..(desc.offset + desc.len) as usize],
                );
                (index, base)
            })
            .collect()
    }
    /// Imported names must be exported by their defining object. Reusing
    /// instantiate's object identity makes all DLL entry points share WS.
    pub(super) fn resolve_dll(
        &mut self,
        cur: usize,
        name: &str,
        allow_imports: bool,
    ) -> Option<usize> {
        if let Some(local) = self.resolve_local_program(cur, name) {
            return Some(local);
        }
        if let Some(&loaded) = self.by_name.get(name) {
            return self.instances[loaded].code.exportall.then_some(loaded);
        }
        for dir in self.search_dirs.clone() {
            let mut candidates: Vec<_> = std::fs::read_dir(&dir)
                .ok()
                .into_iter()
                .flatten()
                .filter_map(Result::ok)
                .map(|e| e.path())
                .filter(|p| {
                    allow_imports && p.extension().is_some_and(|e| e.eq_ignore_ascii_case("x"))
                })
                .collect();
            candidates.sort();
            // A literal import may refer to a differently named object via
            // its sidefile; an identifier with the module's name also works.
            let mut objects = vec![dir.join(format!("{name}.zo")), dir.join(name)];
            for side in candidates {
                let Some(side) = std::fs::read(side)
                    .ok()
                    .and_then(|b| serde_json::from_slice::<crate::dll::Sidefile>(&b).ok())
                else {
                    continue;
                };
                if side.format == "ZEUS-DLL-1"
                    && side.exports.iter().any(|e| e == name)
                    && std::path::Path::new(&side.module).components().count() == 1
                    && std::path::Path::new(&side.module)
                        .components()
                        .all(|component| matches!(component, std::path::Component::Normal(_)))
                {
                    objects.push(dir.join(side.module));
                }
            }
            for object in objects {
                let Ok(module) = crate::load_module(&object) else {
                    continue;
                };
                let Some(index) = module.programs.iter().position(|p| {
                    p.parent.is_none()
                        && p.exportall
                        && (p.name == name || p.entries.iter().any(|e| e.name == name))
                }) else {
                    continue;
                };
                let loaded = self.instantiate(&Rc::new(module), index as u32);
                return Some(loaded);
            }
        }
        None
    }
    pub(super) fn program_file_slot(&mut self) -> usize {
        self.recycled_file_states.pop().unwrap_or_else(|| {
            self.file_states.push(FileState::default());
            self.file_states.len() - 1
        })
    }
    pub(super) fn program_storage(&mut self, bytes: &[u8]) -> usize {
        if let Some(slot) = self
            .recycled_program_storage
            .iter()
            .position(|&(_, len)| len >= bytes.len())
        {
            let (base, len) = self.recycled_program_storage.swap_remove(slot);
            self.mem[base..base + len].fill(0);
            self.mem[base..base + bytes.len()].copy_from_slice(bytes);
            // Retain only an aligned remainder; program areas use the same
            // alignment as Machine::alloc.
            let used = (bytes.len() + 7) & !7;
            if used < len {
                self.recycled_program_storage
                    .push((base + used, len - used));
            }
            base
        } else {
            self.alloc(bytes)
        }
    }

    /// Resolve available COBOL static dependencies before entering a program.
    /// Host services/foreign objects retain their existing linker path.
    pub(super) fn bind_static_calls(&mut self, instance: usize) {
        let mut pending = vec![instance];
        while let Some(instance) = pending.pop() {
            let instance = self.instances[instance].activation_of.unwrap_or(instance);
            if self.instances[instance].bindings_ready {
                continue;
            }
            self.instances[instance].bindings_ready = true;
            let code = self.instances[instance].code.clone();
            let _codepage = crate::codepage::scope(Some(crate::codepage::runtime_ccsid(
                self.program_ccsid(instance),
            )));
            for instruction in &code.code {
                let (bytes, dynamic) = match instruction {
                    Instr::Call {
                        program: zir::Src::Alnum(bytes),
                        interface,
                        ..
                    } => (
                        bytes,
                        interface.unwrap_or(zir::CallInterface::default_for(code.dynam, code.dll)),
                    ),
                    Instr::SetEntry(_, zir::Src::Alnum(bytes)) => {
                        (bytes, zir::CallInterface::default_for(code.dynam, code.dll))
                    }
                    Instr::SetEntryWithInterface(_, zir::Src::Alnum(bytes), mode) => (bytes, *mode),
                    _ => continue,
                };
                if dynamic != zir::CallInterface::Static {
                    continue;
                }
                let name = ebcdic::decode(bytes).trim().to_ascii_uppercase();
                if let Some(target) = self.resolve_program(instance, &name) {
                    self.instances[instance].static_calls.insert(name, target);
                    pending.push(target);
                }
            }
        }
    }

    /// A dynamically loaded object's code can differ from the executable's
    /// machine code with the same name and instruction count.
    pub(super) fn dynamic_object(&self, instance: usize) -> bool {
        let module = &self.instances[instance].module;
        self.instances.iter().any(|i| {
            !i.retired
                && Rc::ptr_eq(&i.module, module)
                && (i.dll_linked || !i.dynamic_calls.is_empty())
        })
    }

    pub(super) fn reset_initial_children(&mut self, parent: usize) -> RResult<()> {
        let module = self.instances[parent].module.clone();
        let program = self.instances[parent].module_index;
        let mut children = self
            .instances
            .iter()
            .enumerate()
            .filter_map(|(index, instance)| {
                if instance.retired
                    || !Rc::ptr_eq(&instance.module, &module)
                    || instance.module_index == program
                {
                    return None;
                }
                let mut ancestor = instance.code.parent;
                while let Some(a) = ancestor {
                    if a == program {
                        return Some(index);
                    }
                    ancestor = module.programs[a as usize].parent;
                }
                None
            })
            .collect::<Vec<_>>();
        children.sort_by_key(|&i| std::cmp::Reverse(self.instances[i].module_index));
        for child in children {
            self.close_program_files(child)?;
            self.reset(child);
            let instance = &mut self.instances[child];
            instance.initialized = false;
            instance.perform.clear();
            instance.linkage.fill(None);
            instance.reports.fill(report::ReportState::default());
            instance.dynamic_calls.clear();
        }
        Ok(())
    }

    pub(super) fn cancel_cobol(&mut self, caller: usize, name: &str) -> RResult<()> {
        let module = self.instances[caller].module.clone();
        let me = self.instances[caller].module_index;
        // A CANCEL must not load or instantiate a program that was never called.
        let nested = self
            .instances
            .iter()
            .enumerate()
            .find_map(|(index, instance)| {
                if instance.retired
                    || instance.activation_of.is_some()
                    || !Rc::ptr_eq(&instance.module, &module)
                    || instance.code.parent.is_none()
                    || instance.code.name != name
                {
                    return None;
                }
                let mut ancestor = Some(me);
                while let Some(a) = ancestor {
                    if instance.code.parent == Some(a) && (a == me || instance.code.common) {
                        return Some(index);
                    }
                    ancestor = module.programs[a as usize].parent;
                }
                None
            });
        let Some(target) = nested.or_else(|| self.by_name.get(name).copied()) else {
            return Ok(());
        };
        let target_module = self.instances[target].module.clone();
        let target_index = self.instances[target].module_index;
        let external_object =
            self.instances[target].code.parent.is_none() && !Rc::ptr_eq(&module, &target_module);
        let group: HashSet<usize> = self
            .instances
            .iter()
            .enumerate()
            .filter_map(|(index, instance)| {
                if instance.retired || !Rc::ptr_eq(&instance.module, &target_module) {
                    return None;
                }
                if external_object {
                    return Some(index);
                }
                let mut ancestor = Some(instance.module_index);
                while let Some(a) = ancestor {
                    if a == target_index {
                        return Some(index);
                    }
                    ancestor = target_module.programs[a as usize].parent;
                }
                None
            })
            .collect();
        if group.iter().any(|&i| self.instances[i].active) {
            return err(format!(
                "CANCEL cannot cancel active program {name} or its containing object"
            ));
        }
        if !(self.instances[target].dynamic_calls.contains(name)
            || nested.is_some() && self.instances[target].initialized)
        {
            return Ok(());
        }
        // A static binding from outside the object keeps the program resident.
        if self.instances.iter().enumerate().any(|(i, instance)| {
            !instance.retired
                && !group.contains(&i)
                && instance
                    .static_calls
                    .values()
                    .any(|target| group.contains(target))
        }) {
            return Ok(());
        }
        let mut ordered: Vec<_> = group.iter().copied().collect();
        ordered.sort_by_key(|&i| std::cmp::Reverse(self.instances[i].module_index));
        let mut released = HashSet::new();
        let mut removed_names = HashSet::new();
        let mut released_files = HashSet::new();
        for index in ordered {
            self.close_program_files(index)?;
            if !external_object {
                self.reset(index);
            }
            let instance = &mut self.instances[index];
            instance.initialized = false;
            instance.perform.clear();
            instance.linkage.fill(None);
            instance.reports.fill(report::ReportState::default());
            instance.dynamic_calls.clear();
            if external_object {
                instance.retired = true;
                instance.static_calls.clear();
                instance.bindings_ready = false;
                removed_names.insert(instance.code.name.clone());
                removed_names.extend(instance.code.entries.iter().map(|e| e.name.clone()));
                released.insert((instance.static_base, instance.code.static_init.len()));
                for (&desc, &address) in &instance.invocation_bases {
                    released.insert((address, instance.code.descs[desc as usize].len as usize));
                }
                if let Some(base) = instance.local_base
                    && let Some((_, len)) = instance.code.local_storage
                {
                    released.insert((base, len as usize));
                }
                for (file, definition) in instance.files.iter().zip(&instance.code.files) {
                    if !definition.external && definition.owner.is_none() {
                        self.file_states[*file] = FileState::default();
                        released_files.insert(*file);
                    }
                }
            }
        }
        if external_object {
            self.by_name.retain(|_, index| !group.contains(index));
            self.entry_pointers
                .retain(|_, entry| !removed_names.contains(entry));
            self.recycled_program_storage
                .extend(released.into_iter().filter(|&(_, len)| len != 0));
            self.recycled_file_states.extend(released_files);
        }
        Ok(())
    }
}
