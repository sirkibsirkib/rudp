use super::*;

use std::{
	io,
	time::{
		Duration,
	},
	io::{
		Write,
	},
};
use mio::net::UdpSocket;

use rand::{thread_rng, Rng};
use mio::*;

#[derive(Debug)]
struct BadUdp {
	messages: Vec<Vec<u8>>,
}

impl BadUdp {
	fn new() -> Self {
		BadUdp {
			messages: vec![],
		}
	}

	fn send(&mut self, buf: &[u8]) -> io::Result<usize> {
		let m = buf.to_vec();
		self.messages.push(m.clone());
		self.messages.push(m);
		Ok(buf.len())
	}

	fn recv(&mut self, mut buf: &mut [u8]) -> io::Result<usize> {
		if self.messages.is_empty() {
			Err(io::ErrorKind::WouldBlock.into())
		} else {
			let i = thread_rng().gen_range(0, self.messages.len());
			let m = self.messages.remove(i);
			buf.write(&m)
		}
	}
}



////////////////////////////////////////////////////////////////////////////////
impl UdpLike for BadUdp {
	fn send(&mut self, buf: &[u8]) -> io::Result<usize> {
		BadUdp::send(self, buf)
	}
	fn recv(&mut self, buf: &mut [u8]) -> io::Result<usize> {
		BadUdp::recv(self, buf)
	}
}


impl UdpLike for UdpSocket {
	fn send(&mut self, buf: &[u8]) -> io::Result<usize> {
		UdpSocket::send(self, buf)
	}
	fn recv(&mut self, buf: &mut [u8]) -> io::Result<usize> {
		UdpSocket::recv(self, buf)
	}
}


//////////////////////// TEST////////////////

/*
	This tests a fake connection of some endpoint with itself
	the BadUdp object will connect messages but duplicate and jumble
	them before sending (no loss).
*/
#[test]
fn bad_udp() {
	use Guarantee::*;

	let socket = BadUdp::new();
	let mut config = EndpointConfig::default();
	config.max_msg_size = 16;
	config.buffer_grow_space = 64;
	config.window_size = 32;

	let mut e = Endpoint::new_with_config(socket, config);

	e.send_payload(b"Dank", Delivery).unwrap();
	while let Ok(_) = e.recv() {}

	e.send_payload(b"Lower", Guarantee::Delivery).unwrap();

	e.send_payload(b"Lower...", Delivery).unwrap();
	e.send_payload(b"...case", Delivery).unwrap();

	e.as_set(|mut s| {
		for letter in ('a' as u8)..=('e' as u8) {
			s.send_payload(&vec![letter], Delivery).unwrap();
		}
	});

	e.send_payload(b"Numbers", Delivery).unwrap();

	e.as_set(|mut s| {
		for letter in ('1' as u8)..=('3' as u8) {
			s.send_payload(&vec![letter], Delivery).unwrap();
		}
	});

	e.send_payload(b"Up...", Delivery).unwrap();
	e.send_payload(b"...percase", Delivery).unwrap();


	e.as_set(|mut s| {
		for letter in ('X' as u8)..=('Z' as u8) {
			s.send_payload(&vec![letter], Delivery).unwrap();
		}
	});

	e.send_payload(b"Done", Delivery).unwrap();

	let mut got = vec![];
	while let Ok(msg) = e.recv() {
		let out: String = String::from_utf8_lossy(&msg[..]).to_string();
		println!("--> yielded: {:?}\n", &out);
		got.push(out);
	}
	println!("got: {:?}", got);
	e.send_payload(b"wahey", Delivery).unwrap();
	while let Ok(_) = e.recv() {}

	e.resend_lost().unwrap();

	println!("E {:#?}", e);
}

/*
	This test will check how well Rudp plays with MIO
	the idea is to set up a proper communication channel between two
	endpoints
*/
#[test]
fn mio_pair() {
	let poll = Poll::new().unwrap();
	let mut events = Events::with_capacity(128);
	let addrs = ["127.0.0.1:8888".parse().unwrap(), "127.0.0.1:8889".parse().unwrap()];
	let mut endpoints = {
		let f = |me_id, peer_id| {
			let sock = UdpSocket::bind(&addrs[me_id]).unwrap();
			sock.connect(addrs[peer_id]).unwrap();
			poll.register(&sock, Token(me_id), Ready::readable(), PollOpt::edge()).unwrap();
			Endpoint::new(sock)
		};
		[f(0, 1), f(1, 0)]
	};

	endpoints[0].send_payload(b"a", Guarantee::Delivery).unwrap();
	println!("SEND OVER\n");

	let poll_timeout = Duration::from_millis(1000);
	loop {
		println!("POLL LOOP...");
		poll.poll(&mut events, Some(poll_timeout)).unwrap();
		for event in events.iter() {
			println!("event {:?}", event);
			let endpt = &mut endpoints[event.token().0];
			let reply: Option<u8> = {
				if let Ok(msg) = endpt.recv() {
					println!("msg {:?} ", msg);
					println!("RECV OVER\n");
					if msg[0] < 'd' as u8 {
						Some(msg[0] + 1)
					} else {None}
				} else {None}
				
			};
			if let Some(x) = reply {
				endpt.send_payload(&vec![x][..], Guarantee::Delivery).unwrap();
			}
        }
        for endpt in endpoints.iter_mut() {
        	let _ = endpt.resend_lost();
        }
	}
}
