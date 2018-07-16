
use std::io;
use std::time::Duration;
use resend_predicates;

pub trait UdpLike: Sized {
	fn send(&mut self, buf: &[u8]) -> io::Result<usize>;
	fn recv(&mut self, buf: &mut [u8]) -> io::Result<usize>;
}

////////////////////////////////////////////////////////////////////////////////

#[derive(Derivative)]
#[derivative(Debug)]
pub struct EndpointConfig {
	pub max_msg_size: usize,
	pub buffer_grow_space: usize,
	pub window_size: u32,
	pub new_set_unsent_action: NewSetUnsent,

	#[derivative(Debug="ignore")]
	pub resend_predicate: Box<FnMut(u32, Duration) -> bool>,
}
impl EndpointConfig {
	pub fn default() -> Self {
		EndpointConfig {
			max_msg_size: 2048,
			buffer_grow_space: 1024,
			window_size: 64,
			new_set_unsent_action: NewSetUnsent::Panic,
			resend_predicate: Box::new(resend_predicates::medium_combination),
		}
	}
}

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum NewSetUnsent {
	Panic,
	Clear,
	IntoSet,
}

////////////////////////////////////////////////////////////////////////////////


#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub enum Guarantee {
	None,
	Order,
	Delivery,
}

////////////////////////////////////////////////////////////////////////////////

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
