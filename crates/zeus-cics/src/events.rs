//! CICS event processing: event bindings (`.evbind` files, as CICS
//! Explorer exports them) whose capture specifications match points in
//! the programs, and the transaction start adapter that emits each event
//! by starting a transaction with the event in channel DFHEP.EVENT (the
//! container-based CCE format: DFHEP.CCECONTEXT, then DFHEP.NAME.nnnnn
//! and DFHEP.DATA.nnnnn for each information item).
//!
//! Capture points: after (or before) EXEC CICS LINK, filtered by the
//! current program, the transaction and the linked program; data from the
//! COMMAREA or a container of the LINK's channel.

use std::collections::BTreeMap;

/// A filter on a value: the operator (EQ, NE, SW starts with, DNSW does
/// not start with, GT, GE, LT, LE) and the value.
#[derive(Debug, Clone, PartialEq)]
pub struct Filter {
    pub op: String,
    pub value: String,
}

impl Filter {
    fn of(node: Option<roxmltree::Node>) -> Option<Filter> {
        let n = node?;
        let op = n
            .attribute("filterOperator")
            .unwrap_or("OFF")
            .to_ascii_uppercase();
        if op == "OFF" {
            return None;
        }
        Some(Filter {
            op,
            value: n
                .attribute("filterValue")
                .or_else(|| n.attribute("value"))
                .unwrap_or("")
                .to_string(),
        })
    }

    pub fn matches(&self, v: &str) -> bool {
        let (v, f) = (v.trim_end(), self.value.trim_end());
        match self.op.as_str() {
            "EQ" => v == f,
            "NE" => v != f,
            "SW" => v.starts_with(f),
            "DNSW" => !v.starts_with(f),
            "GT" => v > f,
            "GE" => v >= f,
            "LT" => v < f,
            "LE" => v <= f,
            _ => true,
        }
    }
}

fn pass(f: &Option<Filter>, v: &str) -> bool {
    f.as_ref().is_none_or(|f| f.matches(v))
}

/// Where a captured item comes from.
#[derive(Debug, Clone, PartialEq)]
pub enum Source {
    Commarea,
    Container(String),
}

/// An item of an event: its name, where it is captured and its format.
#[derive(Debug, Clone, PartialEq)]
pub struct Item {
    pub name: String,
    pub source: Source,
    pub offset: usize,
    pub length: usize,
    pub format_length: usize,
    pub numeric: bool,
}

/// A capture specification on EXEC CICS LINK.
#[derive(Debug, Clone, PartialEq)]
pub struct Capture {
    pub name: String,
    pub event: String,
    pub before: bool,
    pub transaction: Option<Filter>,
    pub program: Option<Filter>,
    pub linked: Option<Filter>,
    pub items: Vec<Item>,
}

/// An event binding with its transaction start adapter.
#[derive(Debug, Clone, PartialEq)]
pub struct Binding {
    pub name: String,
    pub user_tag: String,
    pub captures: Vec<Capture>,
    pub transid: String,
}

fn child<'a, 'i>(n: roxmltree::Node<'a, 'i>, tag: &str) -> Option<roxmltree::Node<'a, 'i>> {
    n.children()
        .find(|c| c.is_element() && c.tag_name().name() == tag)
}

