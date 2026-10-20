//! Bare-metal Hercules runner for ZHVS (HLASM Validation Suite) programs.
//!
//! Assembles each ZHVS program into an 80-byte IBM `OBJ` deck via `hlasm`,
//! links it with a bare-metal `ZHVRPT` report writer (and `HSS01A` for
//! `HS101A`), packs all programs into a single IPL-able `z/Arch` storage image,
//! runs the image on Hercules, and decodes the 120-byte EBCDIC report records.

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use anyhow::{Result, bail};
use serde::{Deserialize, Serialize};
use zeus_cobol::{ebcdic, hlasm};

use crate::hercules::Hercules;

/// Bare-metal `ZHVRPT` report writer that appends 120-byte EBCDIC lines into
/// the in-storage buffer starting at `0x40000` (with current write pointer at
/// `0x30000` and line count at `0x30004`).
const BARE_ZHVRPT_ASM: &str = "\
ZHVRPT   CSECT
         USING ZHVRPT,15
         STM   0,12,ZSAVE
         LR    11,15
         USING ZHVRPT,11
         DROP  15
         L     2,0(1)
         N     2,=X'7FFFFFFF'
         L     3,0(2)
         LTR   3,3
         BZ    ZDONE
         CH    3,=H'2'
         BE    ZDONE
         L     4,ZBUFPTR
         L     4,0(4)
         LTR   4,4
         BNZ   ZHAS
         L     4,ZBUFBASE
ZHAS     MVC   0(120,4),4(2)
         AHI   4,120
         L     5,ZBUFPTR
         ST    4,0(5)
         L     6,ZCNTADDR
         L     7,0(6)
         AHI   7,1
         ST    7,0(6)
ZDONE    LM    0,12,ZSAVE
         XR    15,15
         BR    14
ZSAVE    DS    13F
ZBUFPTR  DC    A(X'30000')
ZCNTADDR DC    A(X'30004')
ZBUFBASE DC    A(X'40000')
         LTORG
         END
";

/// Programs in `zhvs` that run on bare-metal Hercules 3.13 (`z/Arch`) without
/// requiring z/OS supervisor SVCs (`GETMAIN`/`OPEN`/`LINK`) or z15 `PFPO`.
pub const DEFAULT_PROGRAMS: &[(&str, &str)] = &[
    ("HA", "HA101A"),
    ("HA", "HA102A"),
    ("HA", "HA103A"),
    ("HA", "HA104A"),
    ("HA", "HA105A"),
    ("HM", "HM101A"),
    ("HM", "HM102A"),
    ("HM", "HM103A"),
    ("HM", "HM104A"),
    ("HI", "HI101A"),
    ("HI", "HI102A"),
    ("HI", "HI103A"),
    ("HI", "HI104A"),
    ("HI", "HI105A"),
    ("HI", "HI106A"),
    ("HI", "HI107A"),
    ("HI", "HI108A"),
    ("HI", "HI109A"),
    ("HD", "HD101A"),
    ("HD", "HD102A"),
    ("HD", "HD103A"),
    ("HD", "HD104A"),
    ("HD", "HD105A"),
    ("HD", "HD106A"),
    ("HF", "HF101A"),
    ("HF", "HF102A"),
    ("HF", "HF103A"),
    ("HS", "HS101A"),
];

