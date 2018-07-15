
/*
TODO
- organize imports
- log(n) means of scanning inbox
- io::Write interface for sending
- buffered written messages?
- acknowledgements and explicit RESEND_LOST
- encrypt message headers. 
- verify contents of incoming headers
--- ensure sequence numbers are all reasonable and within window
--- check a secret NONCE that is set by the user
- rudp server. takes bind address and 
*/



# TODO
1. nail down io::Write API to work with serialization
1. translate strings into bytes
1. implement growing in-place buffers


## Growing in-place buffers:
the Endpoint struct needs some internal buffer:
`_ _ _ _ _ _ _ _ _ _ _ _ ...`
[                        ...] <-- slice that incoming messages are written to.

after some message is received (say, 3 bytes)
where # are header bytes and 0 are payload bytes for msg 0.

`# # 0 _ _ _ _ _ _ _ _ _ ...`
    [ ] <-- payload slice for msg 0
      [                  ...] <-- subsequent-write slice

several messages may end up in the buffer at once.
`# # 0 # # 1 1 1 1 _ _ _ ...`
    [ ] <-- payload for msg 0
          [       ] <-- payload for msg 1
                  [      ...] <-- subsequent write slice

when a message is consumed, only the bookkeeping for it is removed.
`? ? ? # # 1 1 1 1 _ _ _ ...`

at some stage, the buffer becomes full and a cleanup occurs
all messages still referenced inside the buffer are moved to a separate hashmap
specifically intended for longer-term storage.
Once this is done, the buffer is "clear" again and writing occurs from the 0th byte.

`_ _ _ _ _ _ _ _ _ _ _ _ ...`

## WHY

This approach makes sense given the following (reasonable) assumptions:
1. We want to support very large _possible_ messages. ergo we need a buffer with N+ bytes for some large N
1. Mean size `M` of messages is significantly smaller than N.

By having a buffer not of size N, but rather (N + (M * q)) where q is some small integer,
only after q messages is there not enough space in the buffer.

## API

at its heart, the system exposes functions send() and recv().
The send()ing of a message brings with it typical guarantees:
1. The message will be recv()ed by the peer exactly once
1. Messages are received in the order they were sent.

If at all possible, an incoming message will be yielded directly from the buffer, requiring no redundant copying.
However, as with TCP, maintaining these guarantees often requires extra work:
* outgoing messages must be stored until it is certain they have arrived (in case
they must be resent)
* incoming messages must be stored to facililate reordering as necessary.

Whenever possible, the user is encouraged to _relax_ message guarantees such that
the work is avoided as much as possible.

Each message send() requires an additional parameter, flagging the desired _guarantees_
for the message. These can be specified within 3 tiers:
* DELIVERY: strongest guarantees. will arrive 1 time in the expected order.
* ORDER: medium guarantee. Will arrive 0 or 1 time in the expected order.
* NONE: no guarantees. Will arrive any number of times without any regard for ordering.

Additionally, messages can be grouped into _sets_. This is achieved with a little
extra syntax, but has the effect of relaxing the ordering _between_ messages of a set.


Examples:
* You wish to send messages to a peer. However, stale data is useless and may as well not arrive.
USE `ORDER` GUARANTEE.

* You wish to send some messages, but dont care in which order they arrive relative to each other
SEND THE MESSAGES AS A SET.

* You have a class of message that will do no harm in the event it arrives arbitrarily or not at all. USE NONE GUARANTEE.