
use std::io;
pub trait UdpLike: Sized {
	fn send(&mut self, buf: &[u8]) -> io::Result<usize>;
	fn recv(&mut self, buf: &mut [u8]) -> io::Result<usize>;
}

////////////////////////////////////////////////////////////////////////////////

pub struct EndpointConfig {
	pub max_msg_size: usize,
	pub buffer_grow_space: usize,
	pub window_size: u32,
	// pub must_resend_func: Box<FnMut(u32, time::Duration) -> bool>,
}
impl EndpointConfig {
	pub fn default() -> Self {
		EndpointConfig {
			max_msg_size: 2048,
			buffer_grow_space: 1024,
			window_size: 64,
		}
	}
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