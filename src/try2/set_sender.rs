
/// An Endpoint can send payloads of data. However, all messages sent by a single
/// `SetSender` object of the endpoint are semantically grouped together into an 
/// unordered set. A new set cannot be defined until the current one is dropped.
/// 
/// Note that the concept of _sending_
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
	fn send_written(&mut self, guarantee: Guarantee, dest: &SocketAddr) -> io::Result<usize> {

		// TODO check if you accept the fresh connection
		let mut state = self.states.entry().or_insert_with(
			|| {
				println!("Making new session-state for {:?}", &peer_addr);
				println!("New state consumes {} bytes", mem::size_of::<State>());
				State::new()
			}
		);

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

impl<'a, U> Drop for SetSender<'a, U> where U: UdpLike {
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

impl<'a, U> io::Write for SetSender<'a, U> where U: UdpLike {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
    	self.endpoint.write(bytes)
    }

    fn flush(&mut self) -> io::Result<()> {
    	Ok(())
    }
}