/// Reads an event binding; capture specifications at points Zeus does
/// not capture (other commands, program initiation) are left out, with
/// a note each.
pub fn parse(name: &str, xml: &str) -> Result<(Binding, Vec<String>), String> {
    let doc = roxmltree::Document::parse(xml).map_err(|e| format!("{name}: {e}"))?;
    let root = doc.root_element();
    let text =
        |n: Option<roxmltree::Node>| n.and_then(|n| n.text()).unwrap_or("").trim().to_string();
    let mut notes = Vec::new();
    let mut captures = Vec::new();
    for cs in root
        .children()
        .filter(|c| c.is_element() && c.tag_name().name() == "eventCaptureSpecification")
    {
        let cname = text(child(cs, "name"));
        let event = text(child(cs, "eventIdentifier"));
        let Some(filter) = child(cs, "filter") else {
            continue;
        };
        let ctx = child(filter, "contextFilter");
        let link = child(filter, "locationFilter").and_then(|l| child(l, "linkCommand"));
        let Some(link) = link else {
            notes.push(format!(
                "{name}: capture specification {cname}: only EXEC CICS LINK is captured"
            ));
            continue;
        };
        if child(filter, "dataFilter").is_some_and(|d| d.children().any(|c| c.is_element())) {
            notes.push(format!(
                "{name}: capture specification {cname}: data filters are not applied"
            ));
        }
        let mut items = Vec::new();
        if let Some(dc) = child(cs, "dataCapture") {
            for d in dc
                .descendants()
                .filter(|d| d.tag_name().name() == "dataCaptureItem")
            {
                let num = |a: &str| {
                    d.attribute(a)
                        .and_then(|v| v.parse().ok())
                        .unwrap_or(0usize)
                };
                let source = match d.attribute("source").unwrap_or("") {
                    "COMMAREA" => Source::Commarea,
                    "CONTAINER" | "CHANNEL" => {
                        Source::Container(d.attribute("container").unwrap_or("").to_string())
                    }
                    other => {
                        notes.push(format!(
                            "{name}: capture specification {cname}: data from {other} is not captured"
                        ));
                        continue;
                    }
                };
                let length = num("captureLength");
                items.push(Item {
                    name: d.attribute("eventItemName").unwrap_or("").to_string(),
                    source,
                    offset: num("offset"),
                    length,
                    format_length: match num("formatlength") {
                        0 => length,
                        n => n,
                    },
                    numeric: d.attribute("formatdataType") == Some("numeric"),
                });
            }
        }
        captures.push(Capture {
            name: cname,
            event,
            before: link.attribute("isPre") == Some("true"),
            transaction: Filter::of(ctx.and_then(|c| child(c, "transactionId"))),
            program: Filter::of(ctx.and_then(|c| child(c, "currentProgram"))),
            linked: Filter::of(child(link, "PROGRAM")),
            items,
        });
    }
    let adapter = root
        .descendants()
        .find(|n| n.tag_name().name() == "cicsTransactionAdapter");
    let Some(adapter) = adapter else {
        return Err(format!(
            "{name}: only the CICS transaction start adapter is supported"
        ));
    };
    let transid = text(child(adapter, "transactionId"));
    if transid.is_empty() {
        return Err(format!("{name}: the adapter has no transaction"));
    }
    Ok((
        Binding {
            name: name.to_string(),
            user_tag: text(child(root, "userTag")),
            captures,
            transid,
        },
        notes,
    ))
}

/// What a LINK offers to capture.
pub struct LinkPoint<'a> {
    pub before: bool,
    pub transid: &'a str,
    pub program: &'a str,
    pub linked: &'a str,
    pub commarea: Option<&'a [u8]>,
    pub channel: Option<&'a BTreeMap<String, Vec<u8>>>,
}

/// An event to emit: the adapter's transaction and the channel.
pub struct Emission {
    pub transid: String,
    pub containers: BTreeMap<String, Vec<u8>>,
}

fn fit(mut b: Vec<u8>, n: usize, numeric: bool) -> Vec<u8> {
    let zero = zeus_cobol::ebcdic::encode("0")[0];
    if numeric && b.len() < n {
        let mut z = vec![zero; n - b.len()];
        z.extend(b);
        return z;
    }
    b.resize(n, zeus_cobol::ebcdic::SPACE);
    b
}

fn padded(s: &str, n: usize) -> Vec<u8> {
    fit(zeus_cobol::ebcdic::encode(s), n, false)
}

