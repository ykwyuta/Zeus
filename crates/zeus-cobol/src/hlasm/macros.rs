use super::assemble::{Statement, directive, instruction_size, member, operands};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::{Path, PathBuf};
mod controls;
mod values;
use values::{character, number, substitute};

pub(super) fn expand(
    input: Vec<Statement>,
    dirs: &[PathBuf],
    diagnostics: &mut Vec<super::Diagnostic>,
    options: &super::AssemblyOptions,
    calls: &mut Vec<super::listing::MacroCall>,
    sources: &mut Vec<super::listing::SourceFile>,
    copies: &mut Vec<super::listing::CopyCall>,
) -> Result<Vec<Statement>, String> {
    let mut state = Expansion {
        options: options.clone(),
        ..Expansion::default()
    };
    state.controls.set_pcontrol(&options.pcontrol)?;
    state.controls.macrocase = options.macrocase.unwrap_or(false);
    state
        .controls
        .set_optable(options.optable.as_deref().unwrap_or("ZEUS"))?;
    state.initialize_context(input.first().map(|s| Path::new(&s.file)));
    let result = expand_scope(
        input,
        dirs,
        &mut HashMap::new(),
        0,
        &mut Scope::default(),
        &mut state,
    );
    diagnostics.extend(state.diagnostics);
    calls.extend(state.calls);
    copies.extend(state.copy_calls);
    sources.extend(state.source_files);
    result
}

fn condition(text: &str) -> Result<bool, String> {
    let mut quoted = false;
    let text: String = text
        .chars()
        .map(|c| {
            if c == '\'' {
                quoted = !quoted;
            }
            if quoted { c } else { c.to_ascii_uppercase() }
        })
        .collect();
    condition_at(&text, 0)
}

fn condition_at(text: &str, level: usize) -> Result<bool, String> {
    if level > 64 {
        return Err("logical expression nesting exceeds 64".into());
    }
    let text = text.trim();
    // Strip only parentheses enclosing the complete expression.
    if text.starts_with('(') && text.ends_with(')') {
        let (mut depth, mut quote, mut whole) = (0, false, true);
        for (i, c) in text.char_indices() {
            if c == '\'' {
                quote = !quote;
            }
            if !quote {
                if c == '(' {
                    depth += 1;
                }
                if c == ')' {
                    depth -= 1;
                }
                if depth == 0 && i + 1 < text.len() {
                    whole = false;
                }
            }
        }
        if whole {
            return condition_at(&text[1..text.len() - 1], level + 1);
        }
    }
    for wanted in [" XOR ", " OR ", " AND "] {
        let (mut depth, mut quoted) = (0, false);
        for (i, c) in text.char_indices() {
            if c == '\'' {
                quoted = !quoted;
            }
            if quoted {
                continue;
            }
            if c == '(' {
                depth += 1;
            }
            if c == ')' {
                depth -= 1;
            }
            if depth == 0 && text[i..].starts_with(wanted) {
                let a = condition_at(&text[..i], level + 1)?;
                let b = condition_at(&text[i + wanted.len()..], level + 1)?;
                return Ok(match wanted {
                    " XOR " => a ^ b,
                    " OR " => a | b,
                    _ => a & b,
                });
            }
        }
    }
    let (mut depth, mut quote) = (0, false);
    for (i, c) in text.char_indices() {
        if c == '\'' {
            quote = !quote;
        }
        if quote {
            continue;
        }
        if c == '(' {
            depth += 1;
        }
        if c == ')' {
            depth -= 1;
        }
        if depth != 0 {
            continue;
        }
        for op in [" EQ ", " NE ", " LT ", " LE ", " GT ", " GE "] {
            if text[i..].starts_with(op) {
                let (a, b) = (&text[..i], &text[i + op.len()..]);
                let order = match (character(a), character(b)) {
                    (Ok(a), Ok(b)) => {
                        let (a, b) = (crate::ebcdic::encode(&a), crate::ebcdic::encode(&b));
                        a.len().cmp(&b.len()).then_with(|| a.cmp(&b))
                    }
                    (Err(_), Err(_)) => number(a)?.cmp(&number(b)?),
                    _ => return Err("incompatible character and arithmetic comparands".into()),
                };
                return Ok(match op {
                    " EQ " => order.is_eq(),
                    " NE " => !order.is_eq(),
                    " LT " => order.is_lt(),
                    " LE " => !order.is_gt(),
                    " GT " => order.is_gt(),
                    _ => !order.is_lt(),
                });
            }
        }
    }
    if let Some(t) = text.strip_prefix("NOT ") {
        return Ok(!condition_at(t, level + 1)?);
    }
    Ok(number(text)? != 0)
}

