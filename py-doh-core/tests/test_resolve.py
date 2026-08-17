"""Live tests against real public resolvers -- there is no mocking layer
here, matching the Rust side's own manual/live verification approach.
Requires network access.
"""

from __future__ import annotations

from typing import TypeAlias

import pytest

import py_doh_core

Transport: TypeAlias = py_doh_core.PyDohTransport | py_doh_core.PyDotTransport | py_doh_core.PyDoqTransport

DOH_GET: py_doh_core.PyDohTransport = py_doh_core.PyDohTransport("https://dns.google/dns-query")
DOH_POST: py_doh_core.PyDohTransport = py_doh_core.PyDohTransport(
    "https://cloudflare-dns.com/dns-query", "post"
)
DOT: py_doh_core.PyDotTransport = py_doh_core.PyDotTransport("dns.google")
DOQ: py_doh_core.PyDoqTransport = py_doh_core.PyDoqTransport("dns.adguard.com")

TRANSPORTS: list[Transport] = [DOH_GET, DOH_POST, DOT, DOQ]


def _skip_if_port_853_unreachable(transport: Transport, exc: py_doh_core.DohError) -> None:
    # DoT/DoQ (port 853) is blocked on some networks (corporate firewalls,
    # sandboxes) that otherwise allow plain HTTPS; treat a connect-level
    # failure there as an environment limitation, not a test failure.
    if transport in (DOT, DOQ) and "timed out connecting" in str(exc):
        pytest.skip(f"port 853 unreachable in this network environment: {exc}")


@pytest.mark.parametrize("transport", TRANSPORTS)
def test_resolve_a_record(transport: Transport) -> None:
    try:
        response: py_doh_core.PyParsedResponse = transport.resolve("example.com", "A")
    except py_doh_core.DohError as exc:
        _skip_if_port_853_unreachable(transport, exc)
        raise

    assert response.op_code == py_doh_core.PyOpCode.QUERY
    assert response.response_code == py_doh_core.PyResponseCode.NOERROR
    assert response.answers
    assert all(isinstance(a.rdata, str) and a.rdata for a in response.answers)
    assert response.question_name.rstrip(".") == "example.com"
    assert response.question_type == "A"
    assert response.wire_size > 0


@pytest.mark.parametrize("transport", TRANSPORTS)
async def test_aresolve_a_record(transport: Transport) -> None:
    try:
        response: py_doh_core.PyParsedResponse = await transport.aresolve("example.com", "A")
    except py_doh_core.DohError as exc:
        _skip_if_port_853_unreachable(transport, exc)
        raise

    assert response.response_code == py_doh_core.PyResponseCode.NOERROR
    assert response.answers


def test_resolve_nxdomain_is_not_an_error() -> None:
    response = DOH_GET.resolve("this-name-should-not-exist-doh-rs.example", "A")

    assert response.response_code == py_doh_core.PyResponseCode.NXDOMAIN
    assert response.answers == []


def test_resolve_servfail_raises_doh_error() -> None:
    # A domain with intentionally broken DNSSEC: validating resolvers like
    # dns.google answer SERVFAIL for it, which doh-core surfaces as an
    # error rather than a ParsedResponse (never silently falling back).
    with pytest.raises(py_doh_core.DohError):
        DOH_GET.resolve("dnssec-failed.org", "A")


def test_invalid_server_url_raises_doh_error() -> None:
    with pytest.raises(py_doh_core.DohError):
        py_doh_core.PyDohTransport("not-a-valid-url")


def test_unknown_record_type_raises_doh_error() -> None:
    with pytest.raises(py_doh_core.DohError):
        DOH_GET.resolve("example.com", "NOT_A_RECORD_TYPE")


def test_answers_authorities_additionals_are_populated_separately() -> None:
    response = DOH_GET.resolve("example.com", "A")

    assert isinstance(response.authorities, list)
    assert isinstance(response.additionals, list)
