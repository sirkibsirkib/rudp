
use std::io;
use std;
use std::time::Duration;
use resend_predicates;


/// Object that allows sending and receiving of datagram-like byte payloads
/// using a `io::Write`-like API (buffers and such).
/// 
/// We suggest implementing this trivial trait for, and using `mio::net::UdpSocket`.
pub trait UdpLike: Sized {
	fn send(&mut self, buf: &[u8]) -> io::Result<usize>;
	fn recv(&mut self, buf: &mut [u8]) -> io::Result<usize>;
}

impl UdpLike for std::net::UdpSocket {
	fn send(&mut self, buf: &[u8]) -> io::Result<usize> {
		std::net::UdpSocket::send(self, buf)
	}
	fn recv(&mut self, buf: &mut [u8]) -> io::Result<usize> {
		std::net::UdpSocket::recv(self, buf)
	}
}


////////////////////////////////////////////////////////////////////////////////


/// This object contains all the configuration information required by the `Endpoint`
/// struct. `default()` will return a new configuration object with default values
/// which can be directly manipulated before being passed to the Endpoint.
#[derive(Derivative)]
#[derivative(Debug)]
pub struct EndpointConfig {
	pub max_msg_size: usize,
	pub buffer_grow_space: usize,
	pub window_size: u32,
	pub new_set_unsent_action: NewSetUnsent,

	#[derivative(Debug="ignore")]
	pub resend_predicate: Box<FnMut(u32, Duration) -> bool>,
	pub min_heartbeat_period: Duration,
}
impl EndpointConfig {

	
	/// Return a newly-constructed `EndpointConfig` with fields populated by
	/// default values.
	pub fn default() -> Self {
		EndpointConfig {
			max_msg_size: 2048,
			buffer_grow_space: 1024,
			window_size: 64,
			new_set_unsent_action: NewSetUnsent::Panic,
			resend_predicate: Box::new(resend_predicates::medium_combination),
			min_heartbeat_period: Duration::from_millis(180),
		}
	}
}



/// Enum that is checked by the `Endpoint` to decide what to do in the event the
/// user creates a new `SetSender` but the endpoint itself has written-but-unsent
/// data.
/// 1. `Panic` will panic if the event occurs.
/// 1. `Clear` will discard any unsent bytes.
/// 1. `IntoSet` will transfer the bytes into the first payload inside the set.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum NewSetUnsent {
	Panic,
	Clear,
	IntoSet,
}

////////////////////////////////////////////////////////////////////////////////



/// Enum that is passed into every send call on an `Endpoint` that creates and
/// ships a payload. The enum variant determines what bookkeeping will be performed
/// to deliver the expected guarantees.
/// 1. `None`. Message may arrive 0+ times and may arrive any any time.
/// 1. `Order`. Message may arrive 0-1 times but will respect ordering wrt. other `Order` and `Delivery` messages.
/// 1. `Delivery`. Message will arrive 1 time and will respect the ordering wrt. other `Order` and `Delivery` messages.
#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub enum Guarantee {
	None,
	Order,
	Delivery,
}

////////////////////////////////////////////////////////////////////////////////



/// This trait defines the payload-sending behaviour common to `Endpoint` and `SenderSet`.
pub trait Sender: io::Write {
	fn send_written(&mut self, Guarantee) -> io::Result<usize>;
	fn clear_written(&mut self);
	fn write_send(&mut self, data: &[u8], g: Guarantee) -> io::Result<(usize, usize)> {
		let a = self.write(data)?;
		Ok((a, self.send_written(g)?))
	}
	fn send_payload(&mut self, data: &[u8], g: Guarantee) -> io::Result<usize> {
		self.clear_written();
		self.write(data)?;
		self.send_written(g)
	}
}