fn expand_scope(
    input: Vec<Statement>,
    dirs: &[PathBuf],
    macros: &mut HashMap<String, Macro>,
    depth: usize,
    vars: &mut Scope,
    state: &mut Expansion,
) -> Result<Vec<Statement>, String> {
    if depth > 32 {
        return Err("COPY/macro nesting exceeds 32".into());
    }
    let open_code = vars.macro_names.is_empty();
    if open_code {
        state.forward.push((input.clone(), 0));
    }
    let mut sequences = HashMap::new();
    let mut sequence_generation = usize::MAX;
    let mut visited_sequences = HashMap::new();
    let mut out = Vec::new();
    let (mut pc, mut steps) = (0, 0);
    while let Some(original) = input.get(pc) {
        if state.ended {
            break;
        }
        if original.label.starts_with('.')
            && let Some(previous) = visited_sequences.insert(original.label.clone(), pc)
            && previous != pc
        {
            return Err(original.error("duplicate sequence symbol"));
        }
        steps += 1;
        state.steps += 1;
        if steps > 100_000 || state.steps > 100_000 || out.len() > 100_000 {
            return Err(original.error("conditional assembly limit exceeded"));
        }
        pc += 1;
        if open_code {
            state.forward.last_mut().unwrap().1 = pc;
        }
        let mut s = original.clone();
        s.op = substitute(&s.op, vars, state)
            .map_err(|e| s.error(e))?
            .to_ascii_uppercase();
        let mut builtin_alias = false;
        if let Some(replacement) = state.aliases.get(&s.op) {
            match replacement {
                Some(op) => {
                    s.op = op.clone();
                    builtin_alias = true;
                }
                None => {
                    state.diagnostics.push(super::Diagnostic::at(
                        &s,
                        12,
                        "UNRESOLVED_OPERATION",
                        format!("deleted operation {}", s.op),
                    ));
                    continue;
                }
            }
        }
        let macro_invocation = !builtin_alias && macros.contains_key(&s.op);
        if open_code
            && (macro_invocation
                || matches!(s.op.as_str(), "SETA" | "SETB" | "SETC" | "AIF" | "AGO"))
        {
            state.open_variables = Some(vars.clone());
        }
        if s.op == "OPSYN" && !macro_invocation {
            state.operation_generation += 1;
            let name = substitute(&s.label, vars, state)
                .map_err(|e| s.error(e))?
                .to_ascii_uppercase();
            let target = substitute(&s.args, vars, state)
                .map_err(|e| s.error(e))?
                .to_ascii_uppercase();
            if !super::assemble::symbol_name(&name) {
                return Err(s.error("OPSYN requires an operation name"));
            }
            if target.is_empty() {
                macros.remove(&name);
                state.macro_types.remove(&name);
                state.aliases.insert(name, None);
            } else {
                let resolved = state
                    .aliases
                    .get(&target)
                    .cloned()
                    .unwrap_or(Some(target.clone()))
                    .ok_or_else(|| s.error("OPSYN target has been deleted"))?;
                if let Some(m) = macros
                    .get(&resolved)
                    .filter(|_| !state.aliases.contains_key(&target))
                    .cloned()
                {
                    macros.insert(name.clone(), m);
                    state.aliases.remove(&name);
                    state.macro_types.insert(name);
                } else if super::is_operation(&resolved) {
                    macros.remove(&name);
                    state.macro_types.remove(&name);
                    state.aliases.insert(name, Some(resolved));
                } else {
                    return Err(s.error(format!("undefined OPSYN target {target}")));
                }
            }
            continue;
        }
        state.source_file = PathBuf::from(&s.file);
        state.library_dirs = dirs.to_vec();
        state.current_literals.clear();
        if s.op == "MACRO" && !macro_invocation {
            state.operation_generation += 1;
            let p = input
                .get(pc)
                .ok_or_else(|| s.error("missing macro prototype"))?;
            let parameters = if p.args.is_empty() {
                vec![]
            } else {
                operands(&p.args)
            };
            let mut names = HashSet::new();
            for name in std::iter::once(p.label.as_str())
                .filter(|s| !s.is_empty())
                .chain(
                    parameters
                        .iter()
                        .map(|p| p.split_once('=').map_or(p.as_str(), |(n, _)| n)),
                )
            {
                if !name.starts_with('&')
                    || name.len() > 64
                    || name.len() < 2
                    || name.to_ascii_uppercase().starts_with("&SYS")
                    || !name.as_bytes()[1..]
                        .iter()
                        .all(|c| c.is_ascii_alphanumeric() || b"_@$#".contains(c))
                    || name.as_bytes()[1].is_ascii_digit()
                {
                    return Err(p.error("invalid or reserved macro parameter name"));
                }
                if !names.insert(name.to_ascii_uppercase()) {
                    return Err(p.error("duplicate macro parameter name"));
                }
            }
            pc += 1;
            let mut body = Vec::new();
            let mut nested = 0;
            loop {
                let line = input.get(pc).ok_or_else(|| s.error("missing MEND"))?;
                pc += 1;
                let operation = state.builtin_operation(&line.op);
                if operation == "MEND" {
                    if nested == 0 {
                        break;
                    }
                    nested -= 1;
                }
                if operation == "MACRO" {
                    nested += 1;
                }
                body.push(line.clone());
            }
            macros.insert(
                p.op.clone(),
                Macro {
                    label: p.label.clone(),
                    parameters,
                    body,
                    definition_file: p.file.clone(),
                    definition_location: p
                        .listing
                        .as_ref()
                        .and_then(|a| a.definition.clone())
                        .unwrap_or_else(|| super::listing::SourceLocation {
                            statement: 0,
                            file: p.file.clone(),
                            line: p.line,
                        }),
                },
            );
            state.aliases.remove(&p.op);
            state.macro_types.insert(p.op.clone());
            continue;
        }
        s.op = substitute(&s.op, vars, state)
            .map_err(|e| s.error(e))?
            .to_ascii_uppercase();
        if !macro_invocation
            && !matches!(
                s.op.as_str(),
                "LCLA" | "LCLB" | "LCLC" | "GBLA" | "GBLB" | "GBLC" | "SETA" | "SETB" | "SETC"
            )
        {
            s.args = if matches!(s.op.as_str(), "AIF" | "AGO" | "ACTR") {
                values::macro_expression(&s.args, vars, state)
            } else {
                substitute(&s.args, vars, state)
            }
            .map_err(|e| s.error(e))?;
        }
        match if macro_invocation { "" } else { s.op.as_str() } {
            "LCLA" | "LCLB" | "LCLC" | "GBLA" | "GBLB" | "GBLC" => {
                vars.declare(&original.args, &s.op, state)
                    .map_err(|e| s.error(e))?;
                continue;
            }
            "SETA" | "SETB" | "SETC" => {
                vars.assign(&original.label, &s.op, &original.args, state)
                    .map_err(|e| s.error(e))?;
                continue;
            }
            "AGO" | "AIF" => {
                let target = if s.op == "AGO" {
                    if s.args.starts_with('(') {
                        let mut at = 0;
                        let index =
                            number(&values::group(&s.args, &mut at).map_err(|e| s.error(e))?)
                                .map_err(|e| s.error(e))?;
                        let targets = operands(&s.args[at..]);
                        usize::try_from(index)
                            .ok()
                            .filter(|i| *i > 0)
                            .and_then(|i| targets.get(i - 1))
                            .cloned()
                    } else {
                        Some(s.args.clone())
                    }
                } else {
                    let mut target = None;
                    for clause in operands(&s.args) {
                        let mut at = 0;
                        let test = values::group(&clause, &mut at).map_err(|e| s.error(e))?;
                        if condition(&test).map_err(|e| s.error(e))? {
                            target = Some(clause[at..].trim().to_string());
                            break;
                        }
                    }
                    target
                };
                if let Some(target) = target {
                    if let Some(budget) = &mut vars.branch_budget {
                        if *budget == 0 {
                            return Err(s.error("ACTR conditional branch count exhausted"));
                        }
                        *budget -= 1;
                    }
                    if sequence_generation != state.operation_generation {
                        sequences.clear();
                        let mut definition_depth = 0;
                        for (index, line) in input.iter().enumerate() {
                            let op = state.builtin_operation(&line.op);
                            if op == "MACRO" {
                                definition_depth += 1;
                            }
                            if definition_depth == 0 {
                                if op == "END" {
                                    break;
                                }
                                if line.label.starts_with('.')
                                    && sequences.insert(line.label.clone(), index).is_some()
                                {
                                    return Err(line.error("duplicate sequence symbol"));
                                }
                            }
                            if op == "MEND" {
                                definition_depth -= 1;
                            }
                        }
                        sequence_generation = state.operation_generation;
                    }
                    pc = *sequences
                        .get(&target.to_ascii_uppercase())
                        .ok_or_else(|| s.error(format!("undefined sequence symbol {target}")))?;
                }
                continue;
            }
            "ACTR" => {
                let n = number(&s.args).map_err(|e| s.error(e))?;
                if !(0..=i64::from(i32::MAX)).contains(&n) {
                    return Err(s.error("ACTR requires a nonnegative signed 32-bit count"));
                }
                vars.branch_budget = Some(n);
                continue;
            }
            "ANOP" => continue,
            "MEXIT" => {
                if vars.macro_names.is_empty() {
                    return Err(s.error("MEXIT outside a macro expansion"));
                }
                break;
            }
            "MNOTE" => {
                let args = operands(&s.args);
                let (severity, text) = match args.as_slice() {
                    [text] => (0, text),
                    [severity, text] => {
                        let value = if severity == "*" {
                            0
                        } else if severity.is_empty() {
                            1
                        } else {
                            number(severity).map_err(|e| s.error(e))?
                        };
                        (
                            u8::try_from(value)
                                .map_err(|_| s.error("MNOTE severity must be in 0..255"))?,
                            text,
                        )
                    }
                    _ => return Err(s.error("MNOTE [severity,]message required")),
                };
                if !text.starts_with('\'') || !text.ends_with('\'') || s.args.len() > 1024 {
                    return Err(s.error("invalid MNOTE message or length"));
                }
                let message = character(text).map_err(|e| s.error(e))?;
                state
                    .diagnostics
                    .push(super::Diagnostic::at(&s, severity, "MNOTE", message));
                vars.highest_severity = vars.highest_severity.max(severity);
                continue;
            }
            _ => {}
        }
        s.label = if s.label.starts_with('.') {
            String::new()
        } else {
            substitute(&s.label, vars, state)
                .map_err(|e| s.error(e))?
                .to_ascii_uppercase()
        };
        if s.op == "COPY" && !macro_invocation {
            let file = member(&s.args, Path::new(&s.file), dirs).map_err(|e| s.error(e))?;
            let name = s.args.trim_matches('\'').to_ascii_uppercase();
            if state.options.collect_listing {
                state.copy_calls.push(super::listing::CopyCall {
                    location: super::listing::SourceLocation {
                        statement: state.generated + 1,
                        file: s.file.clone(),
                        line: s.line,
                    },
                    member: name.clone(),
                    definition_file: file.to_string_lossy().into_owned(),
                    parent: state
                        .member_stack
                        .last()
                        .cloned()
                        .unwrap_or_else(|| "PRIMARY INPUT".into()),
                });
            }
            let input = super::source::statements(
                &file,
                &state.options,
                true,
                &mut state.diagnostics,
                Some(&mut state.source_files),
            )?;
            state.member_stack.push(name);
            let expanded = expand_scope(input, dirs, macros, depth + 1, vars, state);
            state.member_stack.pop();
            out.extend(expanded?);
        } else if !macro_invocation && (instruction_size(&s.op).is_some() || directive(&s.op)) {
            if s.op == "ADATA" {
                if !s.label.is_empty() && !s.label.starts_with('.') {
                    return Err(s.error("ADATA permits only a sequence-symbol label"));
                }
                let data = super::adata::parse(&s.args).map_err(|e| s.error(e))?;
                let ccsid = match state.options.source_encoding {
                    super::SourceEncoding::Utf8 => 37,
                    super::SourceEncoding::Ebcdic { ccsid, .. } => ccsid,
                };
                data.record(ccsid).map_err(|e| s.error(e))?;
            }
            state.controls.apply(&s).map_err(|e| s.error(e))?;
            s.afpr = state.controls.afpr;
            if state.options.collect_listing {
                s.listing = Some(super::listing::Attributes {
                    print: state.controls.print_snapshot(),
                    macro_depth: vars.macro_names.len(),
                    calls: vec![],
                    definition: s.listing.as_ref().and_then(|a| a.definition.clone()),
                    model: (open_code
                        && [&original.label, &original.op, &original.args]
                            .iter()
                            .any(|text| has_variable_symbol(text)))
                    .then(|| super::listing::ModelStatement {
                        label: original.label.clone(),
                        operation: original.op.clone(),
                        operands: original.args.clone(),
                    }),
                });
            }
            state.observe_attributes(&s);
            if s.op == "END" {
                state.ended = true;
            }
            out.push(s);
            state.generated += 1;
        } else {
            if open_code {
                state.open_variables = Some(vars.clone());
            }
            // Argument boundaries come from the invocation, before SETC
            // substitution can introduce commas or keyword-like text.
            let invocation = if original.args.is_empty() {
                vec![]
            } else {
                operands(&original.args)
                    .iter()
                    .map(|a| {
                        let pair = a.split_once('=').filter(|(k, _)| {
                            !k.is_empty()
                                && k.bytes()
                                    .all(|c| c.is_ascii_alphanumeric() || b"_@$#".contains(&c))
                                && !k.as_bytes()[0].is_ascii_digit()
                        });
                        let (key, value) = if let Some((k, v)) = pair {
                            (Some(format!("&{}", k.to_ascii_uppercase())), v)
                        } else {
                            (None, a.as_str())
                        };
                        let value = substitute(value, vars, state).map_err(|e| s.error(e))?;
                        let value = if state.controls.macrocase
                            && !(value.starts_with('\'') && value.ends_with('\''))
                        {
                            value.to_ascii_uppercase()
                        } else {
                            value
                        };
                        Ok((key, value))
                    })
                    .collect::<Result<Vec<_>, String>>()?
            };
            if !macros.contains_key(&s.op) {
                let file = match member(&s.op, Path::new(&s.file), dirs) {
                    Ok(file) => file,
                    Err(_) => {
                        state.diagnostics.push(super::Diagnostic::at(
                            &s,
                            12,
                            "UNRESOLVED_OPERATION",
                            format!(
                                "unsupported instruction or macro {}; macro member not found",
                                s.op
                            ),
                        ));
                        continue;
                    }
                };
                if !expand_scope(
                    super::source::statements(
                        &file,
                        &state.options,
                        true,
                        &mut state.diagnostics,
                        Some(&mut state.source_files),
                    )?,
                    dirs,
                    macros,
                    depth + 1,
                    &mut Scope::default(),
                    state,
                )?
                .is_empty()
                {
                    return Err(s.error("macro library has statements outside MACRO"));
                }
            }
            let m = macros
                .get(&s.op)
                .ok_or_else(|| s.error("macro was not defined"))?
                .clone();
            let mut local = Scope {
                label: s.label.clone(),
                macro_names: vars.macro_names.clone(),
                ..Scope::default()
            };
            local.macro_names.insert(0, s.op.clone());
            if !m.label.is_empty() {
                local.values.insert(m.label, s.label.clone());
            }
            let mut positional = Vec::new();
            let mut keywords = HashMap::new();
            for (key, value) in invocation {
                if let Some(key) = key {
                    if keywords.insert(key, value).is_some() {
                        return Err(s.error("duplicate keyword argument"));
                    }
                } else {
                    if !keywords.is_empty() {
                        return Err(s.error("positional argument follows keyword argument"));
                    }
                    positional.push(value);
                }
            }
            local.positional = positional.clone();
            let mut pos = positional.into_iter();
            for param in m.parameters {
                let (key, value) = if let Some((k, default)) = param.split_once('=') {
                    let key = k.to_ascii_uppercase();
                    let value = keywords.remove(&key).unwrap_or_else(|| default.into());
                    (key, value)
                } else {
                    (param.to_ascii_uppercase(), pos.next().unwrap_or_default())
                };
                local.values.insert(key, value);
            }
            if !keywords.is_empty() {
                return Err(s.error("unknown or extra macro argument"));
            }
            state.serial += 1;
            state.local_context(&mut local, &s, &m.definition_file);
            local
                .values
                .insert("&SYSNDX".into(), format!("{:04}", state.serial));
            let mut call = state
                .options
                .collect_listing
                .then(|| super::listing::MacroCall {
                    parent: state
                        .member_stack
                        .last()
                        .cloned()
                        .unwrap_or_else(|| "PRIMARY INPUT".into()),
                    statement: state.generated + 1,
                    generated: 0,
                    print: state.controls.print_snapshot(),
                    file: s.file.clone(),
                    line: s.line,
                    label: s.label.clone(),
                    operation: s.op.clone(),
                    operands: s.args.clone(),
                    depth: vars.macro_names.len(),
                    definition_file: m.definition_location.file.clone(),
                    definition_line: m.definition_location.line,
                });
            let call_index = state.calls.len();
            if let Some(call) = &call {
                state.calls.push(call.clone());
            }
            let mut body = m.body;
            for line in &mut body {
                if state.options.collect_listing {
                    let definition = super::listing::SourceLocation {
                        statement: 0,
                        file: line.file.clone(),
                        line: line.line,
                    };
                    line.listing
                        .get_or_insert_with(Default::default)
                        .definition
                        .get_or_insert(definition);
                }
                line.file = s.file.clone();
                line.line = s.line;
                line.sequence = s.sequence.clone();
            }
            state.member_stack.push(s.op.clone());
            let expanded = expand_scope(body, dirs, macros, depth + 1, &mut local, state);
            state.member_stack.pop();
            let mut body = expanded?;
            if let Some(call) = &mut call {
                call.generated = body.len();
                state.calls[call_index].generated = body.len();
            }
            vars.last_macro_severity = local.highest_severity;
            vars.highest_severity = vars.highest_severity.max(local.highest_severity);
            if let (Some(first), Some(call)) = (body.first_mut(), call) {
                first
                    .listing
                    .get_or_insert_with(Default::default)
                    .calls
                    .insert(0, call);
            }
            if !s.label.is_empty()
                && !body.iter().any(|b| b.label == s.label)
                && let Some(first) = body.first_mut()
            {
                if !first.label.is_empty() {
                    return Err(s.error("macro label conflicts with expansion"));
                }
                first.label = s.label;
                state.observe_attributes(first);
            }
            out.extend(body);
        }
    }
    if open_code {
        state.forward.pop();
    }
    Ok(out)
}

