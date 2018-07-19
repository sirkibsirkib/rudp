#![allow(dead_code)] //////////// REMOVE REMOVE DEBUG DEBUG TODO TODO

use try2::sliding_buffer::SlidingBuffer;
use try2::internal::*;
use try2::traits::*;
use try2::state::*;
use try2::set_sender::SetSender;
use try2::msg_box::*;
use helper::Guarantee;
use std::net::SocketAddr;
use std::{
	collections::{
		HashMap,
		HashSet,
	},
	io, fmt, iter, cmp, mem,
	time::Instant,
	io::ErrorKind,
};
use byteorder::{ReadBytesExt, WriteBytesExt};
use helper::{EndpointConfig, NewSetUnsent};
use mod_ord::ModOrd;

#[derive(Debug)]
pub struct Endpoint<U: UdpLike> {

	//fundamentals
	config: EndpointConfig,
	socket: U,

	// buffer
	sliding_buffer: SlidingBuffer,
	buf_min_space: usize,

	// state 
	states: HashMap<SocketAddr, State>,
}


impl<U> Endpoint<U> where U: UdpLike {
///// PUBLIC

	/// Discard acknowledged outputs, resend lost outputs and send a heartbeat if
	/// necessary.
	pub fn maintain(&mut self) -> io::Result<()> {
		let now = Instant::now();
		for s in self.states.values_mut() {
			s.drop_acknowledged();
			if s.get_time_last_acked().elapsed() >= self.config.min_heartbeat_period {
				s.set_time_last_acked(now);
				s.largest_set_id_yielded.write_to(&mut self.sliding_buffer);
				self.socket.send_to(self.sliding_buffer.get_payload(), s.get_peer_addr())?;
				self.sliding_buffer.reset_payload();
				self.time_last_acked = now;
			}
		}
	}

	
	/// Create a new Endpoint around the given Udp-like object, with the given
	/// configuration.
	pub fn new_with_config(socket: U, config: EndpointConfig) -> Endpoint<U> {
		let buf_min_space = config.max_msg_size + Header::BYTES;
		let buflen = config.max_msg_size + config.buffer_grow_space + Header::BYTES;
		Endpoint {
			config, socket, buf_min_space,
			sliding_buffer: SlidingBuffer::new(buflen),
			inbox2_to_remove: None,
			states: HashMap::new(),
		}
	}

	
	/// Create a new Endpoint around the given Udp-like object, with the default
	/// configuration.
	pub fn new(socket: U) -> Endpoint<U> {
		Self::new_with_config(socket, EndpointConfig::default())
	} 

	
	/// Attempt to yield a message from the peer Endpoint that is ready for receipt.
	/// May block only if the wrapped Udp-like object may block.
	/// recv() calls may not call the inner receive, depending on the contents
	/// of the inbox.
	///
	/// Fatal errors return `Err(_)`
	/// Reads that fail because they would block return `Ok(None)`
	/// Successful reads return `Ok(Some(x))`, where x is an in-place slice into the internal
	/// buffer; thus, you need to drop the slice before interacting with the 
	/// Endpoint again. 
	pub fn recv_from(&mut self) -> io::Result<Option<(&SocketAddr, &mut [u8])>> {
		self.maintain();

		for (peer_addr, state) in self.states.iter_mut() {
			//TODO maybe ack here
			if let Some(msg) = state.pop_inbox_ready() {
				return Ok(Some((peer_addr, msg)))
			}
		}

		// nothing ready from the inbox. receive messages until we can yield
		loop {
			if self.buf_cant_take_another() {
				self.vacate_buffer();
			}

			match self.socket.recv(self.sliding_buffer.get_writable_slice()) {
				Ok((_, 0)) => {
					return Ok(None)
				},
				Err(e) => {
					return if e.kind() == ErrorKind::WouldBlock {
						return Ok(None)
					} else {
						Err(ErrorKind::WouldBlock.into())
					};
				},
				Ok((peer_addr, ModOrd::BYTES)) => {
					let payload = self.sliding_buffer.get_payload_of_len(ModOrd::BYTES);
					let ack = ModOrd::read_from(payload).unwrap();
					self.digest_incoming_ack(ack);
				},
				Ok((peer_addr, bytes)) if bytes >= Header::BYTES => {

					// TODO check if you accept the fresh connection
					let mut state = self.state_for(peer_addr);
					let tail_start_at = bytes - Header::BYTES;
					let (payload, tail_bytes) = self.sliding_buffer.get_payload_of_len(bytes).split_at_mut(tail_start_at);
					let h = Header::read_from(payload)?;
					self.digest_incoming_ack(h.ack);
					if !h.is_valid() || self.known_duplicate(&h) {
						continue;
					}
					let msg = (h, PayloadRef::from_ref(payload));

					// BIG IF ELSE BRANCH.
					if msg.0.id.special() {
						/* read a 'None' guarantee message.
						store - NO
						yield - YES
						*/
						return Ok(Some(msg.1.expose()))
					} else if msg.0.set_id < self.largest_set_id_yielded {
						/* previous-set message. Its OLD data. Must discard to be safe.
						store - NO
						yield - NO
						*/
					} else if msg.0.wait_until > self.n {
						/* future message.
						store - YES
						yield - NO
						*/
						state.inbox_store(msg);
						// shift the buffer right. don't want to obliterate the data
						self.sliding_buffer.shift_right_by(bytes);
					} else if self.seen_before.contains(&msg.0.id) {
						/* current-set message already yielded
						store - NO
						yield - NO
						*/
					} else {
						/* ORDER or DELIVERY message, but we can yield it right away
						store - NO
						yield - YES
						*/
						state.pre_yield(&msg.0);
						return Ok(Some(msg.1.expose()))
					}					
				},
				Ok((_,_)) => (), // invalid size datagram. improper header or bogus.
			}
		}
	}

	
	/// Convenience function that passes a new `SetSender` into the given closure.
	/// See `new_set` for more information.
	pub fn as_set<F,R>(&mut self, addr: &SocketAddr, work: F) -> R
	where
		F: Sized + FnOnce(SetSender) -> R,
		R: Sized,
	{
		work(self.new_set(addr))
	}

	
	/// The `Endpoint` itself implements `Sender`, allowing it to send messages.
	/// `new_set` returns a `SetSender` object, which implements the same trait.
	/// All messages sent by this setsender object have the added semantics of
	/// relaxed ordering _between_ them. 
	pub fn new_set(&mut self, addr: &SocketAddr) -> SetSender {
		if self.out_buf_written > 0 {
			match self.config.new_set_unsent_action {
				NewSetUnsent::Panic => panic!(
					"Endpoint created new set \
					with non-empty write buffer! \
					(Configuration requested a panic)."
				),
				NewSetUnsent::Clear => self.out_buf_written = 0,
				NewSetUnsent::IntoSet => (), // keep the bytes
			}
		}
		self.inner_new_set(addr)
	}



////////////// PRIVATE

