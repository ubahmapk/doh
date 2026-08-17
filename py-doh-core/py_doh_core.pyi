from typing import ClassVar, Optional

class PyOpCode:
    """The DNS message opcode. Compares equal to its int value too
    (e.g. PyOpCode.QUERY == 0).
    """

    QUERY: ClassVar[PyOpCode]
    STATUS: ClassVar[PyOpCode]
    NOTIFY: ClassVar[PyOpCode]
    UPDATE: ClassVar[PyOpCode]
    UNKNOWN: ClassVar[PyOpCode]
    """Any opcode not covered above -- the server's own response is
    simply echoed back and not otherwise validated."""

    def __int__(self) -> int: ...
    def __eq__(self, other: object) -> bool: ...
    def __hash__(self) -> int: ...

class PyResponseCode:
    """The DNS response code. Transport.resolve()/.aresolve() only ever
    return NOERROR or NXDOMAIN here -- every other code (SERVFAIL,
    REFUSED, ...) is raised as a DohError instead, matching doh-core's
    own behavior. Compares equal to its int value too.
    """

    NOERROR: ClassVar[PyResponseCode]
    NXDOMAIN: ClassVar[PyResponseCode]

    def __int__(self) -> int: ...
    def __eq__(self, other: object) -> bool: ...
    def __hash__(self) -> int: ...

class DohError(Exception):
    """Raised for any doh-core transport/DNS failure. str(exc) is the same
    message the Rust DohError produces (bad server URL, connection
    failure, malformed response, SERVFAIL/REFUSED, ...). There is no
    fallback to classic plaintext DNS, matching the Rust library's own
    behavior.
    """

class PyAnswer:
    """One resource record from an answer, authority, or additional
    section.
    """

    name: str
    """Owner name of the record, e.g. "example.com."."""

    record_type: str
    """Record type mnemonic, e.g. "A", "AAAA", "MX"."""

    ttl: int
    """Time-to-live, in seconds."""

    rdata: str
    """The record data, stringified (e.g. an IP address for "A"/"AAAA",
    a hostname for "CNAME"/"NS")."""

class PyParsedResponse:
    """A successfully-received and parsed DNS response. Only "no error"
    and "name does not exist" responses are ever returned here -- any
    other response code (SERVFAIL, REFUSED, ...) is raised as a
    DohError instead, matching the Rust library's own behavior. There
    is no fallback to classic plaintext DNS.
    """

    id: int
    """The 16-bit DNS message ID."""

    op_code: PyOpCode
    """One of QUERY, STATUS, NOTIFY, UPDATE, UNKNOWN."""

    response_code: PyResponseCode
    """NOERROR or NXDOMAIN."""

    authoritative: bool
    """The "AA" header flag."""

    truncated: bool
    """The "TC" header flag."""

    recursion_desired: bool
    """The "RD" header flag."""

    recursion_available: bool
    """The "RA" header flag."""

    authentic_data: bool
    """The "AD" header flag (DNSSEC)."""

    checking_disabled: bool
    """The "CD" header flag (DNSSEC)."""

    question_name: str
    """The name that was queried, e.g. "example.com."."""

    question_type: str
    """The record type that was queried, e.g. "A"."""

    answers: list[PyAnswer]
    """Records answering the question."""

    authorities: list[PyAnswer]
    """Records naming authoritative servers."""

    additionals: list[PyAnswer]
    """Records offered as extra context (e.g. glue records)."""

    wire_size: int
    """Size of the raw response, in bytes, as received on the wire."""

class PyDohTransport:
    """A DNS-over-HTTPS transport (RFC 8484) bound to a single server URL."""

    def __init__(self, server_url: str, method: Optional[str] = None) -> None:
        """server_url: e.g. "https://dns.google/dns-query" (must be https).
        method: "get" (default) or "post", case-insensitive.
        """

    def resolve(self, name: str, record_type: str) -> PyParsedResponse:
        """Blocking. Releases the GIL for the duration of the network
        call, so other Python threads keep running.
        """

    async def aresolve(self, name: str, record_type: str) -> PyParsedResponse:
        """Same as resolve(), as an awaitable."""

class PyDotTransport:
    """A DNS-over-TLS transport (RFC 7858) bound to a single host[:port]
    (default port 853).
    """

    def __init__(self, server_addr: str) -> None: ...
    def resolve(self, name: str, record_type: str) -> PyParsedResponse:
        """Blocking. Releases the GIL for the duration of the network
        call, so other Python threads keep running.
        """

    async def aresolve(self, name: str, record_type: str) -> PyParsedResponse:
        """Same as resolve(), as an awaitable."""

class PyDoqTransport:
    """A DNS-over-QUIC transport (RFC 9250) bound to a single host[:port]
    (default port 853). The underlying connection is pooled and shared
    across every resolve()/aresolve() call on this instance.
    """

    def __init__(self, server_addr: str) -> None: ...
    def resolve(self, name: str, record_type: str) -> PyParsedResponse:
        """Blocking. Releases the GIL for the duration of the network
        call, so other Python threads keep running.
        """

    async def aresolve(self, name: str, record_type: str) -> PyParsedResponse:
        """Same as resolve(), as an awaitable."""
