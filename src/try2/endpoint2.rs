#![allow(dead_code)] //////////// REMOVE REMOVE DEBUG DEBUG TODO TODO

use sliding_buffer::SlidingBuffer;
use internal::*;
use traits::*;
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
		for s in states.iter_mut() {
			s.drop_acknowledged();
			if s.get_time_last_acked.elapsed() >= self.config.min_heartbeat_period {
				s.set_time_last_acked(now);
				let b = self.buf_free_start;
				self.largest_set_id_yielded.write_to(&mut self.buf[b..])?;
				self.socket.send(&self.buf[b..(b+ModOrd::BYTES)])?;
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

			match self.socket.recv(&mut self.sliding_buffer.get_writable_slice() {
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
					let ack = ModOrd::read_from(& self.buf[self.buf_free_start..(self.buf_free_start+ModOrd::BYTES)]).unwrap();
					self.digest_incoming_ack(ack);
				},
				Ok((peer_addr, bytes)) if bytes >= Header::BYTES => {

					// TODO check if you accept the fresh connection
					let mut state = self.state_for(peer_addr);
					let h_starts_at = self.buf_free_start + bytes - Header::BYTES;
					let h = Header::read_from(& self.buf[h_starts_at..])?;
					self.digest_incoming_ack(h.ack);
					if self.invalid_header(&h) || self.known_duplicate(&h) {
						continue;
					}
					let msg = Message {
						h,
						payload: (&mut self.buf[self.buf_free_start..h_starts_at]) as *mut [u8],
					};

					// BIG IF ELSE BRANCH.
					if msg.h.id.special() {
						/* read a 'None' guarantee message.
						store - NO
						yield - YES
						*/
						return Ok(Some(unsafe{&mut *msg.payload}))
					} else if msg.h.set_id < self.largest_set_id_yielded {
						/* previous-set message. Its OLD data. Must discard to be safe.
						store - NO
						yield - NO
						*/
						continue;
					} else if msg.h.wait_until > self.n {
						/* future message.
						store - YES
						yield - NO
						*/
						if !self.inbox.contains_key(&msg.h.id) {
							self.inbox.insert(msg.h.id, msg);
						}
						// shift the buffer right. don't want to obliterate the data
						self.buf_free_start = h_starts_at; 
					} else if self.seen_before.contains(&msg.h.id) {
						/* current-set message already yielded
						store - NO
						yield - NO
						*/
					} else {
						/* ORDER or DELIVERY message, but we can yield it right away
						store - NO
						yield - YES
						*/
						self.pre_yield(msg.h.set_id, msg.h.id, msg.h.del);
						return Ok(Some(unsafe{&mut *msg.payload}))
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
		F: Sized + FnOnce(SetSender<U>) -> R,
		R: Sized,
	{
		work(self.new_set(addr))
	}

	
	/// The `Endpoint` itself implements `Sender`, allowing it to send messages.
	/// `new_set` returns a `SetSender` object, which implements the same trait.
	/// All messages sent by this setsender object have the added semantics of
	/// relaxed ordering _between_ them. 
	pub fn new_set(&mut self, &SocketAddr) -> SetSender<U> {
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
	fn inner_new_set(&mut self, addr: &SocketAddr) -> SetSender<U> {
		let state = self.state_for(addr);
		let set_id = self.next_id;
		SetSender::new(self, set_id)
	}

	fn pre_yield(&mut self, set_id: ModOrd, id: ModOrd, del: bool) {
		if set_id > self.largest_set_id_yielded {
			self.largest_set_id_yielded = set_id;
			self.seen_before.clear();
		}
		self.seen_before.insert(id);
		if self.n < set_id {
			self.n = set_id;
		}
		if del {
			self.n = self.n.new_plus(1);
		}
		if self.max_yielded < id {
			self.max_yielded = id;
		}
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
	fn send_written(&mut self, guarantee: Guarantee, dest: &SocketAddr) -> io::Result<usize> {
		self.inner_new_set().send_written(guarantee, dest)
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

fn state_for(states: &mut HashMap<SocketAddr, State>) -> &mut State {
	states.entry().or_insert_with(
		|| {
			println!("Making new session-state for {:?}", &peer_addr);
			println!("New state consumes {} bytes", mem::size_of::<State>());
			State::new()
		}
	)
}


