//! Run-unit allocation ownership, separate from HLASM invocation lifetime.
use std::collections::BTreeMap;

#[derive(Clone, serde::Serialize, serde::Deserialize)]
struct Block {
    length: usize,
    subpool: u8,
    frame: bool,
}

/// Use one manager for all assembler calls in a run unit. Dropping the run
/// unit releases the backing Vec and this metadata together.
#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct DynamicStorage {
    active: BTreeMap<usize, Block>,
    free: BTreeMap<usize, usize>,
    allocated: usize,
    limit: usize,
    protected: BTreeMap<usize, Vec<u8>>,
    charged: BTreeMap<usize, usize>,
    committed: usize,
    pinned: BTreeMap<usize, usize>,
}

impl Default for DynamicStorage {
    fn default() -> Self {
        Self::with_limit(64 << 20)
    }
}

impl DynamicStorage {
    pub fn with_limit(limit: usize) -> Self {
        Self {
            active: BTreeMap::new(),
            free: BTreeMap::new(),
            allocated: 0,
            limit,
            protected: BTreeMap::new(),
            charged: BTreeMap::new(),
            committed: 0,
            pinned: BTreeMap::new(),
        }
    }

    pub fn allocated_bytes(&self) -> usize {
        self.allocated
    }

    pub(crate) fn check_write(&self, address: usize, length: usize) -> Result<(), String> {
        let end = address.checked_add(length).ok_or("write range overflow")?;
        if length != 0
            && self
                .protected
                .range(..end)
                .any(|(start, bytes)| address < start + bytes.len())
        {
            return Err("storage protection exception: write to RSECT".into());
        }
        Ok(())
    }

    pub(crate) fn pin(&mut self, address: usize, length: usize) {
        self.pinned.insert(address, length);
    }
    pub(crate) fn unpin(&mut self, address: usize) {
        self.pinned.remove(&address);
    }
    fn check_release(&self, address: usize, length: usize) -> Result<(), String> {
        let end = address
            .checked_add(length)
            .ok_or("release range overflow")?;
        if self
            .pinned
            .range(..end)
            .any(|(start, length)| address < start + length)
        {
            return Err("storage is retained by an open data control block".into());
        }
        Ok(())
    }

    pub(crate) fn protect(
        &mut self,
        module: &super::Module,
        memory: &[u8],
        base: usize,
    ) -> Result<(), String> {
        module.validate_read_only()?;
        if module.read_only.is_empty() {
            return Ok(());
        }
        let initial = module.storage_at(base)?;
        for region in &module.read_only {
            let start = base
                .checked_add(region.start as usize)
                .ok_or("RSECT address overflow")?;
            let length = (region.end - region.start) as usize;
            let expected = &initial[region.start as usize..region.end as usize];
            if memory.get(start..start + length) != Some(expected) {
                return Err("RSECT storage was modified outside assembler execution".into());
            }
            self.protected
                .entry(start)
                .or_insert_with(|| expected.to_vec());
        }
        Ok(())
    }

    pub(crate) fn verify_protection(&self, memory: &mut [u8]) -> Result<(), String> {
        let mut changed = false;
        for (&start, bytes) in &self.protected {
            let area = memory
                .get_mut(start..start + bytes.len())
                .ok_or("RSECT storage outside run unit")?;
            if area != bytes {
                area.copy_from_slice(bytes);
                changed = true;
            }
        }
        if changed {
            Err("storage protection exception: write to RSECT".into())
        } else {
            Ok(())
        }
    }

    fn insert_free(&mut self, mut start: usize, mut length: usize) {
        if length == 0 {
            return;
        }
        if let Some((&previous, &size)) = self.free.range(..start).next_back()
            && previous + size == start
        {
            self.free.remove(&previous);
            start = previous;
            length += size;
        }
        if let Some(size) = self.free.remove(&(start + length)) {
            length += size;
        }
        self.free.insert(start, length);
    }

    fn trim(&mut self, memory: &mut Vec<u8>) {
        while let Some((&start, &length)) = self.free.last_key_value() {
            if start + length != memory.len() {
                break;
            }
            self.free.remove(&start);
            memory.truncate(start);
            for (_, size) in self.charged.split_off(&start) {
                self.committed -= size;
            }
            if let Some((&address, &size)) = self.charged.last_key_value()
                && address + size > start
            {
                self.committed -= address + size - start;
                self.charged.insert(address, start - address);
            }
        }
    }

