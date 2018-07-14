
use rand::{thread_rng, Rng};
use std::collections::{HashMap, HashSet};

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
			let mut s = self.by_until.entry(msg_ref.h.wait_until)
			.or_insert_with(|| HashSet::new());
			s.insert(key);
		}
	}
	pub fn try_fetch(&mut self, n: usize) -> Option<Message> {
		let mut found = None;
		'outer:
		for (&until, mut key_set) in self.by_until.iter_mut() {
			if until <= n {
				for key in key_set.iter().cloned() {
					found = Some(key);
					break 'outer;
				}
			}
		}
		if let Some(key) = found {
			{
				let msg_ref = self.slab.get(key).unwrap();
				self.by_id.remove(&msg_ref.h.id);
				if {
					let mut set = self.by_until.get_mut(&msg_ref.h.wait_until).unwrap();
					set.remove(&key);
					set.len() == 0
				} {
					self.by_until.remove(&msg_ref.h.wait_until);
				}
			}
			Some(self.slab.remove(key))
		} else {
			None
		}
	}
}

#[derive(Debug)]
struct Endpoint {
	channel: Channel,
	next_id: ModOrd,
	wait_until: usize,


	n: usize,
	largest_set_id_yielded: usize,
	inbox: Inbox,
	seen_before: HashSet<ModOrd>,
}

impl Endpoint {
	pub fn drop_my_ass(&mut self, count: usize, ord_count: usize) {
		self.next_id += count;
		self.wait_until = self.next_id - ord_count;
	}

	pub fn new(channel: Channel) -> Self {
		Endpoint {
			channel,
			next_id: 0,
			wait_until: 0,
			n: 0,
			largest_set_id_yielded: 0,
			inbox: Inbox::new(),
			seen_before: HashSet::new(),
		}
	}

	pub fn send(&mut self, guarantee: Guarantee, payload: &str) {
		let mut t = self.get_x();
		t.send(guarantee, payload);
	}

	fn pre_yield(&mut self, msg: &Message) {
		if msg.h.set_id > self.largest_set_id_yielded {
			self.largest_set_id_yielded = msg.h.set_id;
			self.seen_before.clear();
		}
		self.seen_before.insert(msg.h.id);
		if self.n < msg.h.set_id {
			self.n = msg.h.set_id;
			println!("n set to set_id={}", self.n);
		}
		if msg.h.del {
			self.n += 1;
			println!("incrementing n because del. now is {}", self.n);
		}
	}

	pub fn recv(&mut self) -> Option<String> {
		// first try inbox
		if let Some(msg) = self.inbox.try_fetch(self.n) {
			self.pre_yield(&msg);
			println!("yeilding from store...");
			return Some(msg.payload);
		}

		// otherwise try recv
		while let Some(msg) = self.channel.recv() {
			println!("recv loop..");
			println!("\n::: channel sent {:?}", &msg);
			if msg.h.id == SPECIAL {
				println!("NO SEQ NUM");
				return Some(msg.payload)
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
				self.pre_yield(&msg);
				println!("yeilding in-place...");
				return Some(msg.payload)
			}
		}
		println!("SIMPLY NO MSG");
		None		
	}

	pub fn x_do<F,R>(&mut self, work: F) -> R
	where
		F: Sized + FnOnce(X) -> R,
		R: Sized,
	{
		work(self.get_x())
	}

	pub fn get_x(&mut self) -> X {
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
	Ord,
	Delivery,
}

//////////////////////////


type ModOrd = usize;

const SPECIAL: ModOrd = 0xFF_FF_FF_FF;

#[derive(Debug)]
struct X<'a> {
	endpoint: &'a mut Endpoint,
	set_id: ModOrd,
	count: usize,
	ord_count: usize,
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

	fn send(&mut self, guarantee: Guarantee, payload: &str) {
		let id = if guarantee == Guarantee::None {SPECIAL} else {self.set_id + self.count};
		let header = Header {
			set_id: self.set_id,
			id,
			wait_until: self.endpoint.wait_until,
			del: guarantee == Guarantee::Delivery,
		};
		println!("sending with g:{:?}. header is {:?}", guarantee, &header);
		if guarantee != Guarantee::None {
			self.count += 1;
			if guarantee != Guarantee::Delivery {
				self.ord_count += 1;
			}
		}
		self.endpoint.channel.send(
			Message {
				h: header,
				payload: payload.to_owned(),
			}
		);
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
	del: bool,
	set_id: ModOrd,
	wait_until: ModOrd,
}


#[test]
fn zoop() {
	let channel = Channel::new();

	println!("YAY");
	let mut e = Endpoint::new(channel);
	e.send(Guarantee::Delivery, "0a");
	e.x_do(|mut s| {
		s.send(Guarantee::Delivery, "1a");
		s.send(Guarantee::Delivery, "1b");
		s.send(Guarantee::Delivery, "1c");
		s.send(Guarantee::Delivery, "1d");
	});
	e.x_do(|mut s| {
		s.send(Guarantee::Ord, "2a");
		s.send(Guarantee::Ord, "2b");
		s.send(Guarantee::Ord, "2c");
		s.send(Guarantee::Ord, "2d");
	});

	let mut got = vec![];
	while let Some(msg) = e.recv() {
		println!("\n --> yielded: {:?}", &msg);
		got.push(msg);
	}
	println!("GOT {:?}", &got);


}

#[derive(Debug, Clone)]
struct Message {
	h: Header,
	payload: String,
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