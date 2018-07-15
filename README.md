# Rudp
* udp with opt-in reliability mechanisms like tdp.
* particularly useful if you need reliability _some_ of the time.

## API
You interact with an Endpoint() object. created with your input UdpSocket.
send() and recv(). recv() may block depending on your socket settings.

## Relaxing guarantees
### Send types
each message you send is given (as parameter) a `classification` for the guarantees it will have.
weaker guarantees incur less overhead. Be as sparing as possible for best performance.

classes:
* Delivery: each send() will be ordered wrt. other sends. Will arrive exactly once.
* 


### Message Sets

## Under the Hood

### Inbox buffer

### Three tiers of inbox storage