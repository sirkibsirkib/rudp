#![allow(dead_code)] //////////// REMOVE REMOVE DEBUG DEBUG TODO TODO

use helper::*;
use std::{
	collections::{HashMap, HashSet},
	io, fmt, time, iter, cmp,
	time::{
		Instant,
		Duration,
	},
	// io::{
	// 	Write,
	// },
};
use byteorder::{ReadBytesExt, WriteBytesExt};
use mod_ord::ModOrd;


#[derive(Debug)]
pub struct Endpoint<U: UdpLike> {
	//both in and out
	socket: U,
	buf: Vec<u8>,
	buf_free_start: usize,
	max_yielded: ModOrd, // for acking
	time_last_acked: Instant,


	//outgoing
	next_id: ModOrd,
	wait_until: ModOrd,
	 // only stores delivery messages
	outbox: HashMap<ModOrd, (Instant, *const [u8])>,
	outbox2: HashMap<ModOrd, (Instant, Vec<u8>)>,
	peer_acked: ModOrd,
	out_buf_written: usize,

	//incoming
	buf_min_space: usize,
	n: ModOrd,
	largest_set_id_yielded: ModOrd,
	seen_before: HashSet<ModOrd>, // contains messages only in THIS set
	inbox: HashMap<ModOrd, Message>,
	inbox2: HashMap<ModOrd, OwnedMessage>, 
	inbox2_to_remove: Option<ModOrd>,
	window_size: u32,
}


impl<U> Endpoint<U> where U: UdpLike {
///// PUBLIC
	pub fn resend_lost(&mut self) -> io::Result<()> {
		let a = self.peer_acked;
		self.outbox.retain(|&id, _| id > a);
		self.outbox2.retain(|&id, _| id > a);
		let now = Instant::now();
		for (id, (ref mut instant, ref_bytes)) in self.outbox.iter_mut() {
			if Self::too_stale_func(id.abs_difference(self.n), instant.elapsed()) {
				//resend
				self.socket.send(unsafe{&**ref_bytes})?;
				*instant = now;
			}
		}
		for (id, (ref mut instant, ref vec)) in self.outbox2.iter_mut() {
			if Self::too_stale_func(id.abs_difference(self.n), instant.elapsed()) {
				//resend
				self.socket.send(&vec[..])?;
				*instant = now;
			}
		}
		self.maybe_ack()?;
		Ok(())
	}

	pub fn new_with_config(socket: U, config: EndpointConfig) -> Endpoint<U> {
		Endpoint {
			buf_min_space: config.max_msg_size + Header::BYTES,
			socket,
			buf: iter::repeat(0)
				.take(config.max_msg_size + config.buffer_grow_space + Header::BYTES)
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
			max_yielded: ModOrd::BEFORE_ZERO,
			outbox: HashMap::new(),
			outbox2: HashMap::new(),
			peer_acked: ModOrd::BEFORE_ZERO,
			window_size: config.window_size,
			time_last_acked: Instant::now(),
			out_buf_written: 0,
		}
	}

	pub fn new(socket: U) -> Endpoint<U> {
		Self::new_with_config(socket, EndpointConfig::default())
	}

	// pub fn write_send(&mut self, bytes: &[u8], guarantee: Guarantee) -> io::Result<usize> {
	// 	self.write(bytes)?;
	// 	self.send(guarantee)
	// }

	// pub fn write_all<R: io::Read>(&mut self, mut r: R) -> io::Result<usize> {
	// 	let mut tot = 0; 
	// 	loop {
	// 		match r.read(&mut self.buf[(self.buf_free_start+self.out_buf_written)..]) {
	// 			Ok(0) => return Ok(tot),
	// 			Ok(x) => {
	// 				tot += x;
	// 				self.out_buf_written += x;
	// 			},
	// 			Err(e) => return Err(e),
	// 		}
	// 	}
	// }

