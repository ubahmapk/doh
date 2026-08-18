"""Python bindings for `doh-core`: `DohTransport`, `DotTransport`, and
`DoqTransport` (DNS-over-HTTPS, -TLS, and -QUIC), each with a blocking
`resolve()` and an `async def`-compatible `aresolve()`, plus
`resolve_many()`/`aresolve_many()` for multiple record types in one
call. No fallback to classic plaintext DNS.
"""

from typing import ClassVar

class OpCode:
    """The DNS message opcode. Compares equal to its int value too
    (e.g. OpCode.QUERY == 0).
    """

    QUERY: ClassVar[OpCode]
    STATUS: ClassVar[OpCode]
    NOTIFY: ClassVar[OpCode]
    UPDATE: ClassVar[OpCode]
    UNKNOWN: ClassVar[OpCode]
    """Any opcode not covered above -- the server's own response is
    simply echoed back and not otherwise validated."""

    def __int__(self) -> int: ...
    def __eq__(self, other: object) -> bool: ...
    def __hash__(self) -> int: ...

class ResponseCode:
    """The DNS response code. Transport.resolve()/.aresolve() only ever
    return NOERROR or NXDOMAIN here -- every other code (SERVFAIL,
    REFUSED, ...) is raised as a DohError instead, matching doh-core's
    own behavior. Compares equal to its int value too.
    """

    NOERROR: ClassVar[ResponseCode]
    NXDOMAIN: ClassVar[ResponseCode]

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

class Answer:
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

class ParsedResponse:
    """A successfully-received and parsed DNS response. Only "no error"
    and "name does not exist" responses are ever returned here -- any
    other response code (SERVFAIL, REFUSED, ...) is raised as a
    DohError instead, matching the Rust library's own behavior. There
    is no fallback to classic plaintext DNS.
    """

    id: int
    """The 16-bit DNS message ID."""

    op_code: OpCode
    """One of QUERY, STATUS, NOTIFY, UPDATE, UNKNOWN."""

    response_code: ResponseCode
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

    answers: list[Answer]
    """Records answering the question."""

    authorities: list[Answer]
    """Records naming authoritative servers."""

    additionals: list[Answer]
    """Records offered as extra context (e.g. glue records)."""

    wire_size: int
    """Size of the raw response, in bytes, as received on the wire."""

class QueryResult:
    """The outcome of one record type from a resolve_many()/
    aresolve_many() call. Exactly one of response/error is set: a query
    for one record type can fail (e.g. SERVFAIL) without aborting the
    others, matching doh-cli's own "query the rest, report each result"
    behavior -- only an unparseable record type string aborts the whole
    batch, before any query is sent.
    """

    record_type: str
    """The record type this result is for, e.g. "A"."""

    response: ParsedResponse | None
    """Set on success."""

    error: str | None
    """Set on failure -- the same message a raised DohError would carry."""

class DohTransport:
    """A DNS-over-HTTPS transport (RFC 8484) bound to a single server URL."""

    def __init__(self, server_url: str, method: str | None = None) -> None:
        """server_url: e.g. "https://dns.google/dns-query" (must be https).
        method: "get" (default) or "post", case-insensitive.
        """

    def resolve(self, name: str, record_type: str) -> ParsedResponse:
        """Blocking. Releases the GIL for the duration of the network
        call, so other Python threads keep running.
        """

    async def aresolve(self, name: str, record_type: str) -> ParsedResponse:
        """Same as resolve(), as an awaitable."""

    def resolve_many(self, name: str, record_types: list[str]) -> list[QueryResult]:
        """Resolve name against every type in record_types, blocking.
        Queries run in turn against this same transport; one type's
        failure doesn't abort the rest -- see QueryResult.
        """

    async def aresolve_many(
        self, name: str, record_types: list[str]
    ) -> list[QueryResult]:
        """Same as resolve_many(), as an awaitable."""

class DotTransport:
    """A DNS-over-TLS transport (RFC 7858) bound to a single host[:port]
    (default port 853).
    """

    def __init__(self, server_addr: str) -> None: ...
    def resolve(self, name: str, record_type: str) -> ParsedResponse:
        """Blocking. Releases the GIL for the duration of the network
        call, so other Python threads keep running.
        """

    async def aresolve(self, name: str, record_type: str) -> ParsedResponse:
        """Same as resolve(), as an awaitable."""

    def resolve_many(self, name: str, record_types: list[str]) -> list[QueryResult]:
        """Resolve name against every type in record_types, blocking.
        Queries run in turn against this same transport; one type's
        failure doesn't abort the rest -- see QueryResult.
        """

    async def aresolve_many(
        self, name: str, record_types: list[str]
    ) -> list[QueryResult]:
        """Same as resolve_many(), as an awaitable."""

class DoqTransport:
    """A DNS-over-QUIC transport (RFC 9250) bound to a single host[:port]
    (default port 853). The underlying connection is pooled and shared
    across every resolve()/aresolve() call on this instance.
    """

    def __init__(self, server_addr: str) -> None: ...
    def resolve(self, name: str, record_type: str) -> ParsedResponse:
        """Blocking. Releases the GIL for the duration of the network
        call, so other Python threads keep running.
        """

    async def aresolve(self, name: str, record_type: str) -> ParsedResponse:
        """Same as resolve(), as an awaitable."""

    def resolve_many(self, name: str, record_types: list[str]) -> list[QueryResult]:
        """Resolve name against every type in record_types, blocking.
        Queries run in turn against this same transport; one type's
        failure doesn't abort the rest -- see QueryResult.
        """

    async def aresolve_many(
        self, name: str, record_types: list[str]
    ) -> list[QueryResult]:
        """Same as resolve_many(), as an awaitable."""