	#[inline]
	fn header_in_range(&self, h: &Header) -> bool {
		if self.largest_set_id_yielded.abs_difference(h.set_id)
		> self.config.window_size {
			// outside of window
			false
		}
	}

	#[inline(always)]
	fn inner_new_set(&mut self, addr: &SocketAddr) -> SetSender {
		let state = self.state_for(addr);
		let set_id = self.next_id;
		SetSender::new(self, set_id)
	}

	/*
	Empty the big buffer. Need to make sure that any inbox/outbox data
	that is still inside is relocated to the secondary storage.
	This requires copying over.
	*/
	fn vacate_buffer(&mut self) {
		for state in self.states.values_mut() {
			state.vacate_primary()
		}
		self.buf_free_start = 0;
	}

	fn known_duplicate(&mut self, header: &Header) -> bool {
		let id = header.id;
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
		}
	}
}



impl<U> ExplicitDestSend for Endpoint<U> where U: UdpLike {
	fn send_written_to(&mut self, guarantee: Guarantee, dest: &SocketAddr) -> io::Result<usize> {
		self.inner_new_set(dest).send_written(guarantee)
	}

	fn write_send_to(&mut self, g: Guarantee, to_write: &[u8], dest: &SocketAddr) -> io::Result<()> {
		let mut x = self.inner_new_set(dest);
		x.write_all(to_write)?;
		x.send_written(g)
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

fn state_for<'a>(states: &'a mut HashMap<SocketAddr, State>, peer_addr: &SocketAddr) -> &'a mut State {
	states.entry(&peer_addr).or_insert_with(
		|| {
			println!("Making new session-state for {:?}", &peer_addr);
			println!("New state consumes {} bytes", mem::size_of::<State>());
			State::new()
		}
	)
}