    fn reserve(
        &mut self,
        memory: &mut Vec<u8>,
        length: usize,
        alignment: usize,
        subpool: u8,
        ceiling: usize,
        frame: bool,
    ) -> Result<usize, String> {
        if length == 0 {
            return Err("storage length must be positive".into());
        }
        let length = if frame {
            length
        } else {
            length
                .checked_add(7)
                .map(|n| n & !7)
                .ok_or("storage length overflow")?
        };
        if !frame
            && self
                .allocated
                .checked_add(length)
                .is_none_or(|n| n > self.limit)
        {
            return Err("run-unit dynamic storage limit exceeded".into());
        }
        let reusable = self.free.iter().find_map(|(&start, &size)| {
            let aligned = start.checked_add(alignment - 1)? & !(alignment - 1);
            let end = aligned.checked_add(length)?;
            (end <= start + size && end <= ceiling).then_some((start, size, aligned))
        });
        let address = if let Some((start, size, aligned)) = reusable {
            self.free.remove(&start);
            self.insert_free(start, aligned - start);
            self.insert_free(aligned + length, start + size - aligned - length);
            memory[aligned..aligned + length].fill(0);
            aligned
        } else {
            let start = memory.len();
            let address = start
                .checked_add(alignment - 1)
                .map(|n| n & !(alignment - 1))
                .ok_or("storage address overflow")?;
            let end = address
                .checked_add(length)
                .filter(|n| *n <= ceiling)
                .ok_or("storage address limit exceeded")?;
            let growth = end - start;
            if !frame
                && self
                    .committed
                    .checked_add(growth)
                    .is_none_or(|size| size > self.limit.saturating_add(4096))
            {
                return Err("run-unit dynamic storage footprint limit exceeded".into());
            }
            memory
                .try_reserve(end - start)
                .map_err(|_| "host storage allocation failed")?;
            memory.resize(end, 0);
            if !frame {
                self.charged.insert(start, growth);
                self.committed += growth;
            }
            self.insert_free(start, address - start);
            address
        };
        self.active.insert(
            address,
            Block {
                length,
                subpool,
                frame,
            },
        );
        if !frame {
            self.allocated += length;
        }
        Ok(address)
    }

    pub(crate) fn invocation(
        &mut self,
        memory: &mut Vec<u8>,
        length: usize,
        mode: u8,
    ) -> Result<usize, String> {
        let ceiling = match mode {
            24 => 1 << 24,
            31 => 0x7fff_0000,
            64 => isize::MAX as usize,
            _ => return Err("invalid invocation addressing mode".into()),
        };
        self.reserve(memory, length, 8, 0, ceiling, true)
    }
    pub(crate) fn frame(&mut self, memory: &mut Vec<u8>, length: usize) -> Result<usize, String> {
        self.reserve(memory, length, 8, 0, 0x7fff_0000, true)
    }

    pub(crate) fn module(
        &mut self,
        memory: &mut Vec<u8>,
        module: &super::Module,
    ) -> Result<usize, String> {
        let base = self.reserve(
            memory,
            module.storage.len(),
            module.required_alignment()?,
            0,
            if module.rmode == 24 || !module.dcbs.is_empty() {
                1 << 24
            } else {
                0x7fff_0000
            },
            true,
        )?;
        let result = module.storage_at(base).and_then(|image| {
            memory[base..base + image.len()].copy_from_slice(&image);
            self.protect(module, memory, base)
        });
        if let Err(e) = result {
            self.release_frame(memory, base);
            return Err(e);
        }
        Ok(base)
    }

    pub(crate) fn release_module(
        &mut self,
        memory: &mut Vec<u8>,
        module: &super::Module,
        base: usize,
    ) {
        for region in &module.read_only {
            self.protected.remove(&(base + region.start as usize));
        }
        self.release_frame(memory, base);
    }

    pub(crate) fn release_frame(&mut self, memory: &mut Vec<u8>, address: usize) {
        let block = self
            .active
            .remove(&address)
            .expect("owned invocation frame");
        debug_assert!(block.frame);
        memory[address..address + block.length].fill(0);
        self.insert_free(address, block.length);
        self.trim(memory);
    }

    pub fn obtain(
        &mut self,
        memory: &mut Vec<u8>,
        length: usize,
        subpool: u8,
        below_16m: bool,
        page: bool,
    ) -> Result<usize, String> {
        self.reserve(
            memory,
            length,
            if page { 4096 } else { 8 },
            subpool,
            if below_16m { 1 << 24 } else { 0x7fff_0000 },
            false,
        )
    }

    /// Release a doubleword-granular portion of an owned block. Static data and
    /// live invocation frames cannot be released through application services.
    pub fn release(
        &mut self,
        memory: &mut Vec<u8>,
        address: usize,
        length: usize,
        subpool: u8,
    ) -> Result<(), String> {
        if !address.is_multiple_of(8) || length == 0 {
            return Err("invalid storage release address or length".into());
        }
        let length = length
            .checked_add(7)
            .map(|n| n & !7)
            .ok_or("storage release length overflow")?;
        let end = address
            .checked_add(length)
            .ok_or("storage release address overflow")?;
        self.check_release(address, length)?;
        let (&start, block) = self
            .active
            .range(..=address)
            .next_back()
            .ok_or("storage is not allocated")?;
        if block.frame || block.subpool != subpool || end > start + block.length {
            return Err("storage release does not match an owned allocation/subpool".into());
        }
        let block = self.active.remove(&start).unwrap();
        if address > start {
            self.active.insert(
                start,
                Block {
                    length: address - start,
                    ..block.clone()
                },
            );
        }
        if end < start + block.length {
            self.active.insert(
                end,
                Block {
                    length: start + block.length - end,
                    ..block
                },
            );
        }
        memory[address..end].fill(0);
        self.allocated -= length;
        self.insert_free(address, length);
        self.trim(memory);
        Ok(())
    }

    pub fn release_subpool(&mut self, memory: &mut Vec<u8>, subpool: u8) -> Result<(), String> {
        let blocks: Vec<_> = self
            .active
            .iter()
            .filter(|(_, b)| !b.frame && b.subpool == subpool)
            .map(|(&a, b)| (a, b.length))
            .collect();
        for (address, length) in &blocks {
            self.check_release(*address, *length)?;
        }
        for (a, length) in blocks {
            self.release(memory, a, length, subpool)
                .expect("owned subpool allocation");
        }
        Ok(())
    }
}