#[derive(Clone)]
struct Macro {
    label: String,
    parameters: Vec<String>,
    body: Vec<Statement>,
    definition_file: String,
    definition_location: super::listing::SourceLocation,
}

#[derive(Default, Clone)]
struct Scope {
    highest_severity: u8,
    last_macro_severity: u8,
    values: HashMap<String, String>,
    globals: HashSet<String>,
    sets: HashMap<String, SetSymbol>,
    positional: Vec<String>,
    label: String,
    macro_names: Vec<String>,
    branch_budget: Option<i64>,
}

#[derive(Default)]
struct Expansion {
    operation_generation: usize,
    open_variables: Option<Scope>,
    forward: Vec<(Vec<Statement>, usize)>,
    controls: controls::Controls,
    options: super::AssemblyOptions,
    system_values: HashMap<String, String>,
    current_section: String,
    current_location: String,
    current_section_type: String,
    section_locations: HashMap<String, String>,
    location_sections: HashMap<String, (String, String)>,
    input_file: String,
    aliases: HashMap<String, Option<String>>,
    macro_types: HashSet<String>,
    source_file: PathBuf,
    library_dirs: Vec<PathBuf>,
    diagnostics: Vec<super::Diagnostic>,
    calls: Vec<super::listing::MacroCall>,
    copy_calls: Vec<super::listing::CopyCall>,
    member_stack: Vec<String>,
    source_files: Vec<super::listing::SourceFile>,
    globals: HashMap<String, SetSymbol>,
    serial: usize,
    steps: usize,
    generated: usize,
    ended: bool,
    attribute_values: super::assemble::Symbols,
    attributes: HashMap<String, Result<super::expression::EquAttributes, String>>,
    storage_types: HashMap<String, String>,
    storage_programs: HashMap<String, Result<Option<u32>, String>>,
    defined_symbols: HashSet<String>,
    ordinary_attributes: HashMap<String, (Result<u32, String>, u8)>,
    defined_literals: HashSet<String>,
    current_literals: HashSet<String>,
    numeric_attributes: HashMap<String, Result<(i64, i64), String>>,
    pending_attribute_values: Vec<(String, String, super::expression::EquAttributes)>,
}

