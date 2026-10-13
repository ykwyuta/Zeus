//! `zmq` - puts and gets text messages on the queue manager of ZEUS_MQ.
//!
//! ```text
//! zmq put [--reply-to Q] QUEUE TEXT...   one message per argument (in the code page)
//! zmq get [--wait MS] QUEUE              the messages, one per line (taken off the
//!                                        queue); with --wait, also those arriving
//!                                        until none has come for MS milliseconds
//! ```

use std::process::ExitCode;

use zeus_cobol::ebcdic;
use zeus_mq::monitor::{get_messages, put_message};

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    // After the queue name, --help can be message text. Only interpret help
    // in the command/option prefix, so putting that text still works.
    if args
        .first()
        .is_some_and(|a| matches!(a.as_str(), "--help" | "-h"))
        || args
            .get(1)
            .is_some_and(|a| matches!(a.as_str(), "--help" | "-h"))
    {
        print!("{}", include_str!("../../../docs/help/zmq.txt"));
        return ExitCode::SUCCESS;
    }
    let a: Vec<&str> = args.iter().map(String::as_str).collect();
    let r = match a.as_slice() {
        ["put", "--reply-to", r, q, texts @ ..] if !texts.is_empty() => texts
            .iter()
            .try_for_each(|t| put_message(q, &ebcdic::encode(t), Some(r))),
        ["put", q, texts @ ..] if !texts.is_empty() => texts
            .iter()
            .try_for_each(|t| put_message(q, &ebcdic::encode(t), None)),
        ["get", "--wait", ms, q] => match ms.parse() {
            Ok(ms) => get(q, ms),
            Err(_) => Err(format!("{ms}: not a number of milliseconds")),
        },
        ["get", q] => get(q, 0),
        _ => {
            eprintln!("usage: zmq put [--reply-to Q] QUEUE TEXT... | get [--wait MS] QUEUE");
            return ExitCode::from(2);
        }
    };
    match r {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("zmq: {e}");
            ExitCode::from(12)
        }
    }
}

fn get(q: &str, wait: i32) -> Result<(), String> {
    get_messages(q, wait).map(|ms| {
        for m in ms {
            println!("{}", ebcdic::decode(&m).trim_end());
        }
    })
}
