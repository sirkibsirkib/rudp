# Reliable UDP
## What RUDP is for
RUDP builds some reliability mechanisms on top of UDP that can drop in _as desired_
at runtime to make it act more like TCP.

RUDP is best for cases where you want
some guarantees sometimes, but not always. RUDP exposes some mechanisms for the user to express
_which_ guarantees are needed for which messages. Weaker guarantees translate
to less bookkeeping and buffering, hopefully translating to speed.

What RUDP already provides:
1. Flag each sent message independently to determine the guarantees RUDP will provide for it. 
	1. 'None': UDP behavior. Unordered, not unique, and may get lost.
	2. 'Order': Message will respect ordering and won't arrive redundantly. May get lost.
	3. 'Delivery': TCP behaviour. Ordered, unique, and won't get lost.
1. The user can group messages into _sets_ on the fly. This relaxes the ordering guarantees _only within the set_ but preserves it otherwise. Useful for sending a "group" of messages.
1. Byte grainularity and the `io::Write` API to go with it. This minimizes redundant copying (eg: you can `serde`-serialize into the outbound-buffer without copying).
1. Configuration to tweak the size of buffers, etc.

What RUDP is planned to provide:
1. Optional encryption of headers + nonce fields to make it more secure.
1. Automatic grouping of small payloads into larger UDP Datagrams under the hood.
1. Additional configuration to control heartbeats and when to re-send 
unacknowledged delivery-guaranteed datagrams.

What RUDP does not provide:
1. Some more advanced TCP mechanisms like backpressure control and AIMD.
1. Autonomous threading to perform heartbeats under the hood. (This is intentional).
All work is performed in response to the user's explicit calls.

## Minimizing copying by using _One Big Buffer_
All buffered data written in or out goes through one buffer per `Endpoint` struct.
The size of the buffer is configured and set up-front according to two properties:
1. The largest single-datagram you want to support.
1. The extra space you allot for _sliding_.

After the `Endpoint` stores a payload of bytes, the 'start' pointer _slides_ right.
Auxiliary bookkeeping structures reference the payload as a slice _in place_.
As messages are yielded to the user or overwritten by new data, pointers are deleted,
but no payload bytes need to move. Anytime there are no references, the start pointer is reset.

If there isn't a reset for long enough that the 'start' pointer is approaching the end of the buffer, payloads are copied to a _secondary_ more conventional heap-like storage, and the 'start' pointer is reset. As most data is consumed shortly after it is written, most payloads never reach the secondary storage.
