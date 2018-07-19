use std::io;
use helper::Guarantee;
use std::net::SocketAddr;

///////////////////////////////// Endpoint & SetSender API /////////////////////
pub trait ImplicitDestSend: io::Write {
	fn clear_written(&mut self);
	fn send_written(&mut self, g: Guarantee) -> io::Result<usize>; 
	fn write_send(&mut self, g: Guarantee, to_write: &[u8]) -> io::Result<()>;
}

pub trait ExplicitDestSend: io::Write {
	fn clear_written(&mut self);
	fn send_written_to(&mut self, g: Guarantee, dest: &SocketAddr) -> io::Result<usize>;
	fn write_send_to(&mut self, g: Guarantee, to_write: &[u8], dest: &SocketAddr) -> io::Result<()>; 
}
///////////////////////// USER-SUPPLIED INNER OBJECT ///////////////////////////

pub trait UdpLike {
	fn send_to(&mut self, payload: &[u8], addr: &SocketAddr) -> io::Result<()>; 
	fn recv_from(&mut self, buf: &mut [u8]) -> io::Result<(usize, SocketAddr)>; 
}
