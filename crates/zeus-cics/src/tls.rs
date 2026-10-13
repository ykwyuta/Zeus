//! TLS for the TN3270 server (TN3270 over TLS, as emulators connect to
//! port 992): rustls with a certificate and key in PEM files.
//!
//! Each TLS connection gets a relay thread between the network socket and
//! one end of a socket pair; the server reads and writes plain text at the
//! other end, as it does for a connection without TLS. (Elsewhere than
//! Unix the pair is two connected loopback TCP sockets.)

use std::io::{self, Read, Write};
use std::net::{Shutdown, TcpStream};
#[cfg(unix)]
use std::os::unix::net::UnixStream as Pipe;
use std::sync::Arc;
use std::time::Duration;

use rustls::{ServerConfig, ServerConnection};
use rustls_pki_types::pem::PemObject;
use rustls_pki_types::{CertificateDer, PrivateKeyDer};

#[cfg(not(unix))]
type Pipe = TcpStream;

/// Two connected sockets.
#[cfg(unix)]
fn pair() -> io::Result<(Pipe, Pipe)> {
    Pipe::pair()
}

#[cfg(not(unix))]
fn pair() -> io::Result<(Pipe, Pipe)> {
    let l = std::net::TcpListener::bind(("127.0.0.1", 0))?;
    let a = TcpStream::connect(l.local_addr()?)?;
    let (b, _) = l.accept()?;
    a.set_nodelay(true)?;
    b.set_nodelay(true)?;
    Ok((a, b))
}

/// The server configuration of a certificate chain and its private key
/// (PEM files).
pub fn config(cert: &str, key: &str) -> io::Result<Arc<ServerConfig>> {
    let certs: Vec<CertificateDer<'static>> = CertificateDer::pem_file_iter(cert)
        .map_err(|e| io::Error::other(format!("{cert}: {e}")))?
        .collect::<Result<_, _>>()
        .map_err(|e| io::Error::other(format!("{cert}: {e}")))?;
    let key =
        PrivateKeyDer::from_pem_file(key).map_err(|e| io::Error::other(format!("{key}: {e}")))?;
    let provider = Arc::new(rustls::crypto::ring::default_provider());
    let cfg = ServerConfig::builder_with_provider(provider)
        .with_safe_default_protocol_versions()
        .map_err(|e| io::Error::other(e.to_string()))?
        .with_no_client_auth()
        .with_single_cert(certs, key)
        .map_err(|e| io::Error::other(e.to_string()))?;
    Ok(Arc::new(cfg))
}

/// A connection of the server: a TCP socket, or the plain-text end of a
/// TLS relay.
pub enum Conn {
    Tcp(TcpStream),
    Relay(Pipe),
}

impl Conn {
    pub fn try_clone(&self) -> io::Result<Conn> {
        Ok(match self {
            Conn::Tcp(s) => Conn::Tcp(s.try_clone()?),
            Conn::Relay(s) => Conn::Relay(s.try_clone()?),
        })
    }

    pub fn shutdown(&self) -> io::Result<()> {
        match self {
            Conn::Tcp(s) => s.shutdown(Shutdown::Both),
            Conn::Relay(s) => s.shutdown(Shutdown::Both),
        }
    }
}

impl Read for Conn {
    fn read(&mut self, b: &mut [u8]) -> io::Result<usize> {
        match self {
            Conn::Tcp(s) => s.read(b),
            Conn::Relay(s) => s.read(b),
        }
    }
}

impl Write for Conn {
    fn write(&mut self, b: &[u8]) -> io::Result<usize> {
        match self {
            Conn::Tcp(s) => s.write(b),
            Conn::Relay(s) => s.write(b),
        }
    }
    fn flush(&mut self) -> io::Result<()> {
        match self {
            Conn::Tcp(s) => s.flush(),
            Conn::Relay(s) => s.flush(),
        }
    }
}

fn would_block(e: &io::Error) -> bool {
    matches!(
        e.kind(),
        io::ErrorKind::WouldBlock | io::ErrorKind::Interrupted
    )
}