/// The events the bindings capture at a LINK.
pub fn capture(bindings: &[Binding], p: &LinkPoint, now: &str) -> Vec<Emission> {
    let mut out = Vec::new();
    for b in bindings {
        for c in &b.captures {
            if c.before != p.before
                || !pass(&c.transaction, p.transid)
                || !pass(&c.program, p.program)
                || !pass(&c.linked, p.linked)
            {
                continue;
            }
            let mut containers = BTreeMap::new();
            // the context: binding, event, capture specification, user
            // tag, transaction, program, time
            let mut ctx = Vec::new();
            ctx.extend(padded(&b.name, 32));
            ctx.extend(padded(&c.event, 32));
            ctx.extend(padded(&c.name, 32));
            ctx.extend(padded(&b.user_tag, 8));
            ctx.extend(padded(p.transid, 4));
            ctx.extend(padded(p.program, 8));
            ctx.extend(padded(now, 26));
            containers.insert("DFHEP.CCECONTEXT".to_string(), ctx);
            for (k, it) in c.items.iter().enumerate() {
                let src: &[u8] = match &it.source {
                    Source::Commarea => p.commarea.unwrap_or(&[]),
                    Source::Container(n) => p
                        .channel
                        .and_then(|ch| ch.get(n.trim_end()))
                        .map_or(&[], Vec::as_slice),
                };
                let end = (it.offset + it.length).min(src.len());
                let data = src.get(it.offset..end).unwrap_or(&[]).to_vec();
                let data = fit(fit(data, it.length, false), it.format_length, it.numeric);
                containers.insert(format!("DFHEP.NAME.{:05}", k + 1), padded(&it.name, 32));
                containers.insert(format!("DFHEP.DATA.{:05}", k + 1), data);
            }
            out.push(Emission {
                transid: b.transid.clone(),
                containers,
            });
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const BIND: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<ns2:eventBinding xmlns:ns2="http://www.ibm.com/xmlns/prod/cics/eventprocessing/eventbinding">
  <userTag>TAG</userTag>
  <eventCaptureSpecification>
    <name>spec1</name>
    <eventIdentifier>Count</eventIdentifier>
    <filter>
      <contextFilter>
        <transactionId filterValue="" filterOperator="OFF"/>
        <currentProgram filterValue="LG" filterOperator="SW"/>
      </contextFilter>
      <locationFilter filterType="CICS_API">
        <linkCommand isPre="false" adVerb="PROGRAM" verb="LINK">
          <PROGRAM filterValue="" filterOperator="OFF" keyword="PROGRAM"/>
        </linkCommand>
      </locationFilter>
      <dataFilter/>
    </filter>
    <dataCapture>
      <captureItem><dataCaptureItem formatdataType="text" formatlength="6" eventItemName="Request_Type" captureLength="6" offset="0" container="" source="COMMAREA"/></captureItem>
      <captureItem><dataCaptureItem formatdataType="numeric" formatlength="2" eventItemName="Return_code" captureLength="2" offset="6" container="" source="COMMAREA"/></captureItem>
    </dataCapture>
  </eventCaptureSpecification>
  <eventDispatcherSpecification><eventDispatcher><eventDispatcherAdapter>
    <cicsTransactionAdapter><transactionId>LGST</transactionId><format>CCE</format></cicsTransactionAdapter>
  </eventDispatcherAdapter></eventDispatcher></eventDispatcherSpecification>
</ns2:eventBinding>"#;

    #[test]
    fn link_events() {
        let (b, notes) = parse("GENAEV01", BIND).unwrap();
        assert!(notes.is_empty(), "{notes:?}");
        assert_eq!(b.transid, "LGST");
        assert_eq!(b.captures.len(), 1);
        let ca = zeus_cobol::ebcdic::encode("01ICUS00rest");
        let p = LinkPoint {
            before: false,
            transid: "SSC1",
            program: "LGTESTC1",
            linked: "LGICUS01",
            commarea: Some(&ca),
            channel: None,
        };
        let e = capture(std::slice::from_ref(&b), &p, "2026-10-12-10.00.00.000000");
        assert_eq!(e.len(), 1);
        assert_eq!(e[0].transid, "LGST");
        assert_eq!(
            e[0].containers["DFHEP.DATA.00001"],
            zeus_cobol::ebcdic::encode("01ICUS")
        );
        assert_eq!(
            e[0].containers["DFHEP.DATA.00002"],
            zeus_cobol::ebcdic::encode("00")
        );
        // other programs, and before the LINK: no event
        let q = LinkPoint {
            program: "XYZ",
            ..p
        };
        assert!(capture(std::slice::from_ref(&b), &q, "").is_empty());
    }
}
