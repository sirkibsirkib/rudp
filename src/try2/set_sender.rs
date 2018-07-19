use std::{
	io,
	time::Instant,
};
use helper::Guarantee;
use mod_ord::ModOrd;
use try2::{
	traits::*,
	state::*,
	internal::*,
};
use std::{
	mem,
	net::SocketAddr,
};


/// An Endpoint can send payloads of data. However, all messages sent by a single
/// `SetSender` object of the endpoint are semantically grouped together into an 
/// unordered set. A new set cannot be defined until the current one is dropped.
/// 
/// Note that the concept of _sending_
#[derive(Debug)]
pub struct SetSender<'a>{
	state: &'a mut State,
	set_id: ModOrd,
	count: u32,
	ord_count: u32,
	peer_addr: &'a SocketAddr,
}

impl<'a> SetSender<'a> {
	fn new(state: &'a mut State, set_id: ModOrd, peer_addr: &SocketAddr) -> SetSender<'a> {
		SetSender {
			peer_addr,
			state,
			set_id,
			count: 0,
			ord_count: 0,
		}
	}
}

impl<'a> ImplicitDestSend for SetSender<'a> {
	fn write_send(&mut self, g: Guarantee, to_write: &[u8]) -> io::Result<()> {
		self.write_all(to_write)?;
		self.send_written(g)
	}

	fn send_written(&mut self, guarantee: Guarantee) -> io::Result<usize> {
		if self.endpoint.buf_cant_take_another() {
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

	fn clear_written(&mut self) {
		self.endpoint.clear_written()
	}
}

impl<'a> Drop for SetSender<'a> {
    fn drop(&mut self) {
		if self.count == 0 {
			// set was empty. nothing to do here
			return;
		}
		// increment the next_id by the number of IDs that the set contained
		self.endpoint.next_id = self.endpoint.next_id.new_plus(self.count);
		if self.ord_count < self.count {
			// there was at least ONE delivery message. future sets must wait fo
			// all of them (instead of waiting for whatever the previous set was waiting for)
			self.endpoint.wait_until = self.endpoint.next_id.new_minus(self.ord_count);
		}
    }
}

impl<'a> io::Write for SetSender<'a> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
    	self.endpoint.write(bytes)
    }

    fn flush(&mut self) -> io::Result<()> {
    	Ok(())
    }
}