	pub fn recv(&mut self) -> io::Result<&[u8]> {
		println!("largest_set_id_yielded {:?}", self.largest_set_id_yielded);
		{
			let intersect: HashSet<ModOrd> = self.inbox.keys().cloned().collect::<HashSet<_>>();
			let wang = self.inbox2.keys().cloned().collect::<HashSet<_>>();
			let i2 = intersect.intersection(
				& wang
			);
			println!("intersection {:?}", &i2);
			assert!(i2.count() == 0);
		}

		// first try in-line inbox
		if let Some(id) = self.ready_from_inbox() {
			let msg = self.inbox.remove(&id).unwrap();
			println!("getting id {:?} from inbox1 with {:?}", id, &msg.h);
			if self.inbox.is_empty() && self.outbox.is_empty() {
				println!("VACATING INTENTIONALLY (trivial)");
				self.vacate_buffer();
			}
			self.pre_yield(msg.h.set_id, msg.h.id, msg.h.del);
			println!("yeilding from store 1...");
			self.maybe_ack()?;
			return Ok(unsafe{&*msg.payload});
		}

		// remove from inbox2 as possible
		if let Some(id) = self.inbox2_to_remove {
			println!("getting id {:?} from inbox2", id);
			println!("removing from inbox2 {:?}", id);
			self.inbox2.remove(&id);
			self.inbox2_to_remove = None;
		} 

		// next try inbox2 (growing owned storage)
		if let Some(id) = self.ready_from_inbox2() {
			let (set_id, id, del) = {
				let msg = self.inbox2.get(&id).unwrap();
				println!("getting id {:?} from inbox2 with {:?}", id, &msg.h);
				(msg.h.set_id, msg.h.id, msg.h.del)
			};
			self.pre_yield(set_id, id, del);
			self.inbox2_to_remove = Some(id); // will remove later
			println!("yeilding from store 2...");
			self.maybe_ack()?;
			return Ok(&self.inbox2.get(&id).unwrap().payload);
		}

		// nothing ready from the inbox. receive messages until we can yield
		loop {
			println!("largest_set_id_yielded {:?}", self.largest_set_id_yielded);
			println!("SPACE IS {}", self.buf.len() - self.buf_free_start);
			if self.buf_cant_take_another() {
			// move messages from inbox1 to inbox2 to make space for a recv
				println!("HAVE TO VACATE");
				self.vacate_buffer();
			}

			println!("recv loop..");
			match self.socket.recv(&mut self.buf[self.buf_free_start..]) {
				Ok(ModOrd::BYTES) => {
					// got an ACK-ONLY field
					println!("GOT ACK ONLY MSG");
					let ack = ModOrd::read_from(& self.buf[self.buf_free_start..(self.buf_free_start+ModOrd::BYTES)]).unwrap();
					self.digest_incoming_ack(ack);
				},
				Ok(bytes) => {
					println!("udp datagram with {} bytes ({} of which are payload)", bytes, bytes-Header::BYTES);
					assert!(bytes >= Header::BYTES);
					let h_starts_at = self.buf_free_start + bytes - Header::BYTES;
					let h = Header::read_from(& self.buf[h_starts_at..])?;
					self.digest_incoming_ack(h.ack);

					if self.largest_set_id_yielded.abs_difference(h.set_id) > self.window_size {
						println!("OUTSIDE OF WINDOW");
						continue;
					}

					if self.known_duplicate(&h) {
						println!("DROPPING KNOWN DUPLICATE {:?}", h);
						continue;
					}
					let msg = Message {
						h,
						payload: (&self.buf[self.buf_free_start..h_starts_at]) as *const [u8],
					};
					self.buf_free_start = h_starts_at; // move the buffer right
					println!("buf starts at {} now...", self.buf_free_start);

					println!("\n::: channel sent {:?}", &msg);
					if msg.h.id.special() {
						println!("NO SEQ NUM");
						self.maybe_ack()?;
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
						self.maybe_ack()?;
						return Ok(unsafe{&*msg.payload})
					}
				},
				Err(e) => {
					let _ = self.maybe_ack();
					return Err(e);
				}
			}
		}	
	}

	pub fn as_set<F,R>(&mut self, work: F) -> R
	where
		F: Sized + FnOnce(SetSender<U>) -> R,
		R: Sized,
	{
		work(self.get_set())
	}

