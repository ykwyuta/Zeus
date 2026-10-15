use super::super::assemble::{Statement, operands};
use std::collections::{HashMap, HashSet};

/// Listing controls are kept separately from USING, so independent PUSH/POP
/// operations restore exactly the classes requested by the source.
pub(super) struct Controls {
    print: HashMap<String, bool>,
    print_stack: Vec<HashMap<String, bool>>,
    pcontrol: std::collections::BTreeMap<String, bool>,
    pub optable: String,
    pub afpr: bool,
    pub macrocase: bool,
    acontrol_stack: Vec<(String, bool, bool)>,
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
                ("UHEAD".into(), true),
            ]),
            print_stack: Vec::new(),
            pcontrol: Default::default(),
            optable: "ZEUS".into(),
            afpr: true,
            macrocase: false,
            acontrol_stack: Vec::new(),
        }
    }
}

impl Controls {
    pub fn set_optable(&mut self, name: &str) -> Result<(), String> {
        self.optable = super::super::options::parse_optable(name)?;
        Ok(())
    }
    pub fn set_pcontrol(
        &mut self,
        options: &std::collections::BTreeMap<String, bool>,
    ) -> Result<(), String> {
        for key in options.keys() {
            if !matches!(
                key.as_str(),
                "ON" | "GEN" | "DATA" | "MCALL" | "MSOURCE" | "UHEAD"
            ) {
                return Err(format!("invalid PCONTROL key {key}"));
            }
        }
        self.pcontrol = options.clone();
        Ok(())
    }
    pub fn print_snapshot(&self) -> std::collections::BTreeMap<String, bool> {
        self.print
            .iter()
            .chain(self.pcontrol.iter())
            .map(|(key, value)| (key.clone(), *value))
            .collect()
    }
    pub fn apply(&mut self, statement: &Statement) -> Result<(), String> {
        let text = statement.args.to_ascii_uppercase();
        let args = operands(&text);
        match statement.op.as_str() {
            "PRINT" => {
                if args.iter().any(|a| a == "NOPRINT")
                    && !args.iter().any(|a| !a.is_empty() && a != "NOPRINT")
                {
                    return Err("PRINT NOPRINT requires at least one other operand".into());
                }
                for arg in args {
                    if matches!(arg.as_str(), "" | "NOPRINT") {
                        continue;
                    }
                    let (name, enabled) = super::super::options::print_option(&arg)
                        .ok_or_else(|| format!("unsupported PRINT option {arg}"))?;
                    self.print.insert(name.into(), enabled);
                }
            }
            "ACONTROL" => {
                for arg in args {
                    if arg == "NOPRINT" {
                        continue;
                    }
                    if arg == "AFPR" {
                        self.afpr = true;
                    } else if arg == "NOAFPR" {
                        self.afpr = false;
                    } else if let Some(compat) = arg
                        .strip_prefix("COMPAT(")
                        .or_else(|| arg.strip_prefix("CPAT("))
                        .and_then(|s| s.strip_suffix(')'))
                    {
                        for option in operands(compat) {
                            match option.as_str() {
                                "MACROCASE" | "MC" => self.macrocase = true,
                                "NOMACROCASE" | "NOMC" => self.macrocase = false,
                                _ => {
                                    return Err(format!(
                                        "unsupported ACONTROL COMPAT option {option}"
                                    ));
                                }
                            }
                        }
                    } else if let Some(table) = arg
                        .strip_prefix("OPTABLE(")
                        .and_then(|v| v.strip_suffix(')'))
                    {
                        self.set_optable(table)?;
                    } else {
                        return Err(format!(
                            "unsupported ACONTROL option {arg}; supported instruction tables are UNI and ZEUS"
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
                        "ACONTROL" if statement.op == "PUSH" => self.acontrol_stack.push((
                            self.optable.clone(),
                            self.afpr,
                            self.macrocase,
                        )),
                        "ACONTROL" => {
                            (self.optable, self.afpr, self.macrocase) = self
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
