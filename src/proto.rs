
use rand::{thread_rng, Rng};
use std::collections::{HashMap, HashSet};
use byteorder::{ReadBytesExt, WriteBytesExt};
use std::io;
use std::fmt;
use std::net::UdpSocket;
use std::io::{
	Read,
	Write,
};
use std::iter;

use mod_ord::ModOrd;

/*
TODO
- organize imports
- log(n) means of scanning inbox
- io::Write interface for sending
- buffered written messages?
- acknowledgements
*/

#[derive(Debug, Clone, Copy)]
struct EndpointConfig {
	pub max_msg_size: usize,
	pub buffer_grow_space: usize,
}
impl EndpointConfig {

	fn default() -> Self {
		EndpointConfig {
			max_msg_size: 2048,
			buffer_grow_space: 1024,
		}
	}
}

type Socket = BadUdp;
#[derive(Debug)]
struct Endpoint {
	//both in and out
	config: EndpointConfig,
	socket: Socket,
	buf: Vec<u8>,
	buf_free_start: usize,

	//outgoing
	next_id: ModOrd,
	wait_until: ModOrd,

	//incoming
	n: ModOrd,
	largest_set_id_yielded: ModOrd,
	seen_before: HashSet<ModOrd>, // contains messages only in THIS set
	inbox: HashMap<ModOrd, Message>,
	inbox2: HashMap<ModOrd, OwnedMessage>, 
	inbox2_to_remove: Option<ModOrd>,
}

impl Endpoint {
	pub fn drop_my_ass(&mut self, count: u32, ord_count: u32) {
		self.next_id = self.next_id.new_plus(count);
		self.wait_until = self.next_id.new_minus(ord_count);
	}

	pub fn new_with_config(socket: Socket, config: EndpointConfig) -> Self {
		Endpoint {
			config,
			socket,
			// channel,
			buf: iter::repeat(0)
				.take(config.max_msg_size + config.buffer_grow_space)
				.collect(),
			buf_free_start: 0,
			next_id: ModOrd::ZERO,
			wait_until: ModOrd::ZERO,
			n: ModOrd::ZERO,
			largest_set_id_yielded: ModOrd::ZERO,
			inbox: HashMap::new(),
			inbox2: HashMap::new(),
			seen_before: HashSet::new(),
			inbox2_to_remove: None,
		}
	}

	pub fn new(socket: Socket) -> Self {
		Self::new_with_config(socket, EndpointConfig::default())
	}

	pub fn send(&mut self, guarantee: Guarantee, payload: &[u8]) -> io::Result<usize> {
		self.get_set().send(guarantee, payload)
	}

	fn pre_yield(&mut self, set_id: ModOrd, id: ModOrd, del: bool) {
		if set_id > self.largest_set_id_yielded {
			self.largest_set_id_yielded = set_id;
			self.seen_before.clear();
		}
		self.seen_before.insert(id);
		if self.n < set_id {
			self.n = set_id;
			println!("n set to set_id={:?}", self.n);
		}
		if del {
			self.n = self.n.new_plus(1);
			println!("incrementing n because del. now is {:?}", self.n);
		}
	}

	fn ready_from_inbox(&self) -> Option<ModOrd> {
		for (&id, msg) in self.inbox.iter() {
			if msg.h.wait_until <= self.n {
				return Some(id);
			}
		}
		None
	}

	fn ready_from_inbox2(&self) -> Option<ModOrd> {
		for (&id, msg) in self.inbox2.iter() {
			if msg.h.wait_until <= self.n {
				return Some(id);
			}
		}
		None
	}

	fn vacate_inbox1(&mut self) {
		println!("VACATING INBOX 1 --> INBOX 2...");
		let mut count = 0;
		for (id, msg) in self.inbox.drain() {
			let payload = unsafe{&*msg.payload}.to_vec();
			println!("- VACATING MESSAGE WITH ID {:?} ({} bytes)...", id, payload.len());
			let h = msg.h;
			let owned_msg = OwnedMessage {
				h, payload,
			};
			self.inbox2.insert(id, owned_msg);
			count += 1;
		}
		println!("VACATING COMPLETE. MOVED {} messages", count);
		self.buf_free_start = 0;
	}

