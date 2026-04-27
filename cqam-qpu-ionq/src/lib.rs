//! IonQ QPU backend for CQAM.
//!
//! Provides circuit submission to IonQ hardware and simulators via the
//! IonQ Cloud REST API v0.4 (api.ionq.co/v0.4). Circuits are serialized
//! to the IonQ native JSON format (`ionq.circuit.v1`) using the QIS gateset.
//!
//! IonQ trapped-ion hardware has all-to-all qubit connectivity, so no
//! SWAP insertion is needed in the routing stage.

pub mod backend;
pub mod calibration;
pub mod circuit;
pub mod error;
pub mod rest;

pub use backend::IonQQpuBackend;
pub use error::IonQError;
