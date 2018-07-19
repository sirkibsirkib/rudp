use std::io;
use std::iter;

/*
                      payload start           
 0                   /         payload end
/                   /         /
[//////////////////|*********|____________]
                             ^--> write appends here
                   ^^^^^^^^^^ current payload
                   ---------->shift_right()
                   ^ payload_end after reset_payload()
                   ^^^^^^^^^^^^^^^^^^^^^^^^ return of get_writable_slice()

*/

#[derive(Debug)]
pub struct SlidingBuffer {
	buf: Vec<u8>,
	payload_start: usize, // INCL
	payload_end: usize, // NOT INCL
}

impl io::Write for SlidingBuffer {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
    	let b = (&mut self.buf[(self.payload_start + self.payload_end)..]).write(bytes)?;
    	self.payload_end += b;
    	Ok(b)
    }

    fn flush(&mut self) -> io::Result<()> { Ok(()) }
}

impl SlidingBuffer {
    pub fn shift_right(&mut self) -> usize {
    	let diff = self.payload_size();
    	self.payload_start = self.payload_end;
    	diff
    }

    pub fn shift_right_by(&mut self, value: usize) {
    	self.payload_start += value;
    	self.payload_end = self.payload_start;
    }

    pub fn reset_start(&mut self) {
    	self.payload_start = 0;
    	self.payload_end = 0;
    }

    pub fn space_left(&self) -> usize {
    	self.buf.len() - self.payload_end
    }

    pub fn new(capacity: usize) -> Self {
    	Self {
    		buf: iter::repeat(0).take(capacity).collect(),
    		payload_start: 0,
    		payload_end: 0,
    	}
    }

    pub fn payload_size(&self) -> usize {
    	self.payload_end - self.payload_start
    }

    pub fn reset_payload(&mut self) -> usize {
    	let diff = self.payload_size();
    	self.payload_end = self.payload_start;
    	diff
    }

    pub fn get_payload(&mut self) -> &mut [u8] {
    	& mut self.buf[self.payload_start..self.payload_end]
    }

    pub fn get_payload_of_len(&mut self, len: usize) -> &mut [u8] {
    	let end = self.payload_start + len;
    	assert!(self.buf.len() >= end);
    	& mut self.buf[self.payload_start..end]
    }

    pub fn set_payload_len(&mut self, len: usize) {
    	self.payload_end = self.payload_start + len;
    	assert!(self.buf.len() >= self.payload_end);
    }

    pub fn get_writable_slice(&mut self) -> &mut [u8] {
    	&mut self.buf[self.payload_start..]
    }
}