	pub fn get_set(&mut self) -> SetSender<U> {
		let set_id = self.next_id;
		SetSender::new(self, set_id)
	}

////////////// PRIVATE
	fn pre_yield(&mut self, set_id: ModOrd, id: ModOrd, del: bool) {
		if set_id > self.largest_set_id_yielded {
			self.largest_set_id_yielded = set_id;
			self.seen_before.clear();
			println!("Clearing `seen before`");
		}
		println!("seen before set is {:?}", &self.seen_before);
		self.seen_before.insert(id);
		if self.n < set_id {
			self.n = set_id;
			println!("n set to set_id={:?}", self.n);
		}
		if del {
			self.n = self.n.new_plus(1);
			println!("incrementing n because del. now is {:?}", self.n);
		}
		if self.max_yielded < id {
			self.max_yielded = id;
		}
	}

	fn maybe_ack(&mut self) -> io::Result<()> {
		let now = Instant::now();
		if self.time_last_acked.elapsed() > Duration::from_millis(300) {
			let b = self.buf_free_start;
			self.largest_set_id_yielded.write_to(&mut self.buf[b..])?;
			self.socket.send(&self.buf[b..(b+ModOrd::BYTES)])?;
			self.time_last_acked = now;
		}
		Ok(())
	}

	fn too_stale_func(seq_difference: u32, age: time::Duration) -> bool {
		if age.as_secs() > 0 {
			true
		} else {
			let x = age.subsec_millis() + seq_difference * 8;
			// println!("staleness {}", x);
			x > 1000 
		}
	}

	fn ready_from_inbox(&self) -> Option<ModOrd> {
		for (&id, msg) in self.inbox.iter() {
			println!("inbox1:: visiting id {:?}", id);
			if msg.h.wait_until <= self.n {
				return Some(id);
			}
		}
		None
	}

	fn ready_from_inbox2(&self) -> Option<ModOrd> {
		for (&id, msg) in self.inbox2.iter() {
			println!("inbox2:: visiting id2 {:?}", id);
			if msg.h.wait_until <= self.n {
				return Some(id);
			}
		}
		None
	}

	fn vacate_buffer(&mut self) {
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
		assert!(self.inbox.is_empty());
		println!("VACATING COMPLETE. MOVED {} INBOX messages", count);

		println!("VACATING OUTBOX 1 --> OUTBOX 2...");
		let mut count = 0;
		for (id, (instant, bytes)) in self.outbox.drain() {
			let vec = unsafe{&*bytes}.to_vec();
			println!("- VACATING MESSAGE WITH ID {:?} ({} bytes)...", id, vec.len());
			self.outbox2.insert(id, (instant, vec));
			count += 1;
		}
		assert!(self.inbox.is_empty());
		println!("VACATING COMPLETE. MOVED {} OUTBOX messages", count);
		self.buf_free_start = 0;
	}

	fn known_duplicate(&mut self, header: &Header) -> bool {
		let id = header.id;
		println!("known?? {} {} {}", self.seen_before.contains(&id), self.inbox.contains_key(&id), self.inbox2.contains_key(&id));
	  	self.seen_before.contains(&id)
		|| self.inbox.contains_key(&id)
		|| self.inbox2.contains_key(&id) 
	}

	fn buf_cant_take_another(&self) -> bool {
		self.buf.len() - self.buf_free_start < self.buf_min_space
	}

	fn digest_incoming_ack(&mut self, ack: ModOrd) {
		if self.peer_acked < ack {
			self.peer_acked = ack;
			println!("peer ack is now {:?}", self.peer_acked);
		}
	}


	fn drop_my_ass(&mut self, count: u32, ord_count: u32) {
		if count == 0 {
			// no need to waste a perfectly good sequence number
			return;
		}
		self.next_id = self.next_id.new_plus(count);
		println!("ORD CNT {} CNT {}", ord_count, count);
		if ord_count < count {
			self.wait_until = self.next_id.new_minus(ord_count);
		}
	}
}



impl<U> Sender for Endpoint<U> where U: UdpLike {
	fn send_written(&mut self, guarantee: Guarantee) -> io::Result<usize> {
		self.get_set().send_written(guarantee)
	}

	fn clear_written(&mut self) {
		self.out_buf_written = 0;
	}
}

impl<U> io::Write for Endpoint<U> where U: UdpLike {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
    	let b = (&mut self.buf[(self.buf_free_start + self.out_buf_written)..]).write(bytes)?;
    	self.out_buf_written += b;
    	Ok(b)
    }

    fn flush(&mut self) -> io::Result<()> {
    	Ok(())
    }
}

/////////////////////////////////

