
use rand::{thread_rng, Rng};
use std::collections::{HashMap, HashSet};
use byteorder::{ReadBytesExt, WriteBytesExt};
use std::io;
use std::net::UdpSocket;
use std::io::{
	Read,
	Write,
};
use std::iter;

use mod_ord::ModOrd;

use slab::Slab;

#[derive(Debug)]
struct Inbox {
	slab: Slab<Message>,
	by_id: HashMap<ModOrd, usize>,
	by_until: HashMap<ModOrd, HashSet<usize>>,
}
impl Inbox {
	pub fn new() -> Self {
		Self {
			slab: Slab::new(),
			by_id: HashMap::new(),
			by_until: HashMap::new(),
		}
	}
	pub fn got_already(&self, id: ModOrd) -> bool {
		self.by_id.contains_key(&id)
	}
	pub fn store(&mut self, msg: Message) {
		if !self.got_already(msg.h.id) {
			let key = self.slab.insert(msg);
			let msg_ref = self.slab.get(key).unwrap();
			self.by_id.insert(msg_ref.h.id, key);
			let s = self.by_until.entry(msg_ref.h.wait_until)
				.or_insert_with(|| HashSet::new());
			s.insert(key);
		}
	}

	pub fn remove(&mut self, id: ModOrd) {
		if !self.by_id.contains_key(&id) {
			return;
		}
		let key: usize = *self.by_id.get(&id).unwrap();
		{
			let msg_ref = self.slab.get(key).unwrap();
			self.by_id.remove(&msg_ref.h.id);
			if {
				let set = self.by_until.get_mut(&msg_ref.h.wait_until).unwrap();
				set.remove(&key);
				set.len() == 0
			} {
				self.by_until.remove(&msg_ref.h.wait_until);
			}
		}
		self.slab.remove(key);
	}

	pub fn try_fetch(&mut self, n: ModOrd) -> Option<Message> {
		for (&until, mut key_set) in self.by_until.iter_mut() {
			if until <= n {
				for key in key_set.iter().cloned() {
					return Some(self.slab.get(key).unwrap().clone()) // TODO
				}
			}
		}
		None
	}
}

#[derive(Debug)]
struct Endpoint {
	socket: UdpSocket,
	channel: Channel,
	next_id: ModOrd,
	wait_until: ModOrd,
	buf: Vec<u8>,
	buf_free_start: usize,


	n: ModOrd,
	largest_set_id_yielded: ModOrd,
	inbox: Inbox,
	seen_before: HashSet<ModOrd>,
	to_remove: Option<ModOrd>, // remove from store
}

impl Endpoint {
	pub fn drop_my_ass(&mut self, count: u32, ord_count: u32) {
		self.next_id = self.next_id.new_plus(count);
		self.wait_until = self.next_id.new_minus(ord_count);
	}

	pub fn new(socket: UdpSocket, channel: Channel) -> Self {
		Endpoint {
			socket,
			channel,
			buf: iter::repeat(0).take(2048).collect(),
			buf_free_start: 0,
			next_id: ModOrd::ZERO,
			wait_until: ModOrd::ZERO,
			n: ModOrd::ZERO,
			largest_set_id_yielded: ModOrd::ZERO,
			inbox: Inbox::new(),
			seen_before: HashSet::new(),
			to_remove: None,
		}
	}

	pub fn send(&mut self, guarantee: Guarantee, payload: &[u8]) -> io::Result<usize> {
		self.get_set().send(guarantee, payload)
	}

	fn pre_yield(&mut self, h: &Header) {
		if h.set_id > self.largest_set_id_yielded {
			self.largest_set_id_yielded = h.set_id;
			self.seen_before.clear();
		}
		self.seen_before.insert(h.id);
		if self.n < h.set_id {
			self.n = h.set_id;
			println!("n set to set_id={:?}", self.n);
		}
		if h.del {
			self.n = self.n.new_plus(1);
			println!("incrementing n because del. now is {:?}", self.n);
		}
	}

	pub fn recv(&mut self) -> io::Result<Vec<u8>> {
		// resolve to_remove
		if let Some(r) = self.to_remove {
			self.inbox.remove(r);
		}

		// first try inbox
		if let Some(msg) = self.inbox.try_fetch(self.n) {
			self.to_remove = Some(msg.h.id);
			self.pre_yield(&msg.h);
			println!("yeilding from store...");
			return Ok(msg.payload);
		}

		// otherwise try recv
		loop {
			println!("recv loop..");
			match self.socket.recv(&mut self.buf[self.buf_free_start..]) {
				Ok(bytes) => {
					println!("udp datagram with {} bytes ({} of which are payload)", bytes, bytes-Header::BYTES);
					assert!(bytes >= Header::BYTES);
					let h_starts_at = self.buf_free_start + bytes - Header::BYTES;
					let msg = Message {
						h: Header::read_from(& self.buf[h_starts_at..])?,
						payload: self.buf[self.buf_free_start..h_starts_at].to_vec(),
					};
					self.buf_free_start = h_starts_at; // move the buffer right
					println!("buf starts at {} now...", self.buf_free_start);

					println!("\n::: channel sent {:?}", &msg);
					if msg.h.id.special() {
						println!("NO SEQ NUM");
						return Ok(msg.payload)
					} else if msg.h.set_id < self.largest_set_id_yielded {
						println!("TOO OLD");
					} else if msg.h.wait_until > self.n {
						println!("NOT YET");
						if !self.inbox.got_already(msg.h.id) {
							println!("STORING");
							self.inbox.store(msg);
						}
					} else if self.seen_before.contains(&msg.h.id) {
						println!("seen before!");
					} else {
						self.pre_yield(&msg.h);
						println!("yeilding in-place...");
						return Ok(msg.payload)
					}
				},
				Err(e) => {
					return Err(e);
				}
			}
		}	
	}

