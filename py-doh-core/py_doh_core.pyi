from typing import Optional

class DohError(Exception): ...

class PyAnswer:
    name: str
    record_type: str
    ttl: int
    rdata: str

class PyParsedResponse:
    id: int
    op_code: str
    response_code: str
    authoritative: bool
    truncated: bool
    recursion_desired: bool
    recursion_available: bool
    authentic_data: bool
    checking_disabled: bool
    question_name: str
    question_type: str
    answers: list[PyAnswer]
    authorities: list[PyAnswer]
    additionals: list[PyAnswer]
    wire_size: int

class PyDohTransport:
    def __init__(self, server_url: str, method: Optional[str] = None) -> None: ...
    def resolve(self, name: str, record_type: str) -> PyParsedResponse: ...
    async def aresolve(self, name: str, record_type: str) -> PyParsedResponse: ...

class PyDotTransport:
    def __init__(self, server_addr: str) -> None: ...
    def resolve(self, name: str, record_type: str) -> PyParsedResponse: ...
    async def aresolve(self, name: str, record_type: str) -> PyParsedResponse: ...

class PyDoqTransport:
    def __init__(self, server_addr: str) -> None: ...
    def resolve(self, name: str, record_type: str) -> PyParsedResponse: ...
    async def aresolve(self, name: str, record_type: str) -> PyParsedResponse: ...
