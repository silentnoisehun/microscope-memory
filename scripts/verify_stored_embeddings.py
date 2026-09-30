#!/usr/bin/env python3
"""Does the stored embedding match a fresh reference embedding of the same text?

This closes an open question. Long stored blocks used to disagree badly with the
reference encoder -- sampled 300-700 character blocks averaged 0.9382 cosine and
700-1100 character blocks only 0.7410. The cause turned out to be
`BLOCK_DATA_SIZE`: at 1,024 bytes, `to_block` truncated 4,300 of the 5,183 SciFact
abstracts at byte 1,021, so a long block's *stored text* was a prefix of the
original document while its *vector* had been computed over the full text before
truncation. Text and vector described different strings.

The fix raised the limit to 16 KiB, so this script re-measures the same buckets to
confirm the divergence is gone rather than assuming it is.

For each embedded block it takes the stored vector from embeddings.bin, the stored
text from data.bin, and a freshly computed reference vector for that exact text,
then reports mean cosine by text length. A cosine near 1.0 means the stored text
and the stored vector agree; a cosine that falls with length is the truncation
signature coming back.

Usage:
    python scripts/verify_stored_embeddings.py [--config eval_config.toml] [--sample 400]
"""
from __future__ import annotations

import argparse
import os
import re
import struct
import subprocess
import sys
from pathlib import Path

import numpy as np

ROOT = Path(__file__).resolve().parent.parent
HEADER_SIZE = 50  # BlockHeader, repr(C, packed)
EMB_HEADER_SIZE = 12  # [u32 embedded_count][u32 dim][u32 max_depth]


def read_output_dir(config: Path) -> Path:
    text = config.read_text(encoding="utf-8", errors="replace")
    m = re.search(r'^\s*output_dir\s*=\s*"([^"]+)"', text, re.M)
    if not m:
        raise SystemExit(f"{config}: no output_dir")
    out = Path(m.group(1))
    return out if out.is_absolute() else ROOT / out


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--config", default="eval_config.toml")
    ap.add_argument("--sample", type=int, default=400)
    ap.add_argument("--model", default="sentence-transformers/all-MiniLM-L6-v2")
    a = ap.parse_args()

    out_dir = read_output_dir(ROOT / a.config)

    # data.bin is packed and variable-length: each block's span lives in its own
    # header, so the offsets cannot be derived from the block index.
    headers = (out_dir / "microscope.bin").read_bytes()
    data = (out_dir / "data.bin").read_bytes()
    emb = (out_dir / "embeddings.bin").read_bytes()
    if not headers or not emb:
        raise SystemExit(f"index incomplete in {out_dir}")

    n_blocks = len(headers) // HEADER_SIZE
    count, dim = struct.unpack_from("<II", emb, 0)
    # Layout: [u32 count][u32 dim][u32 max_depth][u32 block_idx x count][f32 vecs]
    emb_idx = np.frombuffer(emb, dtype="<u4", count=count, offset=EMB_HEADER_SIZE)
    vec_off = EMB_HEADER_SIZE + count * 4
    vectors = np.frombuffer(emb, dtype="<f4", count=count * dim, offset=vec_off)
    vectors = vectors.reshape(count, dim)
    print(f"blocks {n_blocks}, embedded {count}, dim {dim}")

    # Collect the text of every embedded block.
    items: list[tuple[int, str]] = []
    for pos in range(count):
        b = int(emb_idx[pos])
        if b >= n_blocks:
            continue
        off = b * HEADER_SIZE
        start = struct.unpack_from("<I", headers, off + 18)[0]
        length = struct.unpack_from("<H", headers, off + 22)[0]
        end = min(start + length, len(data))
        if start >= end:
            continue
        try:
            text = data[start:end].decode("utf-8")
        except UnicodeDecodeError:
            continue
        items.append((pos, text))

    if not items:
        raise SystemExit("no embedded blocks with readable text")

    rng = np.random.default_rng(0)
    pick = rng.permutation(len(items))[: min(a.sample, len(items))]
    sample = [items[i] for i in sorted(pick)]

    try:
        # The reference is built by hand on purpose. `SentenceTransformer.encode`
        # looks like the obvious choice and is wrong here: it does not reproduce
        # the provider for long inputs, which is how the first version of this
        # script invented a divergence that does not exist. Tokenizing with the
        # raw `tokenizers.Tokenizer`, running `AutoModel` and mean-pooling the
        # un-padded sequence reproduces the provider at cosine 1.0000 on exactly
        # the inputs where the SentenceTransformer path scored 0.79.
        from tokenizers import Tokenizer
        from transformers import AutoModel
        import torch
    except ImportError as e:
        print(f"error: {e}", file=sys.stderr)
        return 1

    tok = Tokenizer.from_pretrained(a.model)
    model = AutoModel.from_pretrained(a.model)
    model.eval()

    rows = []
    for pos, text in sample:
        # Drop the [PAD] the tokenizer appends; the provider trims to the real
        # length and the model is run on the real tokens only.
        ids = [i for i in tok.encode(text).ids if i != 0]
        if not ids:
            continue
        with torch.no_grad():
            out = model(input_ids=torch.tensor([ids])).last_hidden_state
            ref = torch.nn.functional.normalize(out.mean(1).detach())[0].numpy()
        rows.append((len(text.encode("utf-8")), float(np.dot(vectors[pos], ref))))

    rows.sort()
    # Buckets in characters, matching the ranges used when the divergence was
    # first observed so the numbers can be compared directly.
    edges = [0, 300, 700, 1100, 2000, 4096, 1 << 30]
    print(f"\n{'bytes':>16} {'n':>5} {'mean cos':>9} {'min cos':>9}")
    for lo, hi in zip(edges, edges[1:]):
        sel = [c for n, c in rows if lo <= n < hi]
        if not sel:
            continue
        label = f"{lo}-{hi}" if hi < (1 << 30) else f"{lo}+"
        print(f"{label:>16} {len(sel):>5} {np.mean(sel):>9.4f} {np.min(sel):>9.4f}")

    worst = sorted(rows, key=lambda r: r[1])[:3]
    print("\nlowest-cosine samples:")
    for n, c in worst:
        print(f"  {n:>6} bytes  cos {c:.4f}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
