# Reliable UDP
RUDP builds some reliability mechanisms on top of UDP that can drop in _as desired_
at runtime to make it act more like TCP.

If TCP would be too strict but UDP would be too liberal, then RUDP might 
provide the functionality you need.

## Example
An `Endpoint` struct wraps some `UdpLike` object with a stateful wrapper.
This object must just support `send` and `recv` (without explcit peer address); 
UdpSocket already has this functionality.

```rust
// wrap a mio UdpSocket
let sock = mio::net::UdpSocket::bind(addr).unwrap();
sock.connect(peer_addr).unwrap();
let mut endpt = Endpoint::new(sock);

// write a single payload b"Hello, there.".
endpt.write(b"Hello").unwrap();
endpt.write(b", there.").unwrap();
endpt.send_written(Guarantee::Order).unwrap();

// Here's a more convenient form when all the data is in one place
endpt.send_payload(Guarantee::Delivery, b"General Kenobi!").unwrap();

// try to receive the next message. Returns `Ok(None)` if the inner UdpLike
// yields some `WouldBlock` error.
if let Some(msg) = endpt.recv().expect("fatal io error!") {
	println!("msg {:?}", msg);
} else {
	println!("Nothing ready yet!");
}

// Send three distinct payloads, but allow the peer to yield them 
// in any order with respect to one another.
endpt.as_set(|mut s| {
	endpt.send_payload(Guarantee::Delivery, b"A")?;
	endpt.send_payload(Guarantee::None,     b"B")?;
	endpt.send_payload(Guarantee::Order,    b"C")?;
}).unwrap();

// pass control flow to the endpoint so that it can still send heartbeats,
// clear old references and resend lost packets if you dont want to send or recv.
endpt.maintain();
```

## Features

### Currently working
1. Stack-like internal buffer for incoming and outgoing storage so that copying of
messages is rare. Received data is NOT copied by default, instead being passed as
an in-place slice.
1. Secondary, temporary heap-like storage if the primary buffer becomes too full.
1. Optional configuration of heartbeat period, window size, buffer size and more.
1. Message-set semantics for relaxed ordering of a set of messages
1. Three tiers of message class, determining guarantees.

### Planned
1. Opt-in encryption of headers + nonce fields to make connections more secure.
1. Automatic grouping of small payloads into larger UDP Datagrams under the hood.
1. Endpoint-server for multiplexing many Endpoints over a single UDP port.

### Not Planned
1. Some more advanced TCP mechanisms like backpressure control and AIMD.
1. Autonomous threading to perform heartbeats under the hood. (This is intentional).
All work is performed in response to the user's explicit calls.

## Managing a connection
A RUDP connection is maintained by an `Endpoint` structure, built on top
of any Udp-like structure. (We suggest using `mio::net::UdpSocket`). 
All its functions come in three categories:
1. outgoing: `send_written`, `io::write`, `send_payload`, ...
1. incoming: `recv`
1. control: `maintain`

The Endpoint does not have its own thread of control, nor do its calls spin endlessly.
The progress of the communication