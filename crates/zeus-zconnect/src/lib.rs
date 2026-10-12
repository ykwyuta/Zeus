//! A REST gateway for z/OS programs in the manner of z/OS Connect: the API
//! (an OpenAPI document and an operation directory per path and method)
//! maps an HTTP request to the COMMAREA of a CICS program or the input
//! message of an IMS transaction, and the program's output to the HTTP
//! response.

pub mod api;
pub mod dai;
pub mod expr;
pub mod gateway;
