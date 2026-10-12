//! IBM MQ for COBOL programs: the MQI calls (MQCONN, MQOPEN, MQPUT,
//! MQGET, ...) run on an IBM MQ queue manager through the IBM MQ C client
//! library (libmqic_r), which talks to the queue manager over TCP (a
//! client channel).
//!
//! Programs keep their z/OS view of the MQI: structures with big-endian
//! integers and EBCDIC names, message data in their code page. The calls
//! convert the structures to the native C form and back; message data is
//! passed as it is, described by the program's MQMD (Encoding 785 and
//! the CCSID of the code page for MQCCSI_Q_MGR), so the queue manager
//! converts it for other applications (MQGMO_CONVERT).
//!
//! The connection: ZEUS_MQ=`mq://user:password@host:port/QMGR/CHANNEL`
//! (or MQSERVER set as for any MQ client). The library is found through
//! ZEUS_MQ_LIB, MQ_INSTALLATION_PATH, /opt/mqm or the loader's path.
//! Calls with the default connection handle (0, as under CICS) connect
//! to the queue manager of ZEUS_MQ on first use.

mod ffi;
pub mod layout;
pub mod monitor;
pub mod mqi;

pub use mqi::{end_run, syncpoint};

/// Makes the MQI calls callable by the programs of `m`.
pub fn register(m: &mut zeus_cobol::vm::Machine) {
    for (name, f) in mqi::CALLS {
        m.add_native(name, *f);
    }
}
