# Message
Lets consider a message from some edpoint A to endpoint B. 
```
[00 01 02 03 04 05 06 07 08 09 10 11 12 13 14 15 16]
[  |  |  |  |  |  |  |  |  |  |  |  |  |  |  |  |  ]
 \___id____/ \__ack____/ \_depends_/ \__order__/ ^del
 \_____________header_____________________________/
```

receivier keeps track of sequence numbers corresponding roughly with IDs of
messages already YIELDED using variable `N`.

consider a set of messages with ids _q_ to _q+k-1_ (k messages).
Let _d_ of these be _delivery_ messages (marked as such with a bit).
Note _d_ <= _k_.

Each message has fields {
 	// `ack` not important.
	id,
	order,
	delivery,
	depends,
}
1. id carries its unique ID
1. delivery is (1) if the message is marked _delivery_ or (0) otherwise.
1. order will be _q_, with all in the sequence pointing to the ID of the first
message in the sequence.

```
eg:

 00 01 02 03 04 05
 ^
 |  |  |  |  |  |
 '--+--+--+--+--'

 //order value is 00 for all with ids 00-05
```

while yielding these messages, `N` remains in range [q, q+k).
Each message, when yielded, increments `N` by its _delivery_ value (0 or 1).

Once all delivery messages are yielded, `N` == q+d

The _depends_ field on all messages for the _next_ sequence will be q+d.
Accordingly, a message may only be yielded if `N` >= its depends field.
If such a message is yielded, before any incremented, `N` is increased until
`N` == _order_.

Small detail: if a message set X1 contains zero delivery messages, set X2 will
use X0 for its depends message (it skips forward in this way until it hits a
group with a delivery message)

the IDEA is that N is used as a sort of counter for how many of _d_ delivery
messages have arrived in the current set. when all the needed messages are here,
eventually a message from a subsequent set DEPENDING on the arrival of all messages being here itself arrives, it will trigger a jump over to that set.

when a message comes in, its data is checked outright. if it can be yielded 
straight away, it is yielded in-place
otherwise, it is pushed into an Inbox (id->data) and a counter for the order-field is incremented
a sparse hashmap for orderid->requires is also populated on demand. 
these three structures together can be puzzled together in an attempt to yield arbitrary messages
as soon as a new message set yeilds its first message, all data for messages for _prior_ sets
is purged.



enum InboxEntry {
	Yielded,
	ToYield(Vec<u8>),
}

let next: ModOrd = -1
let inbox: HashMap<ModOrd, InboxEntry>

receive message M with {
	<!-- ack: ModOrd, -->
	id: ModOrd,
	requires: ModOrd,
	order: ModOrd,
	increment: bool,
}

if next < orde


consider a range of sequence IDS of size 100, representing some unordered set.
they are all in {delivery, sequence} as they are given a sequence num.
eg: 400->499

let the receiver have some COUNTER that is some integer
while the receiver is < 400 all incoming messages for this range are cached.

when the number is >= 400, numbers arriving for this change are DIGESTED
if a digested number has been seen before it is discarded
otherwise, it checs


### TL;DR
the sequence number set doubles as a counter