#[derive(Debug, Clone)]
struct Header {
	id: ModOrd,
	set_id: ModOrd,
	ack: ModOrd,
	wait_until: ModOrd,
	del: bool,
}
impl Header {
	const BYTES: usize = 4*4 + 1;

	fn write_to<W: io::Write>(&self, mut w: W) -> io::Result<()> {
		self.ack.write_to(&mut w)?;
		self.id.write_to(&mut w)?;
		self.set_id.write_to(&mut w)?;
		self.wait_until.write_to(&mut w)?;
		w.write_u8(if self.del {0x01} else {0x00})?;
		Ok(())
	}

	fn read_from<R: io::Read>(mut r: R) -> io::Result<Self> {
		Ok(Header {
			ack: ModOrd::read_from(&mut r)?,
			id: ModOrd::read_from(&mut r)?,
			set_id: ModOrd::read_from(&mut r)?,
			wait_until: ModOrd::read_from(&mut r)?,
			del: r.read_u8()? == 0x01,
		})
	}
}
impl cmp::PartialEq for Header {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}


////////////////////////////////////////////////////////////////////////////////


#[derive(Debug)]
pub struct SetSender<'a, U: UdpLike + 'a>{
	endpoint: &'a mut Endpoint<U>,
	set_id: ModOrd,
	count: u32,
	ord_count: u32,
}

impl<'a, U> SetSender<'a, U> where U: UdpLike + 'a {
	fn new(endpoint: &mut Endpoint<U>, set_id: ModOrd) -> SetSender<U> {
		SetSender {

			endpoint,
			set_id,
			count: 0,
			ord_count: 0,
		}
	}
}

impl<'a, U> Sender for SetSender<'a, U> where U: UdpLike + 'a {
	fn send_written(&mut self, guarantee: Guarantee) -> io::Result<usize> {
		if self.endpoint.buf_cant_take_another() {
			println!("VACATING BUFFER for SEND");
			self.endpoint.vacate_buffer();
		}
		let id = if guarantee == Guarantee::None {
			ModOrd::SPECIAL
		} else {
			self.set_id.new_plus(self.count)
		};
		let header = Header {
			ack: self.endpoint.max_yielded,
			set_id: self.set_id,
			id,
			wait_until: self.endpoint.wait_until,
			del: guarantee == Guarantee::Delivery,
		};

		let payload_end = self.endpoint.buf_free_start+self.endpoint.out_buf_written;
		header.write_to(&mut self.endpoint.buf[payload_end..])?;
		let bytes_sent = self.endpoint.out_buf_written + Header::BYTES;
		self.endpoint.out_buf_written = 0;
		let new_end = self.endpoint.buf_free_start + bytes_sent;
		let msg_slice = & self.endpoint.buf[self.endpoint.buf_free_start..new_end];
		self.endpoint.socket.send(msg_slice)?;

		if guarantee == Guarantee::Delivery {
			// save into outbox and bump the buffer up
			self.endpoint.outbox.insert(id, (Instant::now(), msg_slice as *const [u8]));
			self.endpoint.buf_free_start = new_end;
		}

		println!("sending with g:{:?}. header is {:?}", guarantee, &header);
		if guarantee != Guarantee::None {
			self.count += 1;
			if guarantee != Guarantee::Delivery {
				self.ord_count += 1;
			}
		}
		Ok(bytes_sent)
	}

	fn clear_written(&mut self) {
		self.endpoint.clear_written()
	}
}

impl<'a, U> Drop for SetSender<'a, U> where U: UdpLike {
    fn drop(&mut self) {
        println!("Dropping!");
        self.endpoint.drop_my_ass(self.count, self.ord_count)
    }
}

impl<'a, U> io::Write for SetSender<'a, U> where U: UdpLike {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
    	self.endpoint.write(bytes)
    }

    fn flush(&mut self) -> io::Result<()> {
    	Ok(())
    }
}

////////////////////////////////////////////////////////////////////////////////

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
impl cmp::PartialEq for Message {
    fn eq(&self, other: &Self) -> bool {
        self.h == other.h
    }
}


#[derive(Debug, Clone)]
struct OwnedMessage {
	h: Header,
	payload: Vec<u8>,
}
impl cmp::PartialEq for OwnedMessage {
    fn eq(&self, other: &Self) -> bool {
        self.h == other.h
    }
}

