use super::super::assemble::{Statement, operands};
use std::collections::{HashMap, HashSet};

/// Listing controls are kept separately from USING, so independent PUSH/POP
/// operations restore exactly the classes requested by the source.
pub(super) struct Controls {
    print: HashMap<String, bool>,
    print_stack: Vec<HashMap<String, bool>>,
    pub optable: String,
    acontrol_stack: Vec<String>,
}

impl Default for Controls {
    fn default() -> Self {
        Self {
            print: HashMap::from([
                ("ON".into(), true),
                ("GEN".into(), true),
                ("DATA".into(), false),
                ("MCALL".into(), false),
                ("MSOURCE".into(), true),
                ("UHEAD".into(), false),
            ]),
            print_stack: Vec::new(),
            optable: "ZEUS".into(),
            acontrol_stack: Vec::new(),
        }
    }
}

impl Controls {
    pub fn apply(&mut self, statement: &Statement) -> Result<(), String> {
        let text = statement.args.to_ascii_uppercase();
        let args = operands(&text);
        match statement.op.as_str() {
            "PRINT" => {
                for arg in args {
                    let (name, enabled) = match arg.as_str() {
                        "" | "NOPRINT" => continue,
                        "ON" => ("ON", true),
                        "OFF" => ("ON", false),
                        "GEN" | "DATA" | "MCALL" | "MSOURCE" | "UHEAD" => (arg.as_str(), true),
                        "NOGEN" | "NODATA" | "NOMCALL" | "NOMSOURCE" | "NOUHEAD" => {
                            (&arg[2..], false)
                        }
                        _ => return Err(format!("unsupported PRINT option {arg}")),
                    };
                    self.print.insert(name.into(), enabled);
                }
            }
            "ACONTROL" => {
                for arg in args {
                    if arg == "NOPRINT" {
                        continue;
                    }
                    if arg == "OPTABLE(ZEUS)" {
                        self.optable = "ZEUS".into();
                    } else {
                        return Err(format!(
                            "unsupported ACONTROL option {arg}; the source instruction table is ZEUS"
                        ));
                    }
                }
            }
            "PUSH" | "POP" => {
                let mut seen = HashSet::new();
                let mut classes = 0;
                for arg in args {
                    if !seen.insert(arg.clone()) {
                        return Err(format!("duplicate {} option {arg}", statement.op));
                    }
                    if arg == "NOPRINT" {
                        continue;
                    }
                    classes += 1;
                    match arg.as_str() {
                        "USING" => {} // The assembler manages addressability snapshots.
                        "PRINT" if statement.op == "PUSH" => {
                            self.print_stack.push(self.print.clone())
                        }
                        "PRINT" => {
                            self.print = self
                                .print_stack
                                .pop()
                                .ok_or("POP PRINT without PUSH PRINT")?
                        }
                        "ACONTROL" if statement.op == "PUSH" => {
                            self.acontrol_stack.push(self.optable.clone())
                        }
                        "ACONTROL" => {
                            self.optable = self
                                .acontrol_stack
                                .pop()
                                .ok_or("POP ACONTROL without PUSH ACONTROL")?
                        }
                        _ => {
                            return Err(format!("unsupported {} status class {arg}", statement.op));
                        }
                    }
                }
                if classes == 0 {
                    return Err("PUSH/POP requires at least one status class".into());
                }
            }
            _ => {}
        }
        Ok(())
    }
}
