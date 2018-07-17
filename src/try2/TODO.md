We want to decouple the endpoint state from the endpoint (socket and buffer)=s&b.
the idea is we want one s&b to support multiple states. This is what is needed for a RUDP server endpoint.

TODO