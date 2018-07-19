
use mod_ord::ModOrd;
use std::{
	cmp,
	io,
	time::Instant,

};

pub type Payload = Vec<u8>;
pub type LastSent = Instant;
pub struct PayloadRef(*mut [u8]);
impl PayloadRef {
	pub fn to_owned(self) -> Payload {
		self.expose().to_vec()
	}
	pub fn expose(self) -> &'static mut [u8] {
		unsafe{&mut *self.1}
	}
	pub fn from_ref(r: &mut [u8]) -> Self {
		PayloadRef(r as *mut [u8])
	}
}


#[derive(Debug, Clone)]
pub struct Header {
	pub id: ModOrd,
	pub set_id: ModOrd,
	pub ack: ModOrd,
	pub wait_until: ModOrd,
	pub del: bool,
}
impl Header {
	const BYTES: usize = 4*4 + 1;

	fn is_valid(&self) -> bool {
		if self.id < self.set_id {
			// set id cannot be AFTER the id
			false
		} else if self.wait_until > self.id {
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