/// Writes all of `data` to a non-blocking socket.
fn write_all(s: &mut impl Write, mut data: &[u8]) -> io::Result<()> {
    while !data.is_empty() {
        match s.write(data) {
            Ok(0) => return Err(io::ErrorKind::WriteZero.into()),
            Ok(n) => data = &data[n..],
            Err(e) if would_block(&e) => std::thread::sleep(Duration::from_millis(2)),
            Err(e) => return Err(e),
        }
    }
    Ok(())
}

/// Accepts TLS on `tcp`: the plain-text end of the connection.
pub fn accept(tcp: TcpStream, cfg: Arc<ServerConfig>) -> io::Result<Conn> {
    let (app, mut inner) = pair()?;
    let mut conn = ServerConnection::new(cfg).map_err(|e| io::Error::other(e.to_string()))?;
    let mut tcp = tcp;
    tcp.set_nonblocking(true)?;
    inner.set_nonblocking(true)?;
    std::thread::spawn(move || {
        let mut buf = vec![0u8; 16384];
        'relay: loop {
            let mut progress = false;
            // network -> TLS
            match conn.read_tls(&mut tcp) {
                Ok(0) => break,
                Ok(_) => {
                    progress = true;
                    if conn.process_new_packets().is_err() {
                        let _ = conn.write_tls(&mut tcp);
                        break;
                    }
                }
                Err(e) if would_block(&e) => {}
                Err(_) => break,
            }
            // TLS plain text -> server
            loop {
                match conn.reader().read(&mut buf) {
                    Ok(0) => break 'relay,
                    Ok(n) => {
                        progress = true;
                        if write_all(&mut inner, &buf[..n]).is_err() {
                            break 'relay;
                        }
                    }
                    Err(e) if would_block(&e) => break,
                    Err(_) => break 'relay,
                }
            }
            // server -> TLS
            match inner.read(&mut buf) {
                Ok(0) => {
                    conn.send_close_notify();
                    while conn.wants_write() {
                        if conn.write_tls(&mut tcp).is_err() {
                            break;
                        }
                    }
                    break;
                }
                Ok(n) => {
                    progress = true;
                    if conn.writer().write_all(&buf[..n]).is_err() {
                        break;
                    }
                }
                Err(e) if would_block(&e) => {}
                Err(_) => break,
            }
            // TLS -> network
            while conn.wants_write() {
                match conn.write_tls(&mut tcp) {
                    Ok(_) => progress = true,
                    Err(e) if would_block(&e) => break,
                    Err(_) => break 'relay,
                }
            }
            if !progress {
                std::thread::sleep(Duration::from_millis(2));
            }
        }
        let _ = tcp.shutdown(Shutdown::Both);
        let _ = inner.shutdown(Shutdown::Both);
    });
    Ok(Conn::Relay(app))
}

/// Telnet option START_TLS and its FOLLOWS subcommand.
const OPT_STARTTLS: u8 = 46;
const FOLLOWS: u8 = 1;

/// STARTTLS on a plain connection: the server offers it; a client that
/// agrees (WILL, then SB FOLLOWS) continues over TLS. The plain or the TLS
/// connection to go on with.
pub fn starttls(tcp: TcpStream, cfg: Arc<ServerConfig>) -> io::Result<Conn> {
    use crate::tn3270::{DO, Event, IAC, Reader, SB, SE, WILL, WONT};
    let mut tcp = tcp;
    tcp.write_all(&[IAC, DO, OPT_STARTTLS])?;
    tcp.flush()?;
    // a client that does not answer within a few seconds goes on in plain
    tcp.set_read_timeout(Some(Duration::from_secs(5)))?;
    let answer = {
        let mut r = Reader::new(&mut tcp);
        loop {
            match r.next_event() {
                Ok(Event::Option(WILL, OPT_STARTTLS)) => break true,
                Ok(Event::Option(WONT, OPT_STARTTLS)) | Err(_) => break false,
                Ok(_) => {}
            }
        }
    };
    tcp.set_read_timeout(None)?;
    if !answer {
        return Ok(Conn::Tcp(tcp));
    }
    tcp.write_all(&[IAC, SB, OPT_STARTTLS, FOLLOWS, IAC, SE])?;
    tcp.flush()?;
    {
        let mut r = Reader::new(&mut tcp);
        loop {
            if let Event::Sub(s) = r.next_event()?
                && s.first() == Some(&OPT_STARTTLS)
            {
                break;
            }
        }
    }
    accept(tcp, cfg)
}