const PROG_BASE: usize = 0x10000;
const PROG_SLOT: usize = 0x10000;
const TABLE_ADDR: usize = 0x2000;
const BUF_PTR_ADDR: usize = 0x30000;
const LINE_CNT_ADDR: usize = 0x30004;
const REPORT_BASE: usize = 0x40000;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ZhvsProgramResult {
    pub module: String,
    pub program: String,
    pub completed: bool,
    pub pic: u16,
    pub pass: usize,
    pub fail: usize,
    pub failed_checks: Vec<String>,
    pub report_lines: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ZhvsOracleRun {
    pub oracle: String,
    pub architecture: String,
    pub total_programs: usize,
    pub completed_programs: usize,
    pub total_checks: usize,
    pub passed_checks: usize,
    pub failed_checks: usize,
    pub programs: Vec<ZhvsProgramResult>,
}

fn assemble_obj(path: &Path, include_dirs: &[std::path::PathBuf]) -> Result<hlasm::Module> {
    let m = hlasm::compile_file(path, include_dirs)
        .map_err(|e| anyhow::anyhow!("assembling {}: {e:?}", path.display()))?;
    let obj = hlasm::object_bytes(&m)
        .map_err(|e| anyhow::anyhow!("emitting OBJ for {}: {e}", path.display()))?;
    hlasm::load_module(&obj).map_err(|e| anyhow::anyhow!("loading OBJ for {}: {e}", path.display()))
}

/// Assembles, links and runs the selected `zhvs` programs on Hercules in
/// `z/Arch` mode.
pub fn run_zhvs(
    suite_dir: &Path,
    herc: &Hercules,
    work_dir: &Path,
    filter: &[String],
) -> Result<ZhvsOracleRun> {
    fs::create_dir_all(work_dir)?;
    let copy_dir = suite_dir.join("copy");
    let hs_lib_dir = suite_dir.join("HS/lib");
    let include_dirs = vec![copy_dir];

    let zhvrpt_path = work_dir.join("ZHVRPT_BARE.ASM");
    fs::write(&zhvrpt_path, BARE_ZHVRPT_ASM)?;
    let zhvrpt_mod = assemble_obj(&zhvrpt_path, &[])?;
    let hss01a_mod = assemble_obj(&hs_lib_dir.join("HSS01A.ASM"), &[])?;

    let selected: Vec<(&str, &str)> = if filter.is_empty() {
        DEFAULT_PROGRAMS.to_vec()
    } else {
        let mut list = Vec::new();
        for p in filter {
            let p_up = p.to_ascii_uppercase();
            let m = &p_up[..2];
            list.push((
                Box::leak(m.to_string().into_boxed_str()) as &str,
                Box::leak(p_up.into_boxed_str()) as &str,
            ));
        }
        list
    };

    if selected.is_empty() {
        bail!("no ZHVS programs selected");
    }

    let total_mem = PROG_BASE + (selected.len() + 2) * PROG_SLOT;
    let dump_end = total_mem.max(REPORT_BASE + 0x40000) - 1;
    let mut core = vec![0u8; dump_end + 1];

    // ESA/390 IPL PSW -> 0x1000
    core[0..8].copy_from_slice(&[0x00, 0x08, 0x00, 0x00, 0x80, 0x00, 0x10, 0x00]);
    // ESA/390 Program New PSW -> 0x0F00
    core[0x68..0x70].copy_from_slice(&[0x00, 0x08, 0x00, 0x00, 0x80, 0x00, 0x0F, 0x00]);
    // z/Arch 128-bit Program New PSW at 0x1D0 -> 0x0F00
    core[0x1D0..0x1D8].copy_from_slice(&[0x00, 0x00, 0x00, 0x01, 0x80, 0x00, 0x00, 0x00]);
    core[0x1D8..0x1E0].copy_from_slice(&[0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x0F, 0x00]);

    // Program-check handler at 0x0F00:
    // Record PIC from 0x8E into 2(R9), advance R9 by 16, branch to driver loop (R8).
    core[0x0F00..0x0F10].copy_from_slice(&[
        0xD2, 0x01, 0x90, 0x02, 0x00, 0x8E, // MVC 2(2,9),142(0)
        0x41, 0x90, 0x90, 0x10, // LA 9,16(9)
        0x07, 0xF8, // BR 8
        0x00, 0x00, 0x00, 0x00,
    ]);

    // Driver at 0x1000:
    // Switch to z/Arch mode (SIGP 1,0,18 with R1=1), enable AFP (CTL0 bit 45),
    // and iterate through the descriptor table at 0x2000.
    let driver: [u8; 78] = [
        0x41, 0x10, 0x00, 0x01, // LA 1,1
        0x41, 0x30, 0x00, 0x00, // LA 3,0
        0xAE, 0x13, 0x00, 0x12, // SIGP 1,3,18 (set z/Arch mode)
        0xEB, 0x00, 0x02, 0x10, 0x00, 0x2F, // LCTLG 0,0,0x210(0)
        0x41, 0x90, 0x20, 0x00, // LA 9,0x2000
        0x0D, 0x80, // BASR 8,0 (loop target)
        0x58, 0x60, 0x00, 0x10, // L 6,0x10(0) -> 0x30004 (line count addr)
        0x58, 0x70, 0x60, 0x00, // L 7,0(6)
        0x50, 0x70, 0x90, 0x08, // ST 7,8(9) -> start_line
        0x58, 0xF0, 0x90, 0x00, // L 15,0(9) -> entry address
        0x12, 0xFF, // LTR 15,15
        0xA7, 0x84, 0x00, 0x10, // BRC 8,+32 -> LPSWE at +74
        0x41, 0xD0, 0x05, 0x00, // LA 13,0x500 (save area)
        0x0D, 0xEF, // BASR 14,15
        0x92, 0x01, 0x90, 0x04, // MVI 4(9),1 (completed)
        0x58, 0x60, 0x00, 0x10, // L 6,0x10(0)
        0x58, 0x70, 0x60, 0x00, // L 7,0(6)
        0x50, 0x70, 0x90, 0x0C, // ST 7,12(9) -> end_line
        0x41, 0x90, 0x90, 0x10, // LA 9,16(9)
        0x07, 0xF8, // BR 8
        0xB2, 0xB2, 0x02, 0x00, // LPSWE 0x200(0) (disabled wait)
    ];
    core[0x1000..0x1000 + driver.len()].copy_from_slice(&driver);
    // Pointer to line count at 0x0010
    core[0x0010..0x0014].copy_from_slice(&(LINE_CNT_ADDR as u32).to_be_bytes());
    // Disabled wait 128-bit PSW at 0x0200
    core[0x0200..0x0208].copy_from_slice(&[0x00, 0x02, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00]);
    core[0x0208..0x0210].copy_from_slice(&[0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0xDE, 0xAD]);
    // Control Register 0 at 0x0210 with AFP register control bit (bit 45 = 0x00040000 in low word)
    core[0x0210..0x0218].copy_from_slice(&[0x00, 0x00, 0x00, 0x00, 0x00, 0x04, 0x00, 0x00]);
    // Initial report buffer pointer at 0x30000
    core[BUF_PTR_ADDR..BUF_PTR_ADDR + 4].copy_from_slice(&(REPORT_BASE as u32).to_be_bytes());

    for (idx, &(m_name, p_name)) in selected.iter().enumerate() {
        let asm_path = suite_dir.join(format!("{m_name}/{p_name}.ASM"));
        let main_mod = assemble_obj(&asm_path, &include_dirs)?;
        let linked = if p_name == "HS101A" {
            hlasm::link_modules(&[main_mod, zhvrpt_mod.clone(), hss01a_mod.clone()])
        } else {
            hlasm::link_modules(&[main_mod, zhvrpt_mod.clone()])
        }
        .map_err(|e| anyhow::anyhow!("linking {p_name}: {e}"))?;

        let base = PROG_BASE + idx * PROG_SLOT;
        let bytes = linked
            .storage_at(base)
            .map_err(|e| anyhow::anyhow!("relocating {p_name}: {e}"))?;
        let entry = base as u32 + linked.entry;
        core[base..base + bytes.len()].copy_from_slice(&bytes);

        let desc = TABLE_ADDR + idx * 16;
        core[desc..desc + 4].copy_from_slice(&entry.to_be_bytes());
    }

    let (dump, version) = herc.run_with_arch(work_dir, &core, dump_end, "z/Arch")?;

    let mut programs = Vec::with_capacity(selected.len());
    let mut completed_programs = 0;
    let mut total_checks = 0;
    let mut passed_checks = 0;
    let mut failed_checks_total = 0;

    for (idx, &(m_name, p_name)) in selected.iter().enumerate() {
        let desc = TABLE_ADDR + idx * 16;
        let pic = u16::from_be_bytes([dump[desc + 2], dump[desc + 3]]);
        let completed = dump[desc + 4] == 1;
        let start_line = u32::from_be_bytes(dump[desc + 8..desc + 12].try_into().unwrap()) as usize;
        let end_line = if completed {
            u32::from_be_bytes(dump[desc + 12..desc + 16].try_into().unwrap()) as usize
        } else {
            start_line
        };

        let mut report_lines = Vec::new();
        let mut pass = 0;
        let mut fail = 0;
        let mut failed_checks = Vec::new();

        for l in start_line..end_line {
            let off = REPORT_BASE + l * 120;
            if off + 120 <= dump.len() {
                let text = ebcdic::decode(&dump[off..off + 120]).trim_end().to_string();
                if let Some(id) = text.strip_prefix("PASS") {
                    pass += 1;
                    let _ = id;
                } else if let Some(id) = text.strip_prefix("FAIL*") {
                    fail += 1;
                    failed_checks.push(id.trim().to_string());
                }
                report_lines.push(text);
            }
        }

        if completed {
            completed_programs += 1;
        }
        total_checks += pass + fail;
        passed_checks += pass;
        failed_checks_total += fail;

        programs.push(ZhvsProgramResult {
            module: m_name.to_string(),
            program: p_name.to_string(),
            completed,
            pic,
            pass,
            fail,
            failed_checks,
            report_lines,
        });
    }

    // Print per-module summary
    let mut by_mod: BTreeMap<&str, (usize, usize, usize, usize)> = BTreeMap::new();
    for p in &programs {
        let e = by_mod.entry(p.module.as_str()).or_default();
        e.0 += 1;
        if p.completed {
            e.1 += 1;
        }
        e.2 += p.pass;
        e.3 += p.fail;
    }
    for (m, (tot_p, ok_p, pass_c, fail_c)) in &by_mod {
        println!(
            "ZHVS {m:<4} programs={ok_p}/{tot_p} checks={pass_c}/{} (fail={fail_c})",
            pass_c + fail_c
        );
    }

    Ok(ZhvsOracleRun {
        oracle: version,
        architecture: "z/Arch".to_string(),
        total_programs: programs.len(),
        completed_programs,
        total_checks,
        passed_checks,
        failed_checks: failed_checks_total,
        programs,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn assembles_all_default_zhvs_programs() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
        let suite = root.join("zhvs");
        let dir = tempfile::tempdir().unwrap();
        let copy_dir = suite.join("copy");
        let hs_lib_dir = suite.join("HS/lib");
        let include_dirs = vec![copy_dir];

        let zhvrpt_path = dir.path().join("ZHVRPT_BARE.ASM");
        fs::write(&zhvrpt_path, BARE_ZHVRPT_ASM).unwrap();
        let zhvrpt_mod = assemble_obj(&zhvrpt_path, &[]).unwrap();
        let hss01a_mod = assemble_obj(&hs_lib_dir.join("HSS01A.ASM"), &[]).unwrap();

        for &(m, p) in DEFAULT_PROGRAMS {
            let asm_path = suite.join(format!("{m}/{p}.ASM"));
            let main_mod = assemble_obj(&asm_path, &include_dirs).unwrap();
            let linked = if p == "HS101A" {
                hlasm::link_modules(&[main_mod, zhvrpt_mod.clone(), hss01a_mod.clone()])
            } else {
                hlasm::link_modules(&[main_mod, zhvrpt_mod.clone()])
            }
            .unwrap();
            let bytes = linked.storage_at(0x10000).unwrap();
            assert!(!bytes.is_empty(), "{p}");
        }
    }
}
