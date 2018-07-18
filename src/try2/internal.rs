type Payload = Vec<u8>;
type PayloadRef = *mut [u8];
type LastSent = Instant;

struct PayloadRef(*mut [u8]);
impl PayloadRef {
	fn to_owned(self) -> Payload { unsafe{&*msg.1}.to_vec() }
}


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

	fn is_valid(&self) -> bool {
		if h.id < h.set_id {
			// set id cannot be AFTER the id
			false
		} else if h.wait_until > h.id {
			// cannot wait for a message after self
			false
		} else {
			true
		}
	}

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
