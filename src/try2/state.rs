
use internal::*;

#[derive(Debug)]
struct State {
	max_yielded: ModOrd, // for acking
	time_last_acked: Instant,
	next_id: ModOrd,
	wait_until: ModOrd,
	peer_acked: ModOrd,
	n: ModOrd,
	largest_set_id_yielded: ModOrd,
	seen_before: HashSet<ModOrd>, // contains messages only in THIS set
	msg_box: MsgBox,
}

impl State {
	fn new() -> Self {
		let time_last_acked = Instant::now();
		State {
			next_id: ModOrd::ZERO,
			wait_until: ModOrd::ZERO,
			n: ModOrd::ZERO,
			largest_set_id_yielded: ModOrd::ZERO,
			max_yielded: ModOrd::BEFORE_ZERO,
			peer_acked: ModOrd::BEFORE_ZERO,
			time_last_acked,
			seen_before: HashSet::new(),
			msg_box: MsgBox::new(),
		}
	}

	pub fn drop_acknowledged(&mut self) {
		self.msg_box.drop_acknowledged();
	}

	#[inline]
	pub fn primary_elements(&self) {
		self.msg_box.primary_elements()
	}

	pub fn pop_inbox_ready(&mut self) -> Option<(ModOrd, &[u8])> {
		let n = self.n
		self.msg_box.pop_inbox_ready(n)
	}

	pub fn get_time_last_acked(&self) -> Instant {
		self.time_last_acked
	}

	pub fn set_time_last_acked(&mut self, to: Instant) {
		self.time_last_acked = to;
	}


	pub fn vacate_primary(&mut self) {
		self.msg_box.vacate_primary()
	}

	pub fn reject_incoming_msg(&self, header: &Header, config: &EndpointConfig) -> bool {
		// not too old
		// not outside of window
		// not in current set
		// not already stored in inbox
		unimplemented!()
	}

	pub fn inbox_store(&mut self, msg:(Header, PayloadRef)) {
		self.msg_box.inbox_store(msg)
	}

	pub fn outbox_store(&mut self, id: ModOrd, msg:(LastSent, PayloadRef)) {
		self.msg_box.outbox_store(id, msg)
	}

	pub fn new_header(&mut self, g: Guarantee) -> Header {
		
	}
}