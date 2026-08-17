"""Live tests against real public resolvers -- there is no mocking layer
here, matching the Rust side's own manual/live verification approach.
Requires network access.
"""

from __future__ import annotations

from typing import TypeAlias

import pytest

import py_doh_core as doh

Transport: TypeAlias = doh.DohTransport | doh.DotTransport | doh.DoqTransport

DOH_GET: doh.DohTransport = doh.DohTransport("https://dns.google/dns-query")
DOH_POST: doh.DohTransport = doh.DohTransport(
    "https://cloudflare-dns.com/dns-query", "post"
)
DOT: doh.DotTransport = doh.DotTransport("dns.google")
DOQ: doh.DoqTransport = doh.DoqTransport("dns.adguard.com")

TRANSPORTS: list[Transport] = [DOH_GET, DOH_POST, DOT, DOQ]


def _skip_if_port_853_unreachable(transport: Transport, exc: doh.DohError) -> None:
    # DoT/DoQ (port 853) is blocked on some networks (corporate firewalls,
    # sandboxes) that otherwise allow plain HTTPS; treat a connect-level
    # failure there as an environment limitation, not a test failure.
    if transport in (DOT, DOQ) and "timed out connecting" in str(exc):
        pytest.skip(f"port 853 unreachable in this network environment: {exc}")


@pytest.mark.parametrize("transport", TRANSPORTS)
def test_resolve_a_record(transport: Transport) -> None:
    try:
        response: doh.ParsedResponse = transport.resolve("example.com", "A")
    except doh.DohError as exc:
        _skip_if_port_853_unreachable(transport, exc)
        raise

    assert response.op_code == doh.OpCode.QUERY
    assert response.response_code == doh.ResponseCode.NOERROR
    assert response.answers
    assert all(isinstance(a.rdata, str) and a.rdata for a in response.answers)
    assert response.question_name.rstrip(".") == "example.com"
    assert response.question_type == "A"
    assert response.wire_size > 0


@pytest.mark.parametrize("transport", TRANSPORTS)
async def test_aresolve_a_record(transport: Transport) -> None:
    try:
        response: doh.ParsedResponse = await transport.aresolve("example.com", "A")
    except doh.DohError as exc:
        _skip_if_port_853_unreachable(transport, exc)
        raise

    assert response.response_code == doh.ResponseCode.NOERROR
    assert response.answers


def test_resolve_nxdomain_is_not_an_error() -> None:
    response = DOH_GET.resolve("this-name-should-not-exist-doh-rs.example", "A")

    assert response.response_code == doh.ResponseCode.NXDOMAIN
    assert response.answers == []


def test_resolve_servfail_raises_doh_error() -> None:
    # A domain with intentionally broken DNSSEC: validating resolvers like
    # dns.google answer SERVFAIL for it, which doh-core surfaces as an
    # error rather than a ParsedResponse (never silently falling back).
    with pytest.raises(doh.DohError):
        DOH_GET.resolve("dnssec-failed.org", "A")


def test_invalid_server_url_raises_doh_error() -> None:
    with pytest.raises(doh.DohError):
        doh.DohTransport("not-a-valid-url")


def test_unknown_record_type_raises_doh_error() -> None:
    with pytest.raises(doh.DohError):
        DOH_GET.resolve("example.com", "NOT_A_RECORD_TYPE")


def test_answers_authorities_additionals_are_populated_separately() -> None:
    response = DOH_GET.resolve("example.com", "A")

    assert isinstance(response.authorities, list)
    assert isinstance(response.additionals, list)


def test_resolve_many_returns_one_result_per_type_in_order() -> None:
    results = DOH_GET.resolve_many("example.com", ["A", "AAAA", "MX"])

    assert [r.record_type for r in results] == ["A", "AAAA", "MX"]
    for r in results:
        assert r.error is None
        assert r.response is not None
        assert r.response.response_code == doh.ResponseCode.NOERROR
        assert r.response.answers


async def test_aresolve_many_returns_one_result_per_type_in_order() -> None:
    results = await DOH_GET.aresolve_many("example.com", ["A", "MX"])

    assert [r.record_type for r in results] == ["A", "MX"]
    for r in results:
        assert r.error is None
        assert r.response is not None


def test_resolve_many_one_failing_type_does_not_abort_the_rest() -> None:
    # SERVFAIL for every type on this domain -- proves a per-type failure
    # is captured on that entry, not raised, and the rest of the batch
    # still runs (matching doh-cli's "query the rest, report each
    # result" behavior).
    results = DOH_GET.resolve_many("dnssec-failed.org", ["A", "AAAA"])

    assert [r.record_type for r in results] == ["A", "AAAA"]
    for r in results:
        assert r.response is None
        assert r.error is not None


def test_resolve_many_unknown_type_raises_before_any_query() -> None:
    with pytest.raises(doh.DohError):
        DOH_GET.resolve_many("example.com", ["A", "NOT_A_RECORD_TYPE"])