	pub fn as_set<F,R>(&mut self, work: F) -> R
	where
		F: Sized + FnOnce(X) -> R,
		R: Sized,
	{
		work(self.get_set())
	}

	pub fn get_set(&mut self) -> X {
		let set_id = self.next_id;
		X::new(
			self,
			set_id,
		)
	}
}

////////////////////

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
enum Guarantee {
	None,
	Order,
	Delivery,
}

//////////////////////////

#[derive(Debug)]
struct X<'a> {
	endpoint: &'a mut Endpoint,
	set_id: ModOrd,
	count: u32,
	ord_count: u32,
}

impl<'a> X<'a> {

	fn new(endpoint: &mut Endpoint, set_id: ModOrd) -> X {
		X {
			endpoint,
			set_id,
			count: 0,
			ord_count: 0,
		}
	}

	fn send(&mut self, guarantee: Guarantee, payload: &[u8]) -> io::Result<usize> {
		let id = if guarantee == Guarantee::None {
			ModOrd::SPECIAL
		} else {
			self.set_id.new_plus(self.count)
		};
		let header = Header {
			set_id: self.set_id,
			id,
			wait_until: self.endpoint.wait_until,
			del: guarantee == Guarantee::Delivery,
		};

		let buf_offset = self.endpoint.buf_free_start;
		let bytes_sent: usize = {
			let mut w = &mut self.endpoint.buf[buf_offset..];
			let len = w.write(payload)?;
			header.write_to(w)?;
			len + Header::BYTES
		};
		self.endpoint.socket.send(
			& self.endpoint.buf[buf_offset..(buf_offset+bytes_sent)]
		)?;

		println!("sending with g:{:?}. header is {:?}", guarantee, &header);
		if guarantee != Guarantee::None {
			self.count += 1;
			if guarantee != Guarantee::Delivery {
				self.ord_count += 1;
			}
		}
		// self.endpoint.channel.send(
		// 	Message {
		// 		h: header,
		// 		payload: payload.to_owned(),
		// 	}
		// );
		Ok(bytes_sent)
	}
}

impl<'a> Drop for X<'a> {
    fn drop(&mut self) {
        println!("Dropping!");
        self.endpoint.drop_my_ass(self.count, self.ord_count)
    }
}

/////////////////////////////////

#[derive(Debug, Clone)]
struct Header {
	id: ModOrd,
	set_id: ModOrd,
	wait_until: ModOrd,
	del: bool,
}
impl Header {
	const BYTES: usize = 4 + 4 + 4 + 1;

	fn write_to<W: io::Write>(&self, mut w: W) -> io::Result<()> {
		self.id.write_to(&mut w)?;
		self.set_id.write_to(&mut w)?;
		self.wait_until.write_to(&mut w)?;
		w.write_u8(if self.del {0x01} else {0x00})?;
		Ok(())
	}

	fn read_from<R: io::Read>(mut r: R) -> io::Result<Self> {
		Ok(Header {
			id: ModOrd::read_from(&mut r)?,
			set_id: ModOrd::read_from(&mut r)?,
			wait_until: ModOrd::read_from(&mut r)?,
			del: r.read_u8()? == 0x01,
		})
	}
}

#[test]
fn zoop() {
	let channel = Channel::new();
	let socket = UdpSocket::bind("127.0.0.1:8888")
	.expect("Failed to bind!");
	socket.set_nonblocking(true).unwrap();
	socket.connect("127.0.0.1:8888").unwrap();

	println!("YAY");
	let mut e = Endpoint::new(socket, channel);
	e.send(Guarantee::Delivery, b"thats a lotta damage");
	e.as_set(|mut s| {
		s.send(Guarantee::Delivery, b"1a");
		s.send(Guarantee::Order, b"1b");
	});
	e.as_set(|mut s| {
		s.send(Guarantee::Delivery, b"2a");
		s.send(Guarantee::Order, b"2b");
	});

	let mut got = vec![];
	while let Ok(msg) = e.recv() {
		println!("\n --> yielded: {:?}", String::from_utf8_lossy(&msg[..]));
		got.push(msg);
	}
	println!("got:");
	for (i, g) in got.iter().enumerate() {
		println!("  {}:\t{}", i, String::from_utf8_lossy(&g[..]));
	}
}

#[derive(Debug, Clone)]
struct Message {
	h: Header,
	payload: Vec<u8>,
}

#[derive(Debug)]
struct Channel {
	messages: Vec<Message>,
}
impl Channel {
	fn new() -> Self {
		Self {
			messages: vec![],
		}
	}
	fn send(&mut self, message: Message) {
		self.messages.push(message.clone());
		self.messages.push(message);
	}
	fn recv(&mut self) -> Option<Message> {
		if self.messages.len() == 0 {
			None
		} else {
			let i = thread_rng().gen_range(0, self.messages.len());
			let x = self.messages.remove(i);
			Some(x)
		}
	}
}