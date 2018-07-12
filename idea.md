

## io::Write + Sender trait
sending a message is performed in steps:
1. using the io `write` to write to your `Sender`
1. calling `send()` on your `Sender` (delimiting the end of a message)
1. calling `flush()` to actually send the data. (todo)

your `Sender` object is the Endpoint itself if you want to send individual messages
if you want a messageset, you need to acquire a `SetSender` from the Endpoint
(which also is a `Sender`).

## Message Set
like tcp, messages can be ordered using a sequence. however,
to afford additional flexibility, the ordering between a particular set of 
messages can be relaxed by grouping some messages into a Set.
These messages all have the same _major_ sequence number, but a different
_minor_ sequence number.

## Guarantees
To further push the idea of flexibility, each message can be sent with a different
_guarantee_
* None: basic UDP behavior. message may arrive 0,1,2,... times. Message will not
be ordered wrt other messages.
* Sequence: message will arrive 0 or 1 times. either arrive in correct order or
not at all
* Delivery: message will arrive 1 time in expected ordering