impl Expansion {
    fn builtin_operation<'a>(&'a self, name: &'a str) -> &'a str {
        match self.aliases.get(name) {
            Some(Some(op)) => op,
            Some(None) => "",
            None if self.macro_types.contains(name) => "",
            None => name,
        }
    }
    fn lookahead(&self, name: &str) -> Result<Option<Statement>, String> {
        fn scan(
            input: &[Statement],
            name: &str,
            state: &Expansion,
            depth: usize,
        ) -> Result<Option<Statement>, String> {
            if depth > 32 {
                return Err("COPY lookahead nesting exceeds 32".into());
            }
            let mut definition_depth = 0;
            for s in input {
                let op = state.builtin_operation(&s.op);
                if op == "MACRO" {
                    definition_depth += 1;
                }
                if definition_depth > 0 {
                    if op == "MEND" {
                        definition_depth -= 1;
                    }
                    continue;
                }
                if op == "END" {
                    break;
                }
                if (s.label == name && !matches!(op, "TITLE" | "AMODE" | "RMODE" | "XATTR"))
                    || (matches!(op, "EXTRN" | "WXTRN")
                        && operands(&s.args)
                            .iter()
                            .any(|symbol| symbol.eq_ignore_ascii_case(name)))
                {
                    let mut found = s.clone();
                    if !op.is_empty() {
                        found.op = op.into();
                    }
                    if found.op == "CXD" {
                        found.op = "DS".into();
                        found.args = "A".into();
                    }
                    if found.op == "DCB" {
                        found.op = "DS".into();
                        found.args = "CL96".into();
                    }
                    return Ok(Some(found));
                }
                if op == "COPY" {
                    let member_name = if s.args.contains('&') {
                        substitute(
                            &s.args,
                            state
                                .open_variables
                                .as_ref()
                                .ok_or("open-code context unavailable for COPY lookahead")?,
                            state,
                        )?
                    } else {
                        s.args.clone()
                    };
                    let path = member(&member_name, Path::new(&s.file), &state.library_dirs)
                        .map_err(|e| s.error(e))?;
                    if let Some(found) = scan(
                        &super::source::statements(&path, &state.options, true, &mut vec![], None)?,
                        name,
                        state,
                        depth + 1,
                    )? {
                        return Ok(Some(found));
                    }
                }
            }
            Ok(None)
        }
        for (input, start) in self.forward.iter().rev() {
            if let Some(s) = scan(&input[*start..], name, self, 0)? {
                return Ok(Some(s));
            }
        }
        Ok(None)
    }
    fn initialize_context(&mut self, input: Option<&Path>) {
        self.input_file = input.map_or_else(String::new, |p| p.display().to_string());
        let (date, short_date, time, _) = super::context::clock(self.options.started);
        for (name, value) in [
            ("&SYSASM", "ZEUS".into()),
            ("&SYSVER", env!("CARGO_PKG_VERSION").into()),
            (
                "&SYSTEM_ID",
                format!("{} {}", std::env::consts::OS, std::env::consts::ARCH),
            ),
            ("&SYSDATC", date),
            ("&SYSDATE", short_date),
            ("&SYSTIME", time),
            (
                "&SYSJOB",
                self.options.job.clone().unwrap_or_else(|| "(NOJOB)".into()),
            ),
            (
                "&SYSSTEP",
                self.options
                    .step
                    .clone()
                    .unwrap_or_else(|| "(NOSTEP)".into()),
            ),
            ("&SYSPARM", self.options.sysparm.clone()),
            ("&SYSOPT_DBCS", u8::from(crate::codepage::active().is_mixed() || matches!(self.options.source_encoding, super::SourceEncoding::Ebcdic { ccsid, .. } if crate::codepage::get(ccsid).is_some_and(|cp| cp.is_mixed()))).to_string()),
            ("&SYSOPT_RENT", "0".into()),
            ("&SYSOPT_XOBJECT", "0".into()),
            ("&SYSOPT_OPTABLE", self.controls.optable.clone()),
            ("&SYSOPT_CURR_OPTABLE", self.controls.optable.clone()),
            ("&SYSCODEPAGE", crate::codepage::active().ccsid.to_string()),
            (
                "&SYSOPT_CODEPAGE",
                crate::codepage::active().ccsid.to_string(),
            ),
        ] {
            self.system_values.insert(name.into(), value);
        }
    }

