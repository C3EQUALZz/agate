"""Domain model of a transparency-log summary.

Pure: no SQLAlchemy, no I/O. The gateway adapter builds these from the mapped
persistence entities; the CLI renders them.
"""

from __future__ import annotations

from dataclasses import dataclass
from uuid import UUID


@dataclass(frozen=True, slots=True)
class LeafSample:
    """One recorded ``(event, verdict)`` decision — index + Merkle leaf hash."""

    index: int
    leaf_hash: bytes

    @property
    def digest_hex(self) -> str:
        """The Merkle leaf hash as a lowercase hex string."""
        return self.leaf_hash.hex()


@dataclass(frozen=True, slots=True)
class TransparencyLogSummary:
    """A summarized Agate transparency log."""

    log_id: UUID
    created_at_ms: int
    updated_at_ms: int
    hash_algo_code: int
    leaf_count: int
    min_index: int | None
    max_index: int | None
    sample: tuple[LeafSample, ...]

    @property
    def is_contiguous(self) -> bool:
        """The append-only tamper-evidence: a gapless 0..N-1 index sequence.

        A removed or reordered decision would break the Merkle head; a gap in
        ``leaf_index`` is the cheap, local signal of that.
        """
        if self.leaf_count == 0:
            return True
        return self.min_index == 0 and self.max_index == self.leaf_count - 1
