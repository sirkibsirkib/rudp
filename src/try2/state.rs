
use std::time::Instant;
use mod_ord::ModOrd;
use try2::internal::*;
use std::collections::HashSet;
use try2::msg_box::*;
use helper::Guarantee;
use helper::EndpointConfig;
use std::net::SocketAddr;

#[derive(Debug)]
pub struct State {
	max_yielded: ModOrd, // for acking
	time_last_acked: Instant,
	next_id: ModOrd,
	wait_until: ModOrd,
	peer_acked: ModOrd,
	n: ModOrd,
	largest_set_id_yielded: ModOrd,
	seen_before: HashSet<ModOrd>, // contains messages only in THIS set
	msg_box: MsgBox,
	peer_addr: SocketAddr,
}

impl State {
	pub fn new(peer_addr: SocketAddr) -> Self {
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
			peer_addr,
		}
	}

	pub fn get_peer_addr(&self) -> &SocketAddr {
		&self.peer_addr
	}

	pub fn drop_acknowledged(&mut self) {
		self.msg_box.drop_acknowledged();
	}

	#[inline]
	pub fn primary_elements(&self) {
		self.msg_box.primary_elements()
	}

	pub fn pop_inbox_ready(&mut self) -> Option<(ModOrd, &[u8])> {
		let n = self.n;
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



	fn pre_yield(&mut self, h: &Header) {
		if h.set_id > self.largest_set_id_yielded {
			self.largest_set_id_yielded = h.set_id;
			self.seen_before.clear();
		}
		self.seen_before.insert(h.id);
		if self.n < h.set_id {
			self.n = h.set_id;
		}
		if h.del {
			self.n = self.n.new_plus(1);
		}
		if self.max_yielded < h.id {
			self.max_yielded = h.id;
		}
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

	pub fn new_header(&mut self, guarantee: Guarantee) -> Header {
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
		let msg_slice = &mut self.endpoint.buf[self.endpoint.buf_free_start..new_end];
		self.endpoint.socket.send(msg_slice)?;

		if guarantee == Guarantee::Delivery {
			// save into outbox and bump the buffer up
			self.endpoint.outbox.insert(id, (Instant::now(), msg_slice as *mut [u8]));
			self.endpoint.buf_free_start = new_end;
		}

		if guarantee != Guarantee::None {
			self.count += 1;
			if guarantee != Guarantee::Delivery {
				self.ord_count += 1;
			}
		}
		Ok(bytes_sent)
	}
}