    fn local_context(&self, scope: &mut Scope, statement: &Statement, definition_file: &str) {
        for (name, value) in [
            ("&SYSECT", self.current_section.clone()),
            ("&SYSLOC", self.current_location.clone()),
            ("&SYSSTYP", self.current_section_type.clone()),
            (
                "&SYSCLOCK",
                super::context::clock(std::time::SystemTime::now()).3,
            ),
            ("&SYSSEQF", statement.sequence.clone()),
        ] {
            scope.values.insert(name.into(), value);
        }
        for dd in [
            "SYSIN", "SYSLIB", "SYSLIN", "SYSPRINT", "SYSPUNCH", "SYSTERM", "SYSADATA",
        ] {
            let mut data = if dd == "SYSLIB" && definition_file == self.input_file {
                super::AssemblyDataSet::default()
            } else {
                let library = if dd == "SYSLIB" {
                    Path::new(definition_file)
                        .parent()
                        .and_then(|p| self.options.library_data_sets.get(p))
                } else {
                    None
                };
                library
                    .or_else(|| self.options.data_sets.get(dd))
                    .cloned()
                    .unwrap_or_else(|| {
                        let source = if dd == "SYSIN" {
                            Some(self.input_file.as_str())
                        } else if dd == "SYSLIB" && definition_file != self.input_file {
                            Some(definition_file)
                        } else {
                            None
                        };
                        source.map_or_else(super::AssemblyDataSet::default, |file| {
                            super::AssemblyDataSet {
                                name: file.into(),
                                member: Path::new(file)
                                    .file_stem()
                                    .map_or_else(String::new, |v| v.to_string_lossy().into()),
                                volume: String::new(),
                            }
                        })
                    })
            };
            if dd == "SYSLIB" && !data.name.is_empty() && data.member.is_empty() {
                data.member = Path::new(definition_file)
                    .file_stem()
                    .map_or_else(String::new, |v| v.to_string_lossy().into());
            }
            for (suffix, value) in [
                ("DSN", data.name),
                ("MEMBER", data.member),
                ("VOLUME", data.volume),
            ] {
                scope.values.insert(format!("&{dd}_{suffix}"), value);
            }
        }
    }
    fn operation_attribute(&self, name: &str) -> char {
        let name = name.to_ascii_uppercase();
        let builtin_alias = self.aliases.contains_key(&name);
        let name = match self.aliases.get(&name) {
            Some(Some(op)) => op.as_str(),
            Some(None) => return 'U',
            None => &name,
        };
        if !builtin_alias && self.macro_types.contains(name) {
            return 'M';
        }
        if directive(name)
            || matches!(
                name,
                "COPY"
                    | "MACRO"
                    | "MEND"
                    | "OPSYN"
                    | "MNOTE"
                    | "AIF"
                    | "AGO"
                    | "ACTR"
                    | "ANOP"
                    | "MEXIT"
                    | "SETA"
                    | "SETB"
                    | "SETC"
                    | "LCLA"
                    | "LCLB"
                    | "LCLC"
                    | "GBLA"
                    | "GBLB"
                    | "GBLC"
            )
        {
            return 'A';
        }
        if instruction_size(name).is_some() {
            return if matches!(name, "BR" | "NOP" | "NOPR")
                || name.starts_with('J')
                || matches!(
                    name,
                    "B" | "BE"
                        | "BNE"
                        | "BH"
                        | "BL"
                        | "BNH"
                        | "BNL"
                        | "BO"
                        | "BNO"
                        | "BP"
                        | "BM"
                        | "BZ"
                        | "BNZ"
                        | "BNP"
                        | "BNM"
                ) {
                'E'
            } else {
                'O'
            };
        }
        if member(name, &self.source_file, &self.library_dirs).is_ok() {
            'S'
        } else {
            'U'
        }
    }
    fn mark_literal(&mut self, literal: &str) {
        if super::assemble::literal_has_location(&literal[1..]) {
            self.current_literals.insert(literal.into());
        } else {
            self.defined_literals.insert(literal.into());
        }
    }

