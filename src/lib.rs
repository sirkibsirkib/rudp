extern crate crossbeam_channel;
extern crate byteorder;
// use std::io::Cursor;

use std::io::{
	Write,
	// Read,
};
use std::io;
use std::collections::{HashMap, HashSet};
use std::net::{SocketAddr, UdpSocket};
use std::iter;

mod mod_ord;
use mod_ord::ModOrd;

mod header;
use header::Header;

#[cfg(test)]
mod tests;

//////////////////////////////////////

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum Guarantee {
	None, // no guarantees. UDP behaviour
	Sequence(u8), // no dup + no reorder 
	Delivery(u8), // `Sequence` + no loss
}



const WINDOW: u32 = 10;
#[derive(Debug)]
pub struct Endpoint {
	sock: UdpSocket,

	//sending
	next_to_send: ModOrd,
	last_acked: ModOrd,
	peer: SocketAddr,
	out_buf: Vec<u8>,
	to_be_ackd: HashMap<ModOrd, Vec<u8>>,
	out_payload_len: usize,

	//receiving
	inbox: HashMap<ModOrd, HashSet<Vec<u8>>>,
	next_to_yield: ModOrd,
	next_msg: Vec<u8>,
	to_pop: Option<ModOrd>,
}

impl Endpoint {
	const RECV_LIMIT: usize = 32;

	pub fn new(sock: UdpSocket, peer: SocketAddr) -> Self {
		Endpoint {
			sock, peer,
			inbox: HashMap::new(),
			to_be_ackd: HashMap::new(),
			out_buf: iter::repeat(99).take(32).collect(),
			next_to_yield: ModOrd::ZERO,
			next_to_send: ModOrd::ZERO,
			last_acked: ModOrd::BEFORE_ZERO,
			next_msg: iter::repeat(99).take(32).collect(),
			to_pop: None,
			out_payload_len: 0,
		}
	}

	pub fn as_set_do<'a,T,F>(&'a mut self, mut work: F) -> T
	where
		F: FnMut(SetSender<'a>) -> T,
		T: Sized,
	{
		work(self.as_set())
	}

	pub fn as_set(&mut self) -> SetSender {
		SetSender {
			bytes: 0,
			endpt: self,
		}
	}

	fn may_yeild(&self, seq: ModOrd, seq_min: u16) -> bool {
		self.next_to_yield == seq
		|| seq == ModOrd::SPECIAL
	}

	pub fn store_into_inbox(&mut self, msg: Vec<u8>, seq: ModOrd, seq_min: u16) {
		//TODO
	}

	pub fn try_recv(&mut self) -> Result<Option<&[u8]>, io::Error> {
		for _ in 0..Self::RECV_LIMIT {
			println!("trying to recv...");
			match self.sock.recv_from(&mut self.next_msg[0..]) {
				Ok((len, src_addr)) => {
					println!("new msg from src_addr {:?}", src_addr);
					println!("read in (total) {} bytes", len);
					println!("in buf is {:?}", &self.next_msg[..]);
					let h = Header::read_from(&self.next_msg[..Header::LEN])
					.expect("BAD HEADER??");
					println!("HEADER: {:?}", h);
					println!("PAYLOAD: {:?}", &self.next_msg[Header::LEN..len]);
					if self.may_yeild(h.seq, h.seq_minor) {
						println!("YEILDING MESSAGE");
						return Ok(Some(&self.next_msg[Header::LEN..len]))
					} else {
						println!("MAY NOT YIELD");
						let v = self.next_msg[Header::LEN..len].to_vec();
						self.store_into_inbox(
							v,
							h.seq,
							h.seq_minor,
						);
					}
				},
				Err(e) => {
					println!("loop err {:?}", e.kind()); 
					if e.kind() == io::ErrorKind::WouldBlock {
						break;
					} else {
						return Err(e);
					}
				},
			}

		}
		Ok(None)
	}

	pub fn recv(&mut self) -> &[u8] {
		unimplemented!()
	}

	fn send_out(&mut self, bytes: usize, guarantee: Guarantee) -> io::Result<usize> {
		let seq = if guarantee == Guarantee::None {
			ModOrd::SPECIAL
		} else {
			let t = self.next_to_send;
			self.next_to_send = self.next_to_send.new_plus(1);
			t	
		};
		let h = Header {
			seq,
			seq_minor: 0,
			ack: self.last_acked,
			ack_minor: 0,
			seqs_since_del: 0,
			seqs_since_del_minor: 0,
		};
		h.write_to(&mut self.out_buf[..])?;

		if let Guarantee::Delivery(_) = guarantee {
			self.to_be_ackd.insert(
				self.next_to_send,
				self.out_buf[..(Header::LEN+(bytes as usize))].to_vec(),
			);
		}
		println!("out buf is {:?}", self.out_buf);
		println!("out payload len {:?}", bytes);
		self.sock.send_to(
			&self.out_buf[..(Header::LEN+(bytes as usize))],
			&self.peer,
		)
	}
}

pub struct SetSender<'a> {
	bytes: usize,
	endpt: &'a mut Endpoint, 
}

impl io::Write for Endpoint {
	fn write(&mut self, bytes: &[u8]) -> Result<usize, io::Error> {
		(&mut self.out_buf[Header::LEN + self.out_payload_len as usize..])
		.write(bytes)?;
		self.out_payload_len += bytes.len();
		Ok(bytes.len())
	}

	fn flush(&mut self) -> Result<(), io::Error> {
		Ok(())
	}
}


impl Sender for Endpoint {
	fn send(&mut self, g: Guarantee) -> io::Result<usize> {
		let b = self.out_payload_len;
		self.out_payload_len = 0;
		self.send_out(b, g)
	}
}

impl<'a> Sender for SetSender<'a> {
	fn send(&mut self, g: Guarantee) -> io::Result<usize> {
		let b = self.bytes;
		self.bytes = 0;
		self.endpt.send_out(b, g)
	}
}

impl<'a> io::Write for SetSender<'a> {
	fn write(&mut self, bytes: &[u8]) -> Result<usize, io::Error> {
		(&mut self.endpt.out_buf[Header::LEN + self.bytes as usize..]).write(bytes)?;
		self.bytes += bytes.len();
		Ok(bytes.len())
	}

	fn flush(&mut self) -> Result<(), io::Error> {
		Ok(())
	}
}

pub trait Sender: io::Write {
	fn send(&mut self, g: Guarantee) -> io::Result<usize>;
}
