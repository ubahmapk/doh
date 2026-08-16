pub mod error;
pub mod message;
pub mod transport;

pub use error::DohError;
pub use message::{Answer, ParsedResponse};
pub use transport::doh::{DohTransport, HttpMethod};
pub use transport::doq::DoqTransport;
pub use transport::dot::DotTransport;
pub use transport::Transport;

pub use hickory_proto::op::ResponseCode;
pub use hickory_proto::rr::RecordType;
