type Payload = Vec<u8>;
type PayloadRef = *mut [u8];
type LastSent = Instant;

struct PayloadRef(*mut [u8]);

impl PayloadRef {
	fn to_owned(self) -> Payload { unsafe{&*msg.1}.to_vec() }
}
struct MessageStorage {
	inbox1: HashMap<ModOrd, (Header, PayloadRef)>,
	inbox2: HashMap<ModOrd, (Header, Payload)>,
	outbox1: HashMap<ModOrd, (LastSent, PayloadRef)>,
	outbox2: HashMap<ModOrd, (LastSent, Payload)>,

	to_remove: Option<(ModOrd, bool)>, // true iff INBOX
}

impl MsgBox {
	pub fn new() -> MsgBox {
		MsgBox {
			inbox1: HashMap::new(),
			inbox2: HashMap::new(),
			outbox1: HashMap::new(),
			outbox2: HashMap::new(),
		}
	},

	// move *1 -> *2 for * in {inbox, outbox}
	pub fn vacate_primary(&mut self) {
		self.inbox1.drain().map(
			|id, (h, payload_ref)|
			self.inbox2.insert(id, (h, payload_ref.to_owned()))
		);
		self.outbox1.drain().map(
			|id, (l, payload_ref)|
			self.outbox2.insert(id, (l, payload_ref.to_owned()))
		);
	}

	pub fn drop_acknowledged(&mut self, ack_to: ModOrd) {
		outbox1.retain(|&id, _| id > ack_to);
		outbox2.retain(|&id, _| id > ack_to);
	}

	pub fn pop_inbox_ready(&mut self, n: ModOrd) -> Option<(ModOrd, &[u8])> {
		self.do_remove();
		if let Some(id) = self.ready_from_inbox1(n) {
			let p = unsafe{&*self.inbox1.remove(&id).unwrap().1};
			Some((id, p))
		} else if let Some(id) = self.ready_from_inbox2(n) {
			self.to_remove = (id, true);
			let p = & self.inbox2.get(&id).1[..];
			Some((id, p))
		} else {
			None
		}
	}

//////////////////////// PRIVATE /////////////////////
	fn ready_from_inbox1(&self, n: ModOrd) -> Option<ModOrd> {
		for (&id, msg) in self.inbox1.iter() {
			if msg.0.wait_until <= n {
				return Some(id);
			}
		}
		None
	}

	fn ready_from_inbox2(&self, n: ModOrd) -> Option<ModOrd> {
		for (&id, msg) in self.inbox2.iter() {
			if msg.0.wait_until <= n {
				return Some(id);
			}
		}
		None
	}

	fn do_remove(&mut self) {
		if let Some((id, is_inbox)) = self.to_remove {
			if is_inbox {
				self.inbox2.remove(&id).unwrap();
			} else {
				self.outbox2.remove(&id).unwrap();
			}
			self.to_remove = None;
		}
	}
}