    fn literal_attribute(&mut self, literal: &str, kind: u8) -> Result<String, String> {
        let (length, symbol_type) =
            super::assemble::literal_attributes(literal, &self.attribute_values)?;
        if kind == b'D' {
            return Ok(
                u8::from(if super::assemble::literal_has_location(&literal[1..]) {
                    self.current_literals.contains(literal)
                } else {
                    self.defined_literals.contains(literal)
                })
                .to_string(),
            );
        }
        if matches!(kind, b'S' | b'I') {
            let (scale, integer) = super::assemble::storage_numeric_attributes(
                &literal[1..],
                length,
                &self.attribute_values,
            )?;
            self.mark_literal(literal);
            return Ok(if kind == b'S' { scale } else { integer }.to_string());
        }
        self.mark_literal(literal);
        Ok(if kind == b'L' {
            length.to_string()
        } else {
            format!("'{}'", crate::ebcdic::decode(&[symbol_type]))
        })
    }

    fn observe_literal_references(&mut self, statement: &Statement) {
        if instruction_size(&statement.op).is_some() {
            for operand in operands(&statement.args) {
                if operand.starts_with('=')
                    && super::assemble::literal_attributes(&operand, &self.attribute_values).is_ok()
                {
                    self.mark_literal(&operand);
                }
            }
        }
        let text = &statement.args;
        let (mut at, mut quoted) = (0, false);
        while at < text.len() {
            let c = text.as_bytes()[at];
            if c == b'\'' && (quoted || !super::assemble::attribute_quote(text, at)) {
                quoted = !quoted;
            }
            if !quoted
                && b"LTSIltsi".contains(&c)
                && text.as_bytes().get(at + 1) == Some(&b'\'')
                && text.as_bytes().get(at + 2) == Some(&b'=')
                && (at == 0 || !text.as_bytes()[at - 1].is_ascii_alphanumeric())
            {
                let Ok(end) = super::assemble::literal_end(text, at + 2) else {
                    break;
                };
                let literal = &text[at + 2..end];
                if super::assemble::literal_attributes(literal, &self.attribute_values).is_ok() {
                    self.mark_literal(literal);
                }
                at = end;
            } else {
                at += 1;
            }
        }
    }

