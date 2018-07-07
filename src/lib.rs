extern crate crossbeam_channel;
extern crate byteorder;
// use std::io::Cursor;

use std::io::{
	Write,
	// Read,
};
use std::io;
use std::collections::HashMap;
use std::net::{SocketAddr, UdpSocket};
use std::iter;

mod mod_ord;
use mod_ord::ModOrd;

#[cfg(test)]
mod tests;

//////////////////////////////////////

#[derive(Copy, Clone, Debug)]
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
	peer: SocketAddr,
	out_buf: Vec<u8>,


	//receiving
	inbox: HashMap<ModOrd, Vec<u8>>,
	next_to_yield: ModOrd,
	next_msg: Vec<u8>,
	to_pop: Option<ModOrd>,
}

impl Endpoint {
	pub fn new(sock: UdpSocket, peer: SocketAddr) -> Self {
		Endpoint {
			sock, peer,
			inbox: HashMap::new(),
			out_buf: vec![],
			next_to_yield: ModOrd::new(),
			next_to_send: ModOrd::new(),
			next_msg: iter::repeat(0).take(512).collect(),
			to_pop: None,
		}
	}

	pub fn sender_do<'a,F>(&'a mut self, g: Guarantee, mut work: F) -> Result<(),io::Error>
	where F: FnMut(Writeymabob<'a>)->Result<(),io::Error> + Sized {
		let sender = self.sender(g);
		work(sender)
	}

	pub fn sender(&mut self, g: Guarantee) -> Writeymabob {
		Writeymabob {
			guarantee: g,
			buf: &mut self.out_buf,
			sock: &mut self.sock,
		}
		// let mut vec = vec![];
		// self.next_to_send.write_to(&mut vec).unwrap();
		// self.next_to_send = self.next_to_send.new_plus(1);
		// vec.write(msg);
		// self.sock.send_to(&vec[..], self.peer).map(|_r| ())
	}

	fn read_incoming(&mut self) -> Result<(), io::Error> {

		loop {
			println!("loop..");
			match self.sock.recv_from(&mut self.next_msg[0..]) {
				Ok((len, _src_addr)) => {
					let seq = ModOrd::read_from(&self.next_msg[..4]).unwrap();
					println!("seq {:?}. payload ", &self.next_msg[4..len]);
					if !self.inbox.contains_key(&seq) {
						self.inbox.insert(seq, self.next_msg[4..len].to_vec());
					}
					//TODO insert into map here
					
				},
				Err(e) => {
					break;
					// [r]
					// println!("ERR read inc {:?}", &e);
					// return Err(e)
				},
			}
		}
		Ok(())
	}

	pub fn try_recv(&mut self) -> Result<Option<&[u8]>, io::Error> {
		// 1. pop if there is something to pop
		self.to_pop.take().map(|t| self.inbox.remove(&t));

		// 2. receive incoming messages
		self.read_incoming()?;

		// 2. check if next message is already ready
		if let Some(vec) = self.inbox.get(& self.next_to_yield) {
			self.to_pop = Some(self.next_to_yield);
			self.next_to_yield = self.next_to_yield.new_plus(1);
			Ok(Some(&vec[..]))
		} else {
			Ok(None)
		}
	}

	pub fn recv(&mut self) -> &[u8] {
		unimplemented!()
	}
}

pub struct Writeymabob<'a> {
	guarantee: Guarantee,
	sock: &'a mut UdpSocket,
	buf: &'a mut Vec<u8>,
}

impl<'a> Writeymabob<'a> {

}
impl<'a> io::Write for Writeymabob<'a> {
	fn write(&mut self, bytes: &[u8]) -> Result<usize, io::Error> {
		unimplemented!()
	}

	fn flush(&mut self) -> Result<(), io::Error> {
		unimplemented!()
	}
}

