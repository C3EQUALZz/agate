from uuid import UUID

from audit_verify.domain import LeafSample, TransparencyLogSummary

_LOG_ID = UUID("3f6c1e2a-0000-0000-0000-000000000000")


def _summary(count: int, lo: int | None, hi: int | None) -> TransparencyLogSummary:
    return TransparencyLogSummary(
        log_id=_LOG_ID,
        created_at_ms=1,
        updated_at_ms=2,
        hash_algo_code=1,
        leaf_count=count,
        min_index=lo,
        max_index=hi,
        sample=(),
    )


def test_empty_log_is_contiguous() -> None:
    assert _summary(0, None, None).is_contiguous


def test_gapless_sequence_is_contiguous() -> None:
    assert _summary(9, 0, 8).is_contiguous


def test_gap_breaks_contiguity() -> None:
    assert not _summary(9, 0, 9).is_contiguous  # 10 indices' span, 9 leaves
    assert not _summary(9, 1, 9).is_contiguous  # does not start at 0


def test_leaf_sample_digest_hex() -> None:
    leaf = LeafSample(index=0, leaf_hash=bytes([0x9F, 0x2B, 0x1C]))
    assert leaf.digest_hex == "9f2b1c"