    fn observe_attributes(&mut self, statement: &Statement) {
        if statement.op == "CXD" {
            let mut data = statement.clone();
            data.op = "DS".into();
            data.args = "A".into();
            self.observe_attributes(&data);
            return;
        }
        if statement.op == "DCB" {
            let mut data = statement.clone();
            data.op = "DS".into();
            data.args = "CL96".into();
            self.observe_attributes(&data);
            return;
        }
        if matches!(statement.op.as_str(), "TITLE" | "AMODE" | "RMODE") {
            return;
        }
        if matches!(statement.op.as_str(), "CSECT" | "RSECT" | "DSECT" | "START") {
            if !self.current_section.is_empty() {
                self.section_locations
                    .insert(self.current_section.clone(), self.current_location.clone());
            }
            self.current_section = statement.label.clone();
            self.current_location = self
                .section_locations
                .get(&statement.label)
                .cloned()
                .unwrap_or_else(|| statement.label.clone());
            self.current_section_type = if statement.op == "START" {
                "CSECT".into()
            } else {
                statement.op.clone()
            };
            self.location_sections
                .entry(statement.label.clone())
                .or_insert_with(|| (statement.label.clone(), self.current_section_type.clone()));
        } else if statement.op == "LOCTR" {
            if !self.current_section.is_empty() {
                self.section_locations
                    .insert(self.current_section.clone(), self.current_location.clone());
            }
            if let Some((section, kind)) = self.location_sections.get(&statement.label) {
                self.current_section = section.clone();
                self.current_section_type = kind.clone();
            }
            self.current_location = if statement.label.is_empty() {
                self.current_section.clone()
            } else {
                statement.label.clone()
            };
            self.location_sections
                .entry(self.current_location.clone())
                .or_insert_with(|| {
                    (
                        self.current_section.clone(),
                        self.current_section_type.clone(),
                    )
                });
        }
        self.observe_literal_references(statement);
        if matches!(statement.op.as_str(), "EXTRN" | "WXTRN") {
            for name in operands(&statement.args) {
                let name = name.to_ascii_uppercase();
                self.defined_symbols.insert(name.clone());
                self.ordinary_attributes.insert(
                    name,
                    (
                        Ok(1),
                        crate::ebcdic::encode(if statement.op == "WXTRN" { "$" } else { "T" })[0],
                    ),
                );
            }
        }
        if statement.label.is_empty() {
            return;
        }
        let name = statement.label.clone();
        self.defined_symbols.insert(name.clone());
        if statement.op == "EQU" {
            self.numeric_attributes.insert(name.clone(), Ok((0, 0)));
            self.storage_types.remove(&name);
            self.storage_programs.remove(&name);
            let attributes =
                super::expression::early_equ_attributes(&statement.args, &self.attribute_values)
                    .map_err(|e| statement.error(e));
            let mut changed = false;
            let ordinary = match &attributes {
                Ok(attributes) => {
                    let length = if let Some(length) = attributes.explicit_length() {
                        Ok(length)
                    } else {
                        self.equ_length(
                            operands(&statement.args).first().map_or("", String::as_str),
                        )
                    };
                    (length, attributes.type_byte())
                }
                Err(e) => (Err(e.clone()), crate::ebcdic::encode("U")[0]),
            };
            self.ordinary_attributes.insert(name.clone(), ordinary);
            if let Ok(attributes) = &attributes
                && let Some(value) = operands(&statement.args).first()
            {
                if let Ok(mut symbol) =
                    super::expression::evaluate_early(value, &self.attribute_values)
                {
                    attributes.apply(&mut symbol);
                    self.attribute_values.insert(name.clone(), symbol);
                    changed = true;
                } else {
                    self.pending_attribute_values.push((
                        name.clone(),
                        value.clone(),
                        attributes.clone(),
                    ));
                }
            }
            self.attributes.insert(name, attributes);
            // Revisit forward constants only when a new value became known.
            while changed {
                changed = false;
                self.pending_attribute_values
                    .retain(|(name, text, attributes)| {
                        if let Ok(mut value) =
                            super::expression::evaluate_early(text, &self.attribute_values)
                        {
                            attributes.apply(&mut value);
                            self.attribute_values.insert(name.clone(), value);
                            changed = true;
                            false
                        } else {
                            true
                        }
                    });
            }
        } else {
            let data = matches!(statement.op.as_str(), "DC" | "DS");
            let ordinary = if data {
                (
                    super::assemble::storage(
                        &statement.args,
                        statement.op == "DC",
                        &self.attribute_values,
                    )
                    .map(|(_, _, length)| length),
                    super::assemble::storage_type(&statement.args),
                )
            } else {
                let kind = if instruction_size(&statement.op).is_some() {
                    "I"
                } else if matches!(
                    statement.op.as_str(),
                    "CSECT" | "RSECT" | "DSECT" | "START" | "LOCTR" | "CATTR" | "DXD"
                ) {
                    "J"
                } else {
                    "U"
                };
                (
                    Ok(instruction_size(&statement.op).unwrap_or(1)),
                    crate::ebcdic::encode(kind)[0],
                )
            };
            let numeric = if data {
                ordinary
                    .0
                    .as_ref()
                    .map_err(Clone::clone)
                    .and_then(|length| {
                        super::assemble::storage_numeric_attributes(
                            &statement.args,
                            *length,
                            &self.attribute_values,
                        )
                    })
            } else {
                Err(
                    "scale/integer attributes require a supported numeric constant or EQU symbol"
                        .into(),
                )
            };
            self.numeric_attributes.insert(name.clone(), numeric);
            self.ordinary_attributes.insert(name.clone(), ordinary);
            // Supported DC/DS types carry an implicit assembler type. Other
            // generated labels have no program or assembler annotation.
            let annotations = if data {
                super::constants::annotations(&statement.args, &self.attribute_values)
            } else {
                Ok((None, String::new()))
            };
            self.storage_programs.insert(
                name.clone(),
                annotations.as_ref().map(|a| a.0).map_err(Clone::clone),
            );
            self.storage_types
                .insert(name.clone(), annotations.map(|a| a.1).unwrap_or_default());
            self.attributes.remove(&name);
        }
    }