	pub fn recv(&mut self) -> io::Result<&[u8]> {

		// first try in-line inbox
		if let Some(id) = self.ready_from_inbox() {
			let msg = self.inbox.remove(&id).unwrap();
			if self.inbox.is_empty() {
				println!("VACATING INTENTIONALLY (trivial)");
				self.vacate_inbox1();
			}
			self.pre_yield(msg.h.set_id, msg.h.id, msg.h.del);
			println!("yeilding from store 1...");
			return Ok(unsafe{&*msg.payload});
		}

		// remove from inbox2 as possible
		if let Some(id) = self.inbox2_to_remove {
			self.inbox2_to_remove = None;
			self.inbox2.remove(&id);
		} 

		// next try inbox2 (growing owned storage)
		if let Some(id) = self.ready_from_inbox2() {
			let (set_id, id, del) = {
				let msg = self.inbox2.get(&id).unwrap();
				(msg.h.set_id, msg.h.id, msg.h.del)
			};
			self.pre_yield(set_id, id, del);
			self.inbox2_to_remove = Some(id); // will remove later
			println!("yeilding from store 2...");
			return Ok(&self.inbox2.get(&id).unwrap().payload);
		}

		// otherwise try recv
		loop {
			// move messages from inbox1 to inbox2 to make space for a recv
			println!("SPACE IS {}", self.buf.len() - self.buf_free_start);
			if self.buf.len() - self.buf_free_start < self.config.max_msg_size {
				println!("HAVE TO VACATE");
				self.vacate_inbox1();
			}

			println!("recv loop..");
			match self.socket.recv(&mut self.buf[self.buf_free_start..]) {
				Ok(bytes) => {
					println!("udp datagram with {} bytes ({} of which are payload)", bytes, bytes-Header::BYTES);
					assert!(bytes >= Header::BYTES);
					let h_starts_at = self.buf_free_start + bytes - Header::BYTES;
					let msg = Message {
						h: Header::read_from(& self.buf[h_starts_at..])?,
						payload: (&self.buf[self.buf_free_start..h_starts_at]) as *const [u8],
					};
					self.buf_free_start = h_starts_at; // move the buffer right
					println!("buf starts at {} now...", self.buf_free_start);

					println!("\n::: channel sent {:?}", &msg);
					if msg.h.id.special() {
						println!("NO SEQ NUM");
						return Ok(unsafe{&*msg.payload})
					} else if msg.h.set_id < self.largest_set_id_yielded {
						println!("TOO OLD");
					} else if msg.h.wait_until > self.n {
						println!("NOT YET");
						if !self.inbox.contains_key(&msg.h.id) {
							println!("STORING");
							self.inbox.insert(msg.h.id, msg);
						}
					} else if self.seen_before.contains(&msg.h.id) {
						println!("seen before!");
					} else {
						self.pre_yield(msg.h.set_id, msg.h.id, msg.h.del);
						println!("yeilding in-place...");
						return Ok(unsafe{&*msg.payload})
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

	let socket = BadUdp::new();
	let mut config = EndpointConfig::default();
	config.max_msg_size = 32;
	config.buffer_grow_space = 16;

	println!("YAY");
	let mut e = Endpoint::new_with_config(socket, config);
	e.send(Guarantee::Delivery, b"thats a lotta damage");
	e.as_set(|mut s| {
		s.send(Guarantee::Delivery, b"1a");
		s.send(Guarantee::Order, b"1b");
	});
	e.as_set(|mut s| {
		s.send(Guarantee::Delivery, b"2a");
		s.send(Guarantee::Order, b"2b");
	});
	for letter in ('a' as u8)..=('g' as u8) {
		e.send(Guarantee::Delivery, &vec![letter]);
	}

	let mut got = vec![];
	while let Ok(msg) = e.recv() {
		let out: String = String::from_utf8_lossy(&msg[..]).to_string();
		println!("--> yielded: {:?}\n", &out);
		got.push(out);
	}
	println!("got: {:?}", got);

	println!("E {:#?}", e);
}

#[derive(Clone)]
struct Message {
	h: Header,
	payload: *const [u8],
}
impl fmt::Debug for Message {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		write!(f, "Inbox1Msg {:?} payload ~= {:?}",
			self.h,
			String::from_utf8_lossy(unsafe{&*self.payload}),
		)
	}
}

#[derive(Debug, Clone)]
struct OwnedMessage {
	h: Header,
	payload: Vec<u8>,
}

#[derive(Debug)]
struct BadUdp {
	messages: Vec<Vec<u8>>,
}

impl BadUdp {
	fn new() -> Self {
		BadUdp {
			messages: vec![],
		}
	}

	fn send(&mut self, buf: &[u8]) -> io::Result<usize> {
		let m = buf.to_vec();
		self.messages.push(m.clone());
		self.messages.push(m);
		Ok(buf.len())
	}

	fn recv(&mut self, mut buf: &mut [u8]) -> io::Result<usize> {
		if self.messages.is_empty() {
			Err(io::ErrorKind::WouldBlock.into())
		} else {
			let i = thread_rng().gen_range(0, self.messages.len());
			let m = self.messages.remove(i);
			buf.write(&m)
		}
	}
}