    fn equ_length(&self, expression: &str) -> Result<u32, String> {
        let text = expression.trim_start_matches(['+', '-', '(']);
        let count = text
            .bytes()
            .take_while(|b| b.is_ascii_alphanumeric() || b"_@$#".contains(b))
            .count();
        if count == 0
            || text.as_bytes()[0].is_ascii_digit()
            || text.as_bytes().get(count) == Some(&b'\'')
        {
            return Ok(1);
        }
        self.ordinary_attributes
            .get(&text[..count].to_ascii_uppercase())
            .ok_or_else(|| {
                format!(
                    "length of EQU term {} is not available during macro expansion",
                    &text[..count]
                )
            })?
            .0
            .clone()
    }

    fn ordinary_attribute(&self, name: &str, kind: u8) -> Result<String, String> {
        let name = name.to_ascii_uppercase();
        if kind == b'D' {
            return Ok(u8::from(self.defined_symbols.contains(&name)).to_string());
        }
        let ahead = if self.ordinary_attributes.contains_key(&name) {
            None
        } else {
            self.lookahead(&name)?
        };
        if matches!(kind, b'S' | b'I') {
            let looked_up;
            let numeric = if let Some(value) = self.numeric_attributes.get(&name) {
                value.as_ref().map_err(Clone::clone)?
            } else if let Some(s) = &ahead {
                looked_up = if matches!(s.op.as_str(), "DC" | "DS") {
                    let (_, _, length) =
                        super::assemble::storage(&s.args, s.op == "DC", &self.attribute_values)?;
                    super::assemble::storage_numeric_attributes(
                        &s.args,
                        length,
                        &self.attribute_values,
                    )?
                } else if s.op == "EQU" {
                    (0, 0)
                } else {
                    return Err(format!(
                        "numeric attributes of {name} require a numeric declaration"
                    ));
                };
                &looked_up
            } else {
                return Err(format!(
                    "numeric attributes of {name} are unavailable after lookahead"
                ));
            };
            return Ok(if kind == b'S' { numeric.0 } else { numeric.1 }.to_string());
        }
        if let Some(Err(error)) = self.attributes.get(&name) {
            return Err(format!(
                "attributes of {name} are unavailable during macro expansion: {error}"
            ));
        }
        let looked_up;
        let value = if let Some(value) = self.ordinary_attributes.get(&name) {
            Some(value)
        } else if let Some(s) = &ahead {
            looked_up = if matches!(s.op.as_str(), "DC" | "DS") {
                (
                    super::assemble::storage(&s.args, s.op == "DC", &self.attribute_values)
                        .map(|(_, _, length)| length),
                    super::assemble::storage_type(&s.args),
                )
            } else if matches!(s.op.as_str(), "EXTRN" | "WXTRN") {
                (
                    Ok(1),
                    crate::ebcdic::encode(if s.op == "WXTRN" { "$" } else { "T" })[0],
                )
            } else if s.op == "EQU" {
                let attributes =
                    super::expression::early_equ_attributes(&s.args, &self.attribute_values)?;
                (
                    attributes.explicit_length().map_or_else(
                        || self.equ_length(operands(&s.args).first().map_or("", String::as_str)),
                        Ok,
                    ),
                    attributes.type_byte(),
                )
            } else {
                (
                    Ok(instruction_size(&s.op).unwrap_or(1)),
                    crate::ebcdic::encode(
                        if matches!(
                            s.op.as_str(),
                            "CSECT" | "RSECT" | "DSECT" | "START" | "LOCTR" | "CATTR" | "DXD"
                        ) {
                            "J"
                        } else if instruction_size(&s.op).is_some() {
                            "I"
                        } else {
                            "U"
                        },
                    )[0],
                )
            };
            Some(&looked_up)
        } else {
            None
        };
        let Some((length, symbol_type)) = value else {
            return if kind == b'T' {
                Ok("'U'".into())
            } else {
                Err(format!("length of {name} is not available after lookahead"))
            };
        };
        let character = crate::ebcdic::decode(&[*symbol_type]);
        if kind == b'T' {
            return Ok(format!(
                "'{}'",
                character.replace('\'', "''").replace('&', "&&")
            ));
        }
        if "MNOTU$".contains(character.as_str()) {
            return Err(format!(
                "length attribute of {name} is invalid for type {character}"
            ));
        }
        length.as_ref().map(u32::to_string).map_err(Clone::clone)
    }

    fn query_attribute(&self, name: &str, program: bool) -> Result<String, String> {
        let name = name.to_ascii_uppercase();
        if let Some(attributes) = self.attributes.get(&name) {
            return attributes
                .as_ref()
                .map(|a| a.query(program))
                .map_err(|e| format!("cannot query {name} during macro expansion: {e}"));
        }
        Ok(if program {
            self.storage_programs
                .get(&name)
                .cloned()
                .unwrap_or(Ok(None))?
                .map_or_else(String::new, |n| crate::ebcdic::decode(&n.to_be_bytes()))
        } else {
            self.storage_types.get(&name).cloned().unwrap_or_default()
        })
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum SetKind {
    Arithmetic,
    Boolean,
    Character,
}

impl SetKind {
    fn from_op(op: &str) -> Self {
        match op.as_bytes().last() {
            Some(b'A') => Self::Arithmetic,
            Some(b'B') => Self::Boolean,
            _ => Self::Character,
        }
    }
    fn initial(self) -> String {
        if self == Self::Character {
            String::new()
        } else {
            "0".into()
        }
    }
}

#[derive(Clone)]
struct SetSymbol {
    kind: SetKind,
    array: bool,
    extent: usize,
    scalar: String,
    elements: BTreeMap<usize, String>,
}

impl SetSymbol {
    fn new(kind: SetKind, dimension: Option<usize>) -> Self {
        Self {
            kind,
            array: dimension.is_some(),
            extent: dimension.unwrap_or(0),
            scalar: kind.initial(),
            elements: BTreeMap::new(),
        }
    }
}

fn has_variable_symbol(text: &str) -> bool {
    let mut chars = text.chars();
    while let Some(c) = chars.next() {
        if c == '&' {
            match chars.next() {
                Some('&') => {}
                Some(c) if c.is_ascii_alphabetic() || "_@$#(".contains(c) => return true,
                _ => {}
            }
        }
    }
    false